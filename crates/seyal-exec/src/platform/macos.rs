//! macOS PTY/process implementation seam.
//!
//! Syscall/library choices are deliberately deferred until the RILL behavior
//! review and current macOS/POSIX API review are complete. This file must not
//! become a second terminal-state or event-loop implementation.
