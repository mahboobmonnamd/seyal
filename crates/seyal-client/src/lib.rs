//! Disposable Seyal.app-side Candidate-D client, renderer-preparation owner,
//! portable headed product composition (Workspace/Tab/Pane), Flow/Raw/TUI
//! presentation fencing, and portable theme/config resolution.
//!
//! Runtime/TerminalExecution remain the sole PTY, VT and canonical TerminalState
//! authority. This crate owns a local socket attachment, an atomically
//! committed `DisplayCache`, derived `seyal-render` presentation state, and the
//! host-facing product shell reducer. It is not a second Workspace database.

pub mod presentation;
pub mod shell;
pub mod theme;

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod block_cache;

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
#[cfg(feature = "benchmark-instrumentation")]
#[doc(hidden)]
pub mod pass8_benchmark;

#[cfg(target_os = "macos")]
pub use local::{
    derive_grid_geometry, ClientError, DiscoveryFailure, GridGeometry, InputAdmissionFailure,
    LocalDisplayClient, ResizeFailure,
};

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod ffi;

#[cfg(target_os = "macos")]
#[doc(hidden)]
pub use ffi::{
    seyal_bridge_adopt_handle, seyal_bridge_disconnect_handle, seyal_bridge_ensure_prepared,
    seyal_bridge_frame, seyal_bridge_poll, seyal_bridge_select, test_register_pending_client,
};
