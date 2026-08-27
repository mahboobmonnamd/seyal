//! Disposable Seyal.app-side Candidate-D client and renderer-preparation owner.
//!
//! Runtime/TerminalExecution remain the sole PTY, VT and canonical TerminalState
//! authority. This crate owns only a local socket attachment, an atomically
//! committed `DisplayCache`, and derived `seyal-render` presentation state.

#[cfg(target_os = "macos")]
mod local;

#[cfg(target_os = "macos")]
pub use local::{ClientError, LocalDisplayClient};

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod ffi;
