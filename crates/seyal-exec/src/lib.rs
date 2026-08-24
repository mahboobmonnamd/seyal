//! Seyal-owned local terminal execution boundary.
//!
//! `TerminalExecution` owns one PTY endpoint/child lifecycle and one
//! authoritative `seyal_terminal::TerminalState`. The macOS PTY implementation
//! is internal; raw descriptors, GUI state, Blocks and Runtime orchestration do
//! not enter this crate's public surface.

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
pub use endpoint::{ReadOutcome, TerminalEndpoint, WriteOutcome};
pub use error::ExecError;
pub use execution::TerminalExecution;
pub use readiness::Readiness;
pub use winsize::WindowSize;
