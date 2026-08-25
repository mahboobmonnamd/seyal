//! Seyal's M001 headless composition and lifecycle authority.
//!
//! The Runtime owns identities, registry, Workspace association, logical
//! attachments, bounded admission and scheduling. Every `TerminalExecution`
//! continues to own its PTY, primary child and sole canonical TerminalState.

mod capability;
pub mod display;
mod error;
mod ids;
mod input;
pub mod local_ipc;
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod platform;
/// Legacy Candidate-B shared-projection machinery retained only for isolated
/// comparator/reference evidence. It is absent from normal production builds.
#[cfg(feature = "benchmark-shared-projection")]
pub mod projection;
mod runtime;
mod singleton;
#[cfg(all(target_os = "macos", feature = "test-fault-injection"))]
#[doc(hidden)]
pub mod test_fault;

pub use capability::{CapabilityPolicy, m001_term_name};
pub use error::RuntimeError;
pub use ids::{AttachmentId, ExecutionId, ProjectionId, RuntimeId, WorkspaceId};
pub use input::InputIngress;
pub use runtime::{ExecutionLifecycle, ExecutionSummary, LocalIpcMode, Runtime, RuntimeConfig};