use std::path::{Path, PathBuf};

use seyal_exec::CommandSpec;

use crate::RuntimeError;

const TERM_NAME: &str = "seyal-m001";

#[derive(Clone, Debug)]
pub struct CapabilityPolicy {
    terminfo_dir: PathBuf,
}

impl CapabilityPolicy {
    pub fn bundled() -> Result<Self, RuntimeError> {
        #[cfg(target_os = "macos")]
        {
            let path = PathBuf::from(env!("SEYAL_M001_TERMINFO_DIR"));
            if path.as_os_str().is_empty() || !path.is_dir() {
                return Err(RuntimeError::Terminfo(
                    "bundled seyal-m001 database was not produced by the build".into(),
                ));
            }
            Ok(Self { terminfo_dir: path })
        }
        #[cfg(not(target_os = "macos"))]
        {
            Err(RuntimeError::UnsupportedPlatform(
                "M001 local capability policy is implemented for macOS only",
            ))
        }
    }

    pub fn from_terminfo_dir(path: impl Into<PathBuf>) -> Result<Self, RuntimeError> {
        let path = path.into();
        if !path.is_dir() {
            return Err(RuntimeError::Terminfo(
                "configured terminfo directory does not exist".into(),
            ));
        }
        Ok(Self { terminfo_dir: path })
    }

    pub fn terminfo_dir(&self) -> &Path {
        &self.terminfo_dir
    }

    pub fn apply(&self, command: CommandSpec) -> CommandSpec {
        command
            .env("TERM", TERM_NAME)
            .env("TERMINFO", self.terminfo_dir.as_os_str())
    }
}

pub fn m001_term_name() -> &'static str {
    TERM_NAME
}
