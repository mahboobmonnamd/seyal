//! SPEC-004 shared-memory display projection: fixed-width ABI layout,
//! race-safe generation publication and Runtime-owned lifecycle.
//!
//! The Runtime is the projection's sole writer. Client mappings are always
//! read-only. This module never exposes canonical `TerminalState`/grid
//! memory, Rust pointers, or `Vec`/`String` internals across the boundary.

pub mod layout;
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
pub mod lifecycle;
pub mod producer;
#[allow(unsafe_code)]
pub mod writer;
