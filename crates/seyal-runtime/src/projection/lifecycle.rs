//! SPEC-004 section 8 shared-memory object lifecycle (macOS).
//!
//! Creates one Runtime-writable POSIX shared-memory mapping and an
//! independently opened read-only descriptor for `SCM_RIGHTS` transfer. The
//! name is unlinked before the region is published to any attachment.

use std::{
    io,
    os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
    sync::atomic::{AtomicU64, Ordering},
};

use crate::projection::layout::{MAX_REGION_BYTES, REGION_HEADER_LEN, RegionHeader};
use crate::projection::writer::RegionMemory;

static NEXT_NAME_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
pub enum LifecycleError {
    ShmOpenWriter(io::Error),
    Truncate(io::Error),
    Mmap(io::Error),
    ShmOpenReader(io::Error),
    Unlink(io::Error),
    CloseOnExec(io::Error),
    RegionTooLarge,
}

pub struct ProjectionRegion {
    writer_fd: OwnedFd,
    ptr: *mut libc::c_void,
    len: usize,
    reader_fd: Option<OwnedFd>,
}

// SAFETY: pointer bookkeeping is thread-independent; all shared bytes are
// accessed through `RegionMemory`, whose concurrent accesses are atomic.
unsafe impl Send for ProjectionRegion {}

impl ProjectionRegion {
    pub fn create(header: &RegionHeader) -> Result<Self, LifecycleError> {
        let region_bytes = header.region_bytes;
        if region_bytes < REGION_HEADER_LEN as u64 || region_bytes > MAX_REGION_BYTES {
            return Err(LifecycleError::RegionTooLarge);
        }
        let name = unique_shm_name();

        // macOS shm_open(2) does not accept O_CLOEXEC. Create the object with
        // only the supported POSIX shm flags, then enforce FD_CLOEXEC with
        // fcntl before the descriptor can escape this function.
        // SAFETY: valid NUL-terminated name; successful descriptor becomes
        // uniquely owned below.
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
        // SAFETY: fresh descriptor returned by `shm_open`.
        let writer_fd = unsafe { OwnedFd::from_raw_fd(writer_raw) };
        if let Err(error) = ensure_close_on_exec(&writer_fd) {
            unlink_best_effort(&name);
            return Err(LifecycleError::CloseOnExec(error));
        }

        // SAFETY: live writable descriptor; bounded nonzero size.
        if unsafe { libc::ftruncate(writer_raw, region_bytes as libc::off_t) } != 0 {
            let error = io::Error::last_os_error();
            unlink_best_effort(&name);
            return Err(LifecycleError::Truncate(error));
        }

        // SAFETY: descriptor has been resized successfully and remains open.
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

        let mut header_bytes = [0u8; REGION_HEADER_LEN];
        header
            .encode(&mut header_bytes)
            .expect("fixed-size region header buffer");
        // SAFETY: no reader can exist yet; this is the one initialization
        // copy before the read-only descriptor is transferred.
        unsafe {
            std::ptr::copy_nonoverlapping(
                header_bytes.as_ptr(),
                ptr.cast::<u8>(),
                REGION_HEADER_LEN,
            );
        }

        // SAFETY: same live shared-memory name, independently opened read-only.
        // As above, apply FD_CLOEXEC explicitly after shm_open succeeds.
        let reader_raw = unsafe { libc::shm_open(name.as_ptr(), libc::O_RDONLY, 0) };
        if reader_raw < 0 {
            let error = io::Error::last_os_error();
            // SAFETY: exact mapping created above, not yet owned by a
            // `ProjectionRegion` value.
            unsafe { libc::munmap(ptr, region_bytes as usize) };
            unlink_best_effort(&name);
            return Err(LifecycleError::ShmOpenReader(error));
        }
        // SAFETY: fresh descriptor returned by `shm_open`.
        let reader_fd = unsafe { OwnedFd::from_raw_fd(reader_raw) };
        if let Err(error) = ensure_close_on_exec(&reader_fd) {
            // SAFETY: exact mapping created above.
            unsafe { libc::munmap(ptr, region_bytes as usize) };
            unlink_best_effort(&name);
            return Err(LifecycleError::CloseOnExec(error));
        }

        // SAFETY: both owned descriptors are live; unlink only removes the
        // global name, not either descriptor/mapping.
        if unsafe { libc::shm_unlink(name.as_ptr()) } != 0 {
            let error = io::Error::last_os_error();
            // SAFETY: exact mapping created above.
            unsafe { libc::munmap(ptr, region_bytes as usize) };
            unlink_best_effort(&name);
            return Err(LifecycleError::Unlink(error));
        }

        Ok(Self {
            writer_fd,
            ptr,
            len: region_bytes as usize,
            reader_fd: Some(reader_fd),
        })
    }

    pub fn writer_memory(&self) -> RegionMemory {
        // SAFETY: live page-aligned mapping for the lifetime of `self`.
        unsafe { RegionMemory::new(self.ptr.cast::<u8>(), self.len) }
    }

    pub fn region_bytes(&self) -> usize {
        self.len
    }

    pub fn writer_fd(&self) -> RawFd {
        self.writer_fd.as_raw_fd()
    }

    pub fn take_reader_fd(&mut self) -> Option<OwnedFd> {
        self.reader_fd.take()
    }
}

impl Drop for ProjectionRegion {
    fn drop(&mut self) {
        // SAFETY: mapping is owned by this value and dropped once.
        unsafe { libc::munmap(self.ptr, self.len) };
    }
}

pub struct ReadOnlyMapping {
    _fd: OwnedFd,
    ptr: *mut libc::c_void,
    len: usize,
}

unsafe impl Send for ReadOnlyMapping {}

impl ReadOnlyMapping {
    /// Validates the transferred descriptor itself before mapping: the backing
    /// object must cover `len`, `len` is ABI-bounded, and the descriptor access
    /// mode must be read-only. Darwin may report a page-rounded shm extent, so
    /// an extent larger than the logical ABI region is valid; only `len` bytes
    /// are mapped and exposed to the projection reader.
    pub fn new(fd: OwnedFd, len: usize) -> io::Result<Self> {
        if len < REGION_HEADER_LEN || len > MAX_REGION_BYTES as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "projection region size outside ABI bounds",
            ));
        }
        let raw = fd.as_raw_fd();
        let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
        // SAFETY: `stat` is a valid writable out-parameter and `raw` is live.
        if unsafe { libc::fstat(raw, stat.as_mut_ptr()) } != 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: successful `fstat` initialized the value.
        let stat = unsafe { stat.assume_init() };
        if stat.st_size < 0 || (stat.st_size as usize) < len {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "projection descriptor is smaller than the control-frame region",
            ));
        }
        // SAFETY: `F_GETFL` only queries the live descriptor.
        let status_flags = unsafe { libc::fcntl(raw, libc::F_GETFL) };
        if status_flags < 0 {
            return Err(io::Error::last_os_error());
        }
        if status_flags & libc::O_ACCMODE != libc::O_RDONLY {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "projection descriptor is not read-only",
            ));
        }

        // SAFETY: validated live descriptor and exact bounded mapping length.
        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                len,
                libc::PROT_READ,
                libc::MAP_SHARED,
                raw,
                0,
            )
        };
        if ptr == libc::MAP_FAILED {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { _fd: fd, ptr, len })
    }

    pub fn memory(&self) -> RegionMemory {
        // SAFETY: live read-only page-aligned mapping for the lifetime of self.
        unsafe { RegionMemory::new(self.ptr.cast::<u8>(), self.len) }
    }
}

impl Drop for ReadOnlyMapping {
    fn drop(&mut self) {
        // SAFETY: mapping is owned by this value and dropped once.
        unsafe { libc::munmap(self.ptr, self.len) };
    }
}

fn unique_shm_name() -> std::ffi::CString {
    let pid = std::process::id();
    let sequence = NEXT_NAME_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    std::ffi::CString::new(format!("/syl{:x}{:x}{:x}", pid, sequence, nanos))
        .expect("generated shm name has no interior NUL")
}

fn unlink_best_effort(name: &std::ffi::CString) {
    // SAFETY: valid name originally passed to `shm_open`.
    unsafe { libc::shm_unlink(name.as_ptr()) };
}

fn ensure_close_on_exec(fd: &OwnedFd) -> io::Result<()> {
    let raw = fd.as_raw_fd();
    // SAFETY: live descriptor.
    let flags = unsafe { libc::fcntl(raw, libc::F_GETFD) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    if flags & libc::FD_CLOEXEC == 0 {
        // SAFETY: live descriptor and flags from `F_GETFD`.
        if unsafe { libc::fcntl(raw, libc::F_SETFD, flags | libc::FD_CLOEXEC) } < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projection::layout::{CellRecord, ModeFlags, WireAttributes, WireColor};
    use crate::projection::writer::{SnapshotWrite, Writer, read_latest, read_region_header};

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
        assert_eq!(read_region_header(&region.writer_memory()).unwrap(), header);
    }

    #[test]
    fn transferred_reader_descriptor_is_enforced_read_only() {
        let header = sample_header(REGION_HEADER_LEN as u64 + 2 * 4096);
        let mut region = ProjectionRegion::create(&header).unwrap();
        let reader_fd = region.take_reader_fd().unwrap();
        let mapping = ReadOnlyMapping::new(reader_fd, region.region_bytes()).unwrap();
        assert_eq!(read_region_header(&mapping.memory()).unwrap(), header);
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
        let read = read_latest(&memory, &header).unwrap();
        assert_eq!(read.generation, 1);
        assert_eq!(read.cells, cells);
    }

    #[test]
    fn region_larger_than_maximum_is_rejected_before_any_syscall() {
        let header = sample_header(MAX_REGION_BYTES + 1);
        let result = ProjectionRegion::create(&header);
        assert!(matches!(result, Err(LifecycleError::RegionTooLarge)));
    }
}
