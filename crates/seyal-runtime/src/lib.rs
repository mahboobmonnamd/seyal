//! Seyal's M001 headless composition and lifecycle authority.
//!
//! The Runtime owns identities, registry, Workspace association, logical
//! attachments, bounded admission and scheduling. Every `TerminalExecution`
//! continues to own its PTY, primary child and sole canonical TerminalState.

mod capability;
mod error;
mod ids;
mod input;
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod platform;
mod runtime;
mod singleton;

pub use capability::{CapabilityPolicy, m001_term_name};
pub use error::RuntimeError;
pub use ids::{AttachmentId, ExecutionId, RuntimeId, WorkspaceId};
pub use input::InputIngress;
pub use runtime::{ExecutionLifecycle, ExecutionSummary, Runtime, RuntimeConfig};
