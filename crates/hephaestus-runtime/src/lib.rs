//! Capability-scoped sandboxes and swappable agent runtime contracts.

mod deterministic;
mod error;
mod provider;
mod runtime;
mod sandbox;
mod spec;

pub use deterministic::DeterministicRuntime;
pub use error::RuntimeError;
pub use provider::ProviderInvocation;
pub use runtime::{
    AdapterCapabilities, Provider, RunHandle, RunSnapshot, RunStatus, RuntimeAdapter,
};
pub use sandbox::{CapabilityToken, Sandbox, SandboxManager};
pub use spec::{Budget, RunSpec};
