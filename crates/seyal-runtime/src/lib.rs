//! Seyal's M001 headless composition and lifecycle authority.
//!
//! The Runtime owns identities, registry, Workspace association, logical
//! attachments, bounded admission and scheduling. Every `TerminalExecution`
//! continues to own its PTY, primary child and sole canonical TerminalState.

mod activity_block_timeline;
mod capability;
#[cfg(target_os = "macos")]
mod command_block_timeline;
pub mod display;
mod error;
mod ids;
mod input;
pub mod local_ipc;
#[cfg(all(target_os = "macos", feature = "benchmark-instrumentation"))]
#[doc(hidden)]
pub mod pass7_benchmark;
#[cfg(feature = "benchmark-instrumentation")]
#[doc(hidden)]
pub mod pass8_benchmark;
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

pub use activity_block_timeline::{BlockLifecycle, BlockSummary};
pub use capability::{CapabilityPolicy, m001_term_name};
pub use error::RuntimeError;
pub use ids::{AttachmentId, BlockId, ExecutionId, ProjectionId, RuntimeId, WorkspaceId};
pub use input::InputIngress;
#[cfg(feature = "benchmark-instrumentation")]
#[doc(hidden)]
pub use runtime::BenchmarkRuntimeDiagnostics;
pub use runtime::{ExecutionLifecycle, ExecutionSummary, LocalIpcMode, Runtime, RuntimeConfig};
