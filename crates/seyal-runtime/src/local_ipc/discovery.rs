//! SPEC-004 section 4 Runtime discovery and endpoint security (macOS).

use std::{
    ffi::{CString, OsString},
    fs::{DirBuilder, Metadata},
    io,
    os::unix::{
        ffi::OsStringExt,
        fs::{DirBuilderExt, MetadataExt},
    },
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub enum DiscoveryError {
    ConfstrFailed,
    NotADirectory,
    NotOwnedByEffectiveUser,
    GroupOrWorldWritable,
    ActiveEndpoint,
    Io(io::Error),
    PathTooLongForSocket,
}

impl From<io::Error> for DiscoveryError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

const SUN_PATH_MAX: usize = 104;
pub const CONTROL_SOCKET_NAME: &str = "control.sock";

pub fn darwin_user_runtime_dir() -> Result<PathBuf, DiscoveryError> {
    // SAFETY: standard two-call `confstr` pattern with owned storage.
    let required =
        unsafe { libc::confstr(libc::_CS_DARWIN_USER_TEMP_DIR, std::ptr::null_mut(), 0) };
    if required == 0 {
        return Err(DiscoveryError::ConfstrFailed);
    }
    let mut buffer = vec![0u8; required];
    // SAFETY: buffer is exactly the size returned above.
    let written = unsafe {
        libc::confstr(
            libc::_CS_DARWIN_USER_TEMP_DIR,
            buffer.as_mut_ptr().cast::<libc::c_char>(),
            buffer.len(),
        )
    };
    if written == 0 || written > buffer.len() {
        return Err(DiscoveryError::ConfstrFailed);
    }
    if let Some(nul_index) = buffer.iter().position(|&b| b == 0) {
        buffer.truncate(nul_index);
    }
    Ok(PathBuf::from(OsString::from_vec(buffer)).join("seyal-runtime"))
}

pub fn ensure_verified_runtime_dir(dir: &Path) -> Result<(), DiscoveryError> {
    match std::fs::symlink_metadata(dir) {
        Ok(metadata) => verify_directory_metadata(&metadata)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let mut builder = DirBuilder::new();
            builder.recursive(false).mode(0o700);
            match builder.create(dir) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(DiscoveryError::Io(error)),
            }
            verify_directory_metadata(&std::fs::symlink_metadata(dir)?)?;
        }
        Err(error) => return Err(DiscoveryError::Io(error)),
    }
    Ok(())
}

fn verify_directory_metadata(metadata: &Metadata) -> Result<(), DiscoveryError> {
    if metadata.is_symlink() || !metadata.is_dir() {
        return Err(DiscoveryError::NotADirectory);
    }
    // SAFETY: `geteuid` reads process credentials only.
    let effective_uid = unsafe { libc::geteuid() };
    if metadata.uid() != effective_uid {
        return Err(DiscoveryError::NotOwnedByEffectiveUser);
    }
    if metadata.mode() & 0o077 != 0 {
        return Err(DiscoveryError::GroupOrWorldWritable);
    }
    Ok(())
}

pub fn control_socket_path(runtime_dir: &Path) -> Result<PathBuf, DiscoveryError> {
    let path = runtime_dir.join(CONTROL_SOCKET_NAME);
    let as_cstring = CString::new(path.as_os_str().as_encoded_bytes().to_vec())
        .map_err(|_| DiscoveryError::PathTooLongForSocket)?;
    if as_cstring.as_bytes_with_nul().len() > SUN_PATH_MAX {
        return Err(DiscoveryError::PathTooLongForSocket);
    }
    Ok(path)
}

/// Verifies a pre-existing socket leaf without following symlinks and proves
/// it is not currently connectable before the singleton-holding Runtime may
/// remove it. A live endpoint is never treated as stale.
pub fn remove_verified_stale_socket(path: &Path) -> Result<(), DiscoveryError> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(DiscoveryError::Io(error)),
    };
    if metadata.is_symlink() {
        return Err(DiscoveryError::NotADirectory);
    }
    use std::os::unix::fs::FileTypeExt;
    if !metadata.file_type().is_socket() {
        return Err(DiscoveryError::NotADirectory);
    }
    // SAFETY: `geteuid` reads process credentials only.
    let effective_uid = unsafe { libc::geteuid() };
    if metadata.uid() != effective_uid {
        return Err(DiscoveryError::NotOwnedByEffectiveUser);
    }

    match std::os::unix::net::UnixStream::connect(path) {
        Ok(_) => return Err(DiscoveryError::ActiveEndpoint),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
            ) => {}
        Err(error) => return Err(DiscoveryError::Io(error)),
    }

    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(DiscoveryError::Io(error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn temp_scope(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "seyal-discovery-test-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn darwin_user_runtime_dir_resolves_a_nonempty_path() {
        let dir = darwin_user_runtime_dir().unwrap();
        assert!(dir.ends_with("seyal-runtime"));
    }

    #[test]
    fn ensure_verified_runtime_dir_creates_a_0700_directory() {
        let dir = temp_scope("create");
        ensure_verified_runtime_dir(&dir).unwrap();
        let metadata = std::fs::symlink_metadata(&dir).unwrap();
        assert!(metadata.is_dir());
        assert_eq!(metadata.mode() & 0o777, 0o700);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn ensure_verified_runtime_dir_rejects_a_symlink_leaf() {
        let real_dir = temp_scope("real");
        std::fs::create_dir(&real_dir).unwrap();
        let symlink_path = temp_scope("symlink");
        std::os::unix::fs::symlink(&real_dir, &symlink_path).unwrap();
        assert!(matches!(
            ensure_verified_runtime_dir(&symlink_path),
            Err(DiscoveryError::NotADirectory)
        ));
        std::fs::remove_file(&symlink_path).unwrap();
        std::fs::remove_dir_all(&real_dir).unwrap();
    }

    #[test]
    fn ensure_verified_runtime_dir_rejects_group_writable_existing_directory() {
        let dir = temp_scope("group-writable");
        let mut builder = DirBuilder::new();
        builder.recursive(false).mode(0o770);
        builder.create(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o770)).unwrap();
        assert!(matches!(
            ensure_verified_runtime_dir(&dir),
            Err(DiscoveryError::GroupOrWorldWritable)
        ));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn control_socket_path_rejects_path_exceeding_sun_path_capacity() {
        let too_long = PathBuf::from("/").join("a".repeat(200));
        assert!(matches!(
            control_socket_path(&too_long),
            Err(DiscoveryError::PathTooLongForSocket)
        ));
    }

    #[test]
    fn stale_socket_removal_accepts_missing_path() {
        assert!(remove_verified_stale_socket(&temp_scope("missing")).is_ok());
    }

    #[test]
    fn stale_socket_removal_rejects_symlink() {
        let real_dir = temp_scope("real-for-stale");
        std::fs::create_dir(&real_dir).unwrap();
        let symlink_path = temp_scope("stale-symlink");
        std::os::unix::fs::symlink(&real_dir, &symlink_path).unwrap();
        assert!(matches!(
            remove_verified_stale_socket(&symlink_path),
            Err(DiscoveryError::NotADirectory)
        ));
        std::fs::remove_file(&symlink_path).unwrap();
        std::fs::remove_dir_all(&real_dir).unwrap();
    }

    #[test]
    fn stale_socket_removal_rejects_non_socket_regular_file() {
        let path = temp_scope("regular-file");
        std::fs::write(&path, b"not a socket").unwrap();
        assert!(matches!(
            remove_verified_stale_socket(&path),
            Err(DiscoveryError::NotADirectory)
        ));
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn active_owned_socket_is_never_unlinked_as_stale() {
        let path = std::env::temp_dir().join(format!("syl{}.sock", std::process::id() % 100_000));
        let _ = std::fs::remove_file(&path);
        let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
        assert!(matches!(
            remove_verified_stale_socket(&path),
            Err(DiscoveryError::ActiveEndpoint)
        ));
        assert!(path.exists());
        drop(listener);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn dead_owned_socket_is_removed() {
        let path = std::env::temp_dir().join(format!(
            "syl-dead-{}-{}.sock",
            std::process::id() % 100_000,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        ));
        let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
        drop(listener);
        remove_verified_stale_socket(&path).unwrap();
        assert!(!path.exists());
    }
}
