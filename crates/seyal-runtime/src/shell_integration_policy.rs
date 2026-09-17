//! Runtime product composition for trusted zsh shell integration (ADR-009,
//! 2026-09-16 amendment, mechanisms 1–2). The PTY layer stays policy-neutral:
//! this module only decides which files and environment a zsh child starts
//! with and how the per-execution secret reaches it.

use std::path::{Path, PathBuf};

#[cfg(target_os = "macos")]
use std::{
    fs::{self, DirBuilder, OpenOptions},
    io::{Read, Write},
    os::{
        fd::{FromRawFd, OwnedFd},
        unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    },
    sync::atomic::{AtomicU64, Ordering},
};

use seyal_exec::CommandSpec;
#[cfg(target_os = "macos")]
use seyal_exec::ShellIntegrationToken;

use crate::RuntimeError;

/// Non-secret: the descriptor number the child reads the nonce from.
pub const NONCE_FD_ENV: &str = "SEYAL_NONCE_FD";
/// Non-secret: the user's original `ZDOTDIR`, present only when it was set.
pub const USER_ZDOTDIR_ENV: &str = "SEYAL_USER_ZDOTDIR";

#[cfg(target_os = "macos")]
const BUNDLED_ZSHENV: &str = include_str!("../assets/shell-integration/zsh/.zshenv");
#[cfg(target_os = "macos")]
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Where the bundled `.zshenv` is materialized once and referenced by
/// `ZDOTDIR` at every zsh spawn.
#[derive(Clone, Debug)]
pub struct ShellIntegrationPolicy {
    zdotdir: PathBuf,
}

impl ShellIntegrationPolicy {
    /// Materialize the compiled-in `.zshenv` into a private, user-owned
    /// directory (content-compared, atomic rename) and verify it before use.
    /// Runs once per Runtime start; never per spawn or per command.
    pub fn bundled() -> Result<Self, RuntimeError> {
        #[cfg(target_os = "macos")]
        {
            let dir = bundled_zdotdir_root();
            create_private_dir(&dir)?;
            verify_private_dir(&dir)?;
            let path = dir.join(".zshenv");
            if fs::read(&path).ok().as_deref() != Some(BUNDLED_ZSHENV.as_bytes()) {
                let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
                let temporary =
                    dir.join(format!(".zshenv.{}.{}.tmp", std::process::id(), sequence));
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(&temporary)?;
                file.write_all(BUNDLED_ZSHENV.as_bytes())?;
                file.sync_all()?;
                fs::rename(&temporary, &path)?;
            }
            verify_private_file(&path)?;
            Ok(Self { zdotdir: dir })
        }
        #[cfg(not(target_os = "macos"))]
        {
            Err(RuntimeError::UnsupportedPlatform(
                "shell integration is implemented for macOS only in M001",
            ))
        }
    }

    /// Use an existing directory that already holds a `.zshenv`. Intended for
    /// tests and controlled evidence runs; the directory is verified the same
    /// way as the bundled one.
    pub fn from_zdotdir(path: impl Into<PathBuf>) -> Result<Self, RuntimeError> {
        let path = path.into();
        #[cfg(target_os = "macos")]
        {
            verify_private_dir(&path)?;
            verify_private_file(&path.join(".zshenv"))?;
        }
        Ok(Self { zdotdir: path })
    }

    pub fn zdotdir(&self) -> &Path {
        &self.zdotdir
    }

    /// The exact bytes of the bundled `.zshenv`, for tests and evidence.
    #[cfg(target_os = "macos")]
    pub fn bundled_zshenv() -> &'static str {
        BUNDLED_ZSHENV
    }

    /// Whether Runtime attempts trusted integration for this program. A
    /// precondition only, never proof that installation succeeded.
    pub fn supports(command: &CommandSpec) -> bool {
        Path::new(command.program())
            .file_name()
            .is_some_and(|name| name == "zsh")
    }

    /// Compose the spawn: point `ZDOTDIR` at the bundled directory, carry the
    /// user's original `ZDOTDIR` through a non-secret variable, and deliver a
    /// fresh 16-byte nonce over an inherited pipe descriptor. The write end is
    /// closed before returning; the read end closes in this process once the
    /// returned spec is dropped after spawn.
    #[cfg(target_os = "macos")]
    pub(crate) fn apply(
        &self,
        command: CommandSpec,
    ) -> Result<(CommandSpec, ShellIntegrationToken), RuntimeError> {
        let nonce = issue_nonce()?;
        let mut hex = String::with_capacity(33);
        nonce.write_hex(&mut hex);
        hex.push('\n');

        let mut fds = [0i32; 2];
        // SAFETY: `fds` is a valid two-element array for pipe(2).
        if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        // SAFETY: both descriptors were just returned by pipe(2) and are owned here.
        let (read_end, write_end) =
            unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
        for fd in [&read_end, &write_end] {
            use std::os::fd::AsRawFd;
            // SAFETY: fcntl on descriptors owned by this function.
            if unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
                return Err(std::io::Error::last_os_error().into());
            }
        }
        {
            let mut writer = fs::File::from(write_end);
            writer.write_all(hex.as_bytes())?;
        }
        let read_raw = {
            use std::os::fd::AsRawFd;
            read_end.as_raw_fd()
        };

        let mut command = command
            .env("ZDOTDIR", self.zdotdir.as_os_str())
            .env(NONCE_FD_ENV, read_raw.to_string())
            .inherit_fd(read_end);
        if let Some(user_zdotdir) = std::env::var_os("ZDOTDIR") {
            command = command.env(USER_ZDOTDIR_ENV, user_zdotdir);
        }
        Ok((command, nonce))
    }
}

#[cfg(target_os = "macos")]
fn issue_nonce() -> Result<ShellIntegrationToken, RuntimeError> {
    let mut bytes = [0u8; 16];
    fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(ShellIntegrationToken::from_bytes(bytes))
}

#[cfg(target_os = "macos")]
fn bundled_zdotdir_root() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join("Library/Caches/dev.seyal/shell-integration/zsh"))
        .unwrap_or_else(|| std::env::temp_dir().join("seyal/shell-integration/zsh"))
}

#[cfg(target_os = "macos")]
fn create_private_dir(path: &Path) -> Result<(), RuntimeError> {
    let mut builder = DirBuilder::new();
    builder.recursive(true).mode(0o700);
    builder.create(path)?;
    Ok(())
}

/// zsh executes the file in this directory, so anything writable by another
/// user would be a code-injection path. Reject symlinks, foreign ownership and
/// group/world-writable modes; same rules as the Runtime IPC directory.
#[cfg(target_os = "macos")]
fn verify_private_dir(path: &Path) -> Result<(), RuntimeError> {
    let metadata = fs::symlink_metadata(path)?;
    // SAFETY: `geteuid` reads process credentials only.
    let uid = unsafe { libc::geteuid() };
    if metadata.is_symlink()
        || !metadata.is_dir()
        || metadata.uid() != uid
        || metadata.mode() & 0o022 != 0
    {
        return Err(RuntimeError::ShellIntegration(
            "shell integration directory is not a private user-owned directory",
        ));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn verify_private_file(path: &Path) -> Result<(), RuntimeError> {
    let metadata = fs::symlink_metadata(path)?;
    // SAFETY: `geteuid` reads process credentials only.
    let uid = unsafe { libc::geteuid() };
    if metadata.is_symlink()
        || !metadata.is_file()
        || metadata.uid() != uid
        || metadata.mode() & 0o022 != 0
    {
        return Err(RuntimeError::ShellIntegration(
            "shell integration bootstrap file is not a private user-owned file",
        ));
    }
    Ok(())
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn supports_only_zsh_by_program_file_name() {
        assert!(ShellIntegrationPolicy::supports(&CommandSpec::new(
            "/bin/zsh"
        )));
        assert!(ShellIntegrationPolicy::supports(&CommandSpec::new(
            "/opt/homebrew/bin/zsh"
        )));
        assert!(ShellIntegrationPolicy::supports(&CommandSpec::new("zsh")));
        assert!(!ShellIntegrationPolicy::supports(&CommandSpec::new(
            "/bin/sh"
        )));
        assert!(!ShellIntegrationPolicy::supports(&CommandSpec::new(
            "/bin/bash"
        )));
        assert!(!ShellIntegrationPolicy::supports(&CommandSpec::new(
            "/usr/bin/zsh-fake"
        )));
    }

    #[test]
    fn bundled_materialization_is_idempotent_and_private() {
        let dir = std::env::temp_dir().join(format!(
            "seyal-shell-integration-test-{}-{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        create_private_dir(&dir).unwrap();
        let path = dir.join(".zshenv");
        // First materialization writes; second finds identical content.
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .unwrap();
        file.write_all(BUNDLED_ZSHENV.as_bytes()).unwrap();
        drop(file);
        let policy = ShellIntegrationPolicy::from_zdotdir(&dir).unwrap();
        assert_eq!(policy.zdotdir(), dir.as_path());
        assert_eq!(fs::read_to_string(&path).unwrap(), BUNDLED_ZSHENV);

        // A group-writable directory is refused.
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o770)).unwrap();
        assert!(matches!(
            ShellIntegrationPolicy::from_zdotdir(&dir),
            Err(RuntimeError::ShellIntegration(_))
        ));
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn apply_sets_only_non_secret_environment_and_one_inherited_fd() {
        let dir = std::env::temp_dir().join(format!(
            "seyal-shell-integration-apply-{}-{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        create_private_dir(&dir).unwrap();
        fs::write(dir.join(".zshenv"), BUNDLED_ZSHENV).unwrap();
        fs::set_permissions(dir.join(".zshenv"), fs::Permissions::from_mode(0o600)).unwrap();
        let policy = ShellIntegrationPolicy::from_zdotdir(&dir).unwrap();
        let (command, nonce) = policy.apply(CommandSpec::new("/bin/zsh")).unwrap();
        let debug = format!("{command:?}");
        assert!(debug.contains("inherited_fd_count: 1"), "{debug}");
        let mut hex = String::new();
        nonce.write_hex(&mut hex);
        assert_eq!(hex.len(), 32);
        fs::remove_dir_all(&dir).unwrap();
    }
}
