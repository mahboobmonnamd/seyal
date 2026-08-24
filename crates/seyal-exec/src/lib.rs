//! Seyal-owned local terminal execution boundary.
//!
//! `TerminalExecution` owns one internal PTY endpoint/child lifecycle and one
//! authoritative `seyal_terminal::TerminalState`. The endpoint itself is not a
//! public construction surface: callers cannot create an untracked PTY without
//! the corresponding terminal state.

mod child;
mod command;
mod endpoint;
mod error;
mod execution;
mod platform;
mod readiness;
mod winsize;

pub use child::{ChildExit, TerminationPolicy};
pub use command::CommandSpec;
pub use endpoint::{ReadOutcome, WriteOutcome};
pub use error::ExecError;
pub use execution::TerminalExecution;
pub use readiness::Readiness;
pub use winsize::WindowSize;
