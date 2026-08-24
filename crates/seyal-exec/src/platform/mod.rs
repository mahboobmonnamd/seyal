//! Internal operating-system seam for terminal execution.
//!
//! Do not turn this into a generic cross-platform framework. Add only the
//! platform operations required by the active implementation milestone.

#[cfg(target_os = "macos")]
mod macos;
