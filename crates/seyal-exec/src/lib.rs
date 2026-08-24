//! Seyal terminal-execution ownership boundary.
//!
//! Issue #28 creates this crate before PTY behavior is implemented so the
//! permanent dependency and module boundaries exist first. The scaffold does
//! not expose placeholder endpoint/process APIs. Each module gains public
//! surface only when its behavior is specified, tested and implemented.

mod child;
mod endpoint;
mod execution;
mod platform;
mod readiness;
mod winsize;
