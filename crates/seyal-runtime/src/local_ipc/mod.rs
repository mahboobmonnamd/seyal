//! SPEC-004 local attachment protocol: binary framing, connection state
//! machine, peer authentication, attachment/authority and Runtime discovery.
//!
//! This module never owns a PTY, VT parser, terminal grid or canonical
//! terminal memory; it is strictly the Runtime-side control-plane boundary
//! described by `docs/specs/SPEC-004-M001-LOCAL-ATTACHMENT-PROJECTION.md`.

pub mod framing;
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
pub mod auth;
pub mod attachment;
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
pub mod discovery;
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
pub mod fd_transfer;
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod kq;
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
pub mod connection;
