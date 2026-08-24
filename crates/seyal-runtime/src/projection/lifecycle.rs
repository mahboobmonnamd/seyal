//! SPEC-004 section 8 shared-memory object lifecycle (macOS).
//!
//! Creates the Runtime-writable mapping and an independent read-only
//! descriptor for `SCM_RIGHTS` transfer, then immediately unlinks the name
//! so no other process can `shm_open` it by path. All raw syscalls are
//! isolated to this module.

use std::{
    io,
    os::fd::{FromRawFd, OwnedFd, RawFd},
    sync::atomic::{AtomicU64, Ordering},
};

use crate::projection::layout::RegionHeader;
use crate::projection::writer::RegionMemory;

static NEXT_NAME_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
pub enum LifecycleError {
    NameTooLong,
    ShmOpenWriter(io::Error),
    Truncate(io::Error),
    Mmap(io::Error),
    ShmOpenReader(io::Error),
    Unlink(io::Error),
    CloseOnExec(io::Error),
    RegionTooLarge,
}

/// A Runtime-owned writable projection region plus the independent
/// read-only descriptor meant for exactly one `SCM_RIGHTS` transfer.
pub struct ProjectionRegion {
    writer_fd: OwnedFd,
    ptr: *mut libc::c_void,
    len: usize,
    reader_fd: Option<OwnedFd>,
}

// SAFETY: the mmap'd pointer is process-wide shared memory, not tied to any
// particular thread; all access to its bytes goes through
// `RegionMemory`/`projection::writer`, which document their own
// synchronization. `ProjectionRegion` itself only performs pointer
// bookkeeping and syscalls, none of which assume thread affinity.
unsafe impl Send for ProjectionRegion {}

impl ProjectionRegion {
    /// Creates a fresh, exact-`region_bytes`-sized shared-memory region.
    ///
    /// Follows SPEC-004 section 8.1 exactly: `O_CREAT | O_EXCL` writer
    /// open, overflow-checked sizing, an independent `O_RDONLY` open for
    /// client transfer, then immediate `shm_unlink` so the name can never
    /// be reused/raced by another local actor.
    /// Creates a fresh region sized/described by `header` and writes the
    /// static region header bytes into it immediately.
    ///
    /// Follows SPEC-004 section 8.1 exactly: `O_CREAT | O_EXCL` writer
    /// open, overflow-checked sizing, an independent `O_RDONLY` open for
    /// client transfer, then immediate `shm_unlink` so the name can never
    /// be reused/raced by another local actor.
    pub fn create(header: &RegionHeader) -> Result<Self, LifecycleError> {
        let region_bytes = header.region_bytes;
        if region_bytes < crate::projection::layout::REGION_HEADER_LEN as u64
            || region_bytes > crate::projection::layout::MAX_REGION_BYTES
        {
            return Err(LifecycleError::RegionTooLarge);
        }
        let name = unique_shm_name();

        // SAFETY: `name` is a valid NUL-terminated C string built below;
        // `shm_open` only reads it. The returned fd (if non-negative) is
        // owned exclusively by this call site.
        let writer_raw = unsafe {
            libc::shm_open(
                name.as_ptr(),
                libc::O_CREAT | libc::O_EXCL | libc::O_RDWR,
                0o600,
            )
        };
        if writer_raw < 0 {
            return Err(LifecycleError::ShmOpenWriter(io::Error::last_os_error()));
        }
        // SAFETY: `writer_raw` was just returned by a successful `shm_open`
        // and is not owned anywhere else yet.
        let writer_fd = unsafe { OwnedFd::from_raw_fd(writer_raw) };
        set_close_on_exec(&writer_fd).map_err(LifecycleError::CloseOnExec)?;

        // SAFETY: `writer_fd` is a live, owned descriptor to the region we
        // just created; `region_bytes` was checked non-zero and bounded
        // above.
        let truncate_result = unsafe { libc::ftruncate(writer_raw, region_bytes as libc::off_t) };
        if truncate_result != 0 {
            let error = io::Error::last_os_error();
            unlink_best_effort(&name);
            return Err(LifecycleError::Truncate(error));
        }

        // SAFETY: `writer_raw` is open for read/write and sized to at least
        // `region_bytes` by the successful `ftruncate` above; `region_bytes`
        // fits in `usize` on all supported 64-bit targets since it is
        // bounded by `MAX_REGION_BYTES` (8 MiB).
        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                region_bytes as usize,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                writer_raw,
                0,
            )
        };
        if ptr == libc::MAP_FAILED {
            let error = io::Error::last_os_error();
            unlink_best_effort(&name);
            return Err(LifecycleError::Mmap(error));
        }

        // Write the static region header (SPEC-004 section 9.2) into the
        // mapping now, before any descriptor is handed to a client, so a
        // reader can always independently validate the mapped bytes rather
        // than merely trusting values received over the control socket.
        let mut header_bytes = [0u8; crate::projection::layout::REGION_HEADER_LEN];
        header
            .encode(&mut header_bytes)
            .expect("fixed-size local buffer is always large enough");
        // SAFETY: `ptr` is a live mapping of at least `region_bytes >=
        // REGION_HEADER_LEN` bytes (guaranteed by `RegionHeader` validation
        // requiring both slots, which start at/after the header, to fit
        // within `region_bytes`); no reader/writer protocol has started yet
        // so a plain copy is race-free.
        unsafe {
            std::ptr::copy_nonoverlapping(
                header_bytes.as_ptr(),
                ptr.cast::<u8>(),
                crate::projection::layout::REGION_HEADER_LEN,
            );
        }

        // SAFETY: `name` is the same NUL-terminated string used to create
        // the object above; opening it `O_RDONLY` before unlinking is the
        // only race-free way to hand a read-only descriptor to a future
        // client while still being able to unlink the name immediately
        // after.
        let reader_raw = unsafe { libc::shm_open(name.as_ptr(), libc::O_RDONLY) };
        if reader_raw < 0 {
            let error = io::Error::last_os_error();
            // SAFETY: `ptr`/`region_bytes` came from the successful mmap
            // above and are being torn down on this failure path only.
            unsafe {
                libc::munmap(ptr, region_bytes as usize);
            }
            unlink_best_effort(&name);
            return Err(LifecycleError::ShmOpenReader(error));
        }
        // SAFETY: `reader_raw` was just returned by a successful `shm_open`.
        let reader_fd = unsafe { OwnedFd::from_raw_fd(reader_raw) };
        set_close_on_exec(&reader_fd).map_err(LifecycleError::CloseOnExec)?;

        // SAFETY: both descriptors are acquired; unlinking now (matching
        // SPEC-004 section 8.1 step 5) leaves no name for another process
        // to open, while both live fds/mappings remain fully valid per
        // POSIX shared-memory semantics.
        if unsafe { libc::shm_unlink(name.as_ptr()) } != 0 {
            let error = io::Error::last_os_error();
            // Best-effort: the region is still fully usable even if unlink
            // failed (e.g. already raced away), so this is not fatal, but
            // callers should see it for diagnostics.
            return Err(LifecycleError::Unlink(error));
        }

        Ok(Self {
            writer_fd,
            ptr,
            len: region_bytes as usize,
            reader_fd: Some(reader_fd),
        })
    }

    /// A backend-agnostic handle over the writable mapping for
    /// [`crate::projection::writer::Writer`].
    ///
    /// # Safety
    /// The returned [`RegionMemory`] is valid only for the lifetime of
    /// `self` (the mapping is torn down in `Drop`); callers must not retain
    /// it beyond that.
    pub fn writer_memory(&self) -> RegionMemory {
        // SAFETY: `self.ptr` is a live `mmap` mapping of exactly `self.len`
        // bytes for the lifetime of `self`, and `mmap` always returns
        // page-aligned (hence >= 8-byte-aligned) pointers.
        unsafe { RegionMemory::new(self.ptr.cast::<u8>(), self.len) }
    }

    pub fn region_bytes(&self) -> usize {
        self.len
    }

    pub fn writer_fd(&self) -> RawFd {
        use std::os::fd::AsRawFd;
        self.writer_fd.as_raw_fd()
    }

    /// Takes the read-only descriptor for exactly one `SCM_RIGHTS`
    /// transfer. Returns `None` if it was already taken (SPEC-004 permits
    /// only one live client transfer per created region).
    pub fn take_reader_fd(&mut self) -> Option<OwnedFd> {
        self.reader_fd.take()
    }
}

impl Drop for ProjectionRegion {
    fn drop(&mut self) {
        // SAFETY: `self.ptr`/`self.len` describe the mapping created in
        // `create` and not yet unmapped; `Drop` runs at most once.
        unsafe {
            libc::munmap(self.ptr, self.len);
        }
    }
}

/// A client-side, strictly read-only mapping of a projection region
/// received via `SCM_RIGHTS`. Never `PROT_WRITE`; attempting to map the
/// received descriptor writable is the protocol/security failure SPEC-004
/// section 8.1 forbids.
pub struct ReadOnlyMapping {
    _fd: OwnedFd,
    ptr: *mut libc::c_void,
    len: usize,
}

// SAFETY: same rationale as `ProjectionRegion`'s `Send` impl; all access to
// the mapped bytes goes through `RegionMemory`/`projection::writer`.
unsafe impl Send for ReadOnlyMapping {}

impl ReadOnlyMapping {
    /// Maps `fd` read-only for exactly `len` bytes. Callers must first
    /// validate `len` against `fstat`-reported size, per SPEC-004 section
    /// 9.1, before trusting any encoded offset inside it.
    pub fn new(fd: OwnedFd, len: usize) -> io::Result<Self> {
        use std::os::fd::AsRawFd;
        // SAFETY: `fd` is a live, owned descriptor for the duration of this
        // call; `len` is caller-provided and validated by the caller
        // against the real object size before being trusted further.
        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                len,
                libc::PROT_READ,
                libc::MAP_SHARED,
                fd.as_raw_fd(),
                0,
            )
        };
        if ptr == libc::MAP_FAILED {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { _fd: fd, ptr, len })
    }

    /// A backend-agnostic, read-only handle for
    /// [`crate::projection::writer::read_latest`].
    ///
    /// # Safety
    /// Valid only for the lifetime of `self`.
    pub fn memory(&self) -> RegionMemory {
        // SAFETY: `self.ptr` is a live, `PROT_READ`-only `mmap` mapping of
        // exactly `self.len` bytes for the lifetime of `self`.
        unsafe { RegionMemory::new(self.ptr.cast::<u8>(), self.len) }
    }
}

impl Drop for ReadOnlyMapping {
    fn drop(&mut self) {
        // SAFETY: `self.ptr`/`self.len` describe the mapping created in
        // `new` and not yet unmapped; `Drop` runs at most once.
        unsafe {
            libc::munmap(self.ptr, self.len);
        }
    }
}

fn unique_shm_name() -> std::ffi::CString {
    // Darwin's `sun_path`-independent `shm_open` name limit is short
    // (`PSHMNAMLEN`, 31 bytes including the leading '/' and NUL); keep the
    // encoded name well inside that bound while remaining collision
    // resistant for the lifetime of the brief create/unlink window.
    let pid = std::process::id();
    let sequence = NEXT_NAME_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    let name = format!("/syl{:x}{:x}{:x}", pid, sequence, nanos);
    std::ffi::CString::new(name).expect("generated shm name has no interior NUL")
}

fn unlink_best_effort(name: &std::ffi::CString) {
    // SAFETY: `name` is the same valid NUL-terminated string passed to the
    // preceding `shm_open`. This is best-effort cleanup on an already-error
    // path; its result is intentionally not further escalated.
    unsafe {
        libc::shm_unlink(name.as_ptr());
    }
}

fn set_close_on_exec(fd: &OwnedFd) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    let raw = fd.as_raw_fd();
    // SAFETY: `raw` is a live, owned descriptor for the duration of this
    // call (borrowed from `fd`).
    let flags = unsafe { libc::fcntl(raw, libc::F_GETFD) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: same live fd; `flags` came from the successful `F_GETFD`
    // immediately above.
    if unsafe { libc::fcntl(raw, libc::F_SETFD, flags | libc::FD_CLOEXEC) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projection::layout::{
        CellRecord, ModeFlags, REGION_HEADER_LEN, RegionHeader, WireAttributes, WireColor,
    };
    use crate::projection::writer::{SnapshotWrite, Writer};
    use std::os::fd::AsRawFd;

    fn sample_header(region_bytes: u64) -> RegionHeader {
        RegionHeader {
            region_bytes,
            execution_id: 11,
            attachment_id: 22,
            projection_id: 33,
            slot_stride: 4096,
            slot0_offset: REGION_HEADER_LEN as u64,
            capacity_rows: 2,
            capacity_cols: 2,
        }
    }

    #[test]
    fn create_produces_distinct_writable_and_readonly_descriptors() {
        let header = sample_header(REGION_HEADER_LEN as u64 + 2 * 4096);
        let mut region = ProjectionRegion::create(&header).unwrap();
        assert!(region.writer_fd() >= 0);
        let reader_fd = region.take_reader_fd().expect("reader fd available once");
        assert_ne!(reader_fd.as_raw_fd(), region.writer_fd());
        assert!(region.take_reader_fd().is_none(), "reader fd is single-use");
    }

    #[test]
    fn create_writes_a_decodable_region_header_into_the_mapping() {
        let header = sample_header(REGION_HEADER_LEN as u64 + 2 * 4096);
        let region = ProjectionRegion::create(&header).unwrap();
        let memory = region.writer_memory();
        let bytes = memory.read_bytes(0..REGION_HEADER_LEN).unwrap();
        let decoded = RegionHeader::decode(&bytes).unwrap();
        assert_eq!(decoded, header);
    }

    #[test]
    fn writable_mapping_round_trips_a_published_generation() {
        let header = sample_header(REGION_HEADER_LEN as u64 + 2 * 4096);
        let region = ProjectionRegion::create(&header).unwrap();
        let memory = region.writer_memory();
        let mut writer = Writer::new(memory, header).unwrap();

        let cells = vec![
            CellRecord {
                scalar: ' ',
                foreground: WireColor::Default,
                background: WireColor::Default,
                attributes: WireAttributes::default(),
            };
            4
        ];
        let snapshot = SnapshotWrite {
            rows: 2,
            columns: 2,
            cursor_row: 0,
            cursor_col: 0,
            cursor_visible: true,
            cursor_style: 0,
            mode_flags: ModeFlags::default(),
            cells: &cells,
            damages: &[],
            full_snapshot: true,
            source_damage_generation: 1,
        };
        writer.publish(&snapshot).unwrap();

        let read = crate::projection::writer::read_latest(&memory, &header).unwrap();
        assert_eq!(read.generation, 1);
        assert_eq!(read.cells, cells);
    }

    #[test]
    fn region_larger_than_maximum_is_rejected_before_any_syscall() {
        let header = sample_header(crate::projection::layout::MAX_REGION_BYTES + 1);
        let result = ProjectionRegion::create(&header);
        assert!(matches!(result, Err(LifecycleError::RegionTooLarge)));
    }
}
