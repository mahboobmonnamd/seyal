//! Disposable Seyal.app-side Candidate-D client and renderer-preparation owner.
//!
//! Runtime/TerminalExecution remain the sole PTY, VT and canonical TerminalState
//! authority. This crate owns only a local socket attachment, an atomically
//! committed `DisplayCache`, and derived `seyal-render` presentation state.

mod block;

// Keep the existing internal import path mechanically stable while severing the
// production dependency on the Runtime crate. `seyal_runtime` below is only an
// alias for the authority-neutral protocol/value crate; integration tests still
// use the real Runtime as a dev-dependency.
#[cfg(target_os = "macos")]
extern crate seyal_protocol as seyal_runtime;

#[cfg(target_os = "macos")]
mod local;
#[cfg(all(target_os = "macos", feature = "benchmark-instrumentation"))]
#[doc(hidden)]
pub mod pass7_benchmark;

#[cfg(target_os = "macos")]
pub use local::{
    ClientError, GridGeometry, InputAdmissionFailure, LocalDisplayClient, ResizeFailure,
    derive_grid_geometry,
};

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod ffi;
