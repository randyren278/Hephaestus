//! Capability-scoped sandboxes and swappable agent runtime contracts.

mod deterministic;
mod error;
mod isolation;
mod provider;
mod runtime;
mod sandbox;
mod spec;
mod supervisor;

pub use deterministic::DeterministicRuntime;
pub use error::RuntimeError;
pub use isolation::{IsolationBackend, IsolationPolicy};
pub use provider::ProviderInvocation;
pub use runtime::{
    AdapterCapabilities, CompletionReason, Provider, RunHandle, RunSnapshot, RunStatus,
    RuntimeAdapter, RuntimeObservation, RuntimeObservationKind,
};
pub use sandbox::{CapabilityToken, Sandbox, SandboxManager};
pub use spec::{Budget, ExperimentContext, RunSpec};
pub use supervisor::SupervisedRuntime;
