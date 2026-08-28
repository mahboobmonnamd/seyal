//! Seyal-owned local terminal execution boundary.
//!
//! `TerminalExecution` owns one internal PTY endpoint/child lifecycle and one
//! authoritative `seyal_terminal::TerminalState`. `ExecutionReactor` composes
//! readiness only: it never owns or duplicates those execution resources.

mod child;
mod command;
mod endpoint;
mod error;
mod execution;
mod platform;
mod projection;
mod reactor;
mod readiness;
#[cfg(all(target_os = "macos", feature = "test-fault-injection"))]
#[doc(hidden)]
pub mod test_fault;
mod winsize;

pub use child::{ChildExit, SignalDisposition, TerminationPolicy};
pub use command::CommandSpec;
pub use endpoint::{ReadOutcome, WriteOutcome};
pub use error::ExecError;
pub use execution::TerminalExecution;
pub use projection::{
    ProjectionAttributes, ProjectionCell, ProjectionColor, ProjectionDamage,
    TerminalProjectionSnapshot, TerminalProjectionUpdate,
};
pub use reactor::{
    ExecutionReactor, ReactorEvent, ReactorEventKind, ReactorWaker, RegistrationToken,
};
pub use readiness::Readiness;
pub use winsize::WindowSize;
