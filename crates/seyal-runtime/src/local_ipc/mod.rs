//! SPEC-004 Candidate-D local attachment protocol: binary framing, connection
//! state, peer authentication, attachment authority and display-state delivery.
//!
//! This module never owns a PTY, VT parser, terminal grid or canonical terminal
//! memory. Normal presentation is snapshot/delta over the Runtime UDS; legacy
//! shared-projection code is isolated outside this production control boundary.

pub mod attachment;
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
pub mod auth;
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
pub mod connection;
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
pub mod discovery;
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
pub mod fd_transfer;
pub mod framing;
