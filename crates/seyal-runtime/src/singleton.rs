use std::{fs::File, path::Path};

#[cfg(target_os = "macos")]
use std::{
    fs::{DirBuilder, OpenOptions},
    os::{
        fd::AsRawFd,
        unix::fs::{DirBuilderExt, OpenOptionsExt},
    },
};

use crate::RuntimeError;

pub(crate) struct SingletonGuard {
    #[allow(dead_code)]
    file: File,
}

impl SingletonGuard {
    pub(crate) fn acquire(path: &Path) -> Result<Self, RuntimeError> {
        #[cfg(target_os = "macos")]
        {
            let parent = path.parent().ok_or_else(|| {
                RuntimeError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "singleton path has no parent",
                ))
            })?;
            let mut builder = DirBuilder::new();
            builder.recursive(true).mode(0o700);
            builder.create(parent)?;

            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(path)?;
            crate::platform::set_close_on_exec(file.as_raw_fd())?;
            if !crate::platform::try_lock_exclusive(file.as_raw_fd())? {
                return Err(RuntimeError::AlreadyRunning);
            }
            Ok(Self { file })
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = path;
            Err(RuntimeError::UnsupportedPlatform(
                "M001 Runtime singleton is implemented for macOS only",
            ))
        }
    }
}

#[cfg(target_os = "macos")]
impl Drop for SingletonGuard {
    fn drop(&mut self) {
        // Release deliberately instead of relying only on descriptor-close
        // semantics. Drop cannot report failure; closing the owned descriptor
        // immediately afterward remains the final kernel cleanup boundary.
        let _ = crate::platform::unlock(self.file.as_raw_fd());
    }
}
