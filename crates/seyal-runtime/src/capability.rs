use std::path::{Path, PathBuf};

#[cfg(target_os = "macos")]
use std::{
    fs::{self, DirBuilder, OpenOptions},
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
};

use seyal_exec::CommandSpec;

use crate::RuntimeError;

const TERM_NAME: &str = "seyal-m001";

#[cfg(target_os = "macos")]
const BUNDLED_ENTRY: &[u8] = include_bytes!(env!("SEYAL_M001_TERMINFO_ENTRY"));

#[derive(Clone, Debug)]
pub struct CapabilityPolicy {
    terminfo_dir: PathBuf,
}

impl CapabilityPolicy {
    pub fn bundled() -> Result<Self, RuntimeError> {
        #[cfg(target_os = "macos")]
        {
            let root = runtime_terminfo_root();
            let entry_dir = root.join("s");
            create_private_dir(&entry_dir)?;
            let entry = entry_dir.join(TERM_NAME);
            if fs::read(&entry).ok().as_deref() != Some(BUNDLED_ENTRY) {
                let temporary = entry_dir.join(format!(".{TERM_NAME}.{}.tmp", std::process::id()));
                let mut file = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .mode(0o600)
                    .open(&temporary)?;
                file.write_all(BUNDLED_ENTRY)?;
                file.sync_all()?;
                fs::rename(&temporary, &entry)?;
            }
            Ok(Self { terminfo_dir: root })
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

#[cfg(target_os = "macos")]
fn runtime_terminfo_root() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join("Library/Caches/dev.seyal/terminfo"))
        .unwrap_or_else(|| std::env::temp_dir().join("seyal/terminfo"))
}

#[cfg(target_os = "macos")]
fn create_private_dir(path: &Path) -> Result<(), RuntimeError> {
    let mut builder = DirBuilder::new();
    builder.recursive(true).mode(0o700);
    builder.create(path)?;
    Ok(())
}
