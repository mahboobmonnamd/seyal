//! Explicit `--runtime-dir` selection for an isolated Runtime namespace.
//!
//! Production discovery still defaults to the canonical per-user endpoint.
//! Environment variables never select or relocate that endpoint.

use std::{
    ffi::{OsStr, OsString},
    fmt,
    path::{Path, PathBuf},
    sync::Mutex,
};

pub const RUNTIME_DIR_FLAG: &str = "--runtime-dir";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeDirArgError {
    MissingValue,
    NotAbsolute,
    AlreadySet,
}

impl fmt::Display for RuntimeDirArgError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingValue => {
                write!(f, "missing {RUNTIME_DIR_FLAG} path")
            }
            Self::NotAbsolute => {
                write!(f, "{RUNTIME_DIR_FLAG} path must be absolute")
            }
            Self::AlreadySet => {
                f.write_str("explicit Runtime directory already set for this process")
            }
        }
    }
}

impl std::error::Error for RuntimeDirArgError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessRuntimeArgs {
    pub runtime_dir: Option<PathBuf>,
    pub command: Vec<OsString>,
}

static EXPLICIT_RUNTIME_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);
static OVERRIDE_TEST_LOCK: Mutex<()> = Mutex::new(());

/// Serialize tests that install or observe the process-wide override.
#[doc(hidden)]
pub fn override_test_lock() -> std::sync::MutexGuard<'static, ()> {
    OVERRIDE_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn override_lock() -> std::sync::MutexGuard<'static, Option<PathBuf>> {
    EXPLICIT_RUNTIME_DIR
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn require_absolute(path: PathBuf) -> Result<PathBuf, RuntimeDirArgError> {
    if path.as_os_str().is_empty() || !path.is_absolute() {
        return Err(RuntimeDirArgError::NotAbsolute);
    }
    Ok(path)
}

/// Parse `--runtime-dir PATH` as the first option after argv0. Remaining
/// tokens are the helper command; the flag is never scanned out of that
/// command.
pub fn parse_process_runtime_args<I, S>(args: I) -> Result<ProcessRuntimeArgs, RuntimeDirArgError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut iter = args.into_iter();
    let _argv0 = iter.next();
    let Some(first) = iter.next() else {
        return Ok(ProcessRuntimeArgs {
            runtime_dir: None,
            command: Vec::new(),
        });
    };
    if first.as_ref() == OsStr::new(RUNTIME_DIR_FLAG) {
        let value = iter.next().ok_or(RuntimeDirArgError::MissingValue)?;
        let runtime_dir = require_absolute(PathBuf::from(value.as_ref()))?;
        return Ok(ProcessRuntimeArgs {
            runtime_dir: Some(runtime_dir),
            command: iter.map(|arg| arg.as_ref().to_os_string()).collect(),
        });
    }
    let mut command = vec![first.as_ref().to_os_string()];
    command.extend(iter.map(|arg| arg.as_ref().to_os_string()));
    Ok(ProcessRuntimeArgs {
        runtime_dir: None,
        command,
    })
}

pub fn set_explicit_runtime_dir(path: PathBuf) -> Result<(), RuntimeDirArgError> {
    let path = require_absolute(path)?;
    let mut slot = override_lock();
    match slot.as_ref() {
        Some(existing) if existing == &path => Ok(()),
        Some(_) => Err(RuntimeDirArgError::AlreadySet),
        None => {
            *slot = Some(path);
            Ok(())
        }
    }
}

pub fn explicit_runtime_dir() -> Option<PathBuf> {
    override_lock().clone()
}

/// Test-only reset so isolation cases can share one process.
#[doc(hidden)]
pub fn reset_explicit_runtime_dir() {
    *override_lock() = None;
}

pub fn control_socket_leaf(runtime_dir: &Path) -> PathBuf {
    runtime_dir.join("control.sock")
}

pub fn singleton_lock_path(runtime_dir: &Path) -> PathBuf {
    runtime_dir.join("runtime.lock")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_flag_leaves_command_intact() {
        let parsed =
            parse_process_runtime_args(["seyal-runtime", "/bin/sh", "-c", "true"]).unwrap();
        assert_eq!(parsed.runtime_dir, None);
        assert_eq!(
            parsed.command,
            vec![
                OsString::from("/bin/sh"),
                OsString::from("-c"),
                OsString::from("true")
            ]
        );
    }

    #[test]
    fn runtime_dir_is_consumed_before_the_helper_command() {
        let parsed = parse_process_runtime_args([
            "seyal-runtime",
            "--runtime-dir",
            "/tmp/seyal-iso",
            "/bin/sleep",
            "1",
        ])
        .unwrap();
        assert_eq!(
            parsed.runtime_dir.as_deref(),
            Some(Path::new("/tmp/seyal-iso"))
        );
        assert_eq!(
            parsed.command,
            vec![OsString::from("/bin/sleep"), OsString::from("1")]
        );
    }

    #[test]
    fn runtime_dir_flag_is_not_taken_from_the_command() {
        let parsed =
            parse_process_runtime_args(["seyal-runtime", "/bin/echo", "--runtime-dir", "/tmp/x"])
                .unwrap();
        assert_eq!(parsed.runtime_dir, None);
        assert_eq!(
            parsed.command,
            vec![
                OsString::from("/bin/echo"),
                OsString::from("--runtime-dir"),
                OsString::from("/tmp/x")
            ]
        );
    }

    #[test]
    fn missing_and_relative_paths_fail_closed() {
        assert_eq!(
            parse_process_runtime_args(["seyal-runtime", "--runtime-dir"]).unwrap_err(),
            RuntimeDirArgError::MissingValue
        );
        assert_eq!(
            parse_process_runtime_args(["seyal-runtime", "--runtime-dir", "relative"]).unwrap_err(),
            RuntimeDirArgError::NotAbsolute
        );
    }

    #[test]
    fn argv_without_runtime_dir_flag_does_not_infer_a_directory() {
        let parsed = parse_process_runtime_args(["seyal-runtime"]).unwrap();
        assert_eq!(parsed.runtime_dir, None);
        assert!(parsed.command.is_empty());
    }

    #[test]
    fn explicit_override_is_sticky_and_rejects_a_different_path() {
        let _lock = override_test_lock();
        reset_explicit_runtime_dir();
        let first = PathBuf::from("/tmp/seyal-iso-a");
        set_explicit_runtime_dir(first.clone()).unwrap();
        set_explicit_runtime_dir(first.clone()).unwrap();
        assert_eq!(explicit_runtime_dir(), Some(first));
        assert_eq!(
            set_explicit_runtime_dir(PathBuf::from("/tmp/seyal-iso-b")).unwrap_err(),
            RuntimeDirArgError::AlreadySet
        );
        reset_explicit_runtime_dir();
        assert_eq!(explicit_runtime_dir(), None);
    }
}
