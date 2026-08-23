use std::path::PathBuf;

use hephaestus_core::authority::CapabilitySet;

use crate::{CapabilityToken, RunSpec, RuntimeError, Sandbox};

/// Runtime provider selected independently from Genome semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Provider {
    /// Deterministic local reference implementation used by CI and offline tests.
    Deterministic,
    /// `OpenAI` Codex CLI adapter.
    Codex,
    /// Anthropic Claude Code CLI adapter.
    Claude,
}

/// Lifecycle operations and authority dimensions supported by an adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterCapabilities {
    /// Provider sessions can resume from a checkpoint or provider session ID.
    pub resume: bool,
    /// Running work can be interrupted by the supervisor.
    pub interrupt: bool,
    /// Provider emits a durable snapshot path.
    pub snapshot: bool,
    /// Maximum authority this adapter can enforce.
    pub authority: CapabilitySet,
}

/// Stable handle returned after a run starts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunHandle {
    /// Stable `RunSpec` identity.
    pub run_id: String,
    /// Provider executing the run.
    pub provider: Provider,
}

/// Observed supervised lifecycle state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunStatus {
    /// Child process or deterministic task is still active.
    Running,
    /// Task exited successfully inside all budgets.
    Succeeded,
    /// Task exited unsuccessfully.
    Failed,
    /// Operator or scheduler interrupted the task.
    Interrupted,
    /// Supervisor terminated the task at its wall deadline.
    TimedOut,
}

/// Provider-neutral observable snapshot with no private reasoning content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunSnapshot {
    /// Stable run identity.
    pub run_id: String,
    /// Current lifecycle state.
    pub status: RunStatus,
    /// Provider exit code when available.
    pub exit_code: Option<i32>,
    /// Bounded provider stdout artifact.
    pub stdout_path: PathBuf,
    /// Bounded provider stderr artifact.
    pub stderr_path: PathBuf,
    /// Authority actually assigned to the run.
    pub capabilities: CapabilitySet,
}

/// Swappable runtime lifecycle contract used by the daemon scheduler.
pub trait RuntimeAdapter {
    /// Identifies the provider implementation.
    fn provider(&self) -> Provider;

    /// Reports enforceable lifecycle and authority capabilities.
    fn report_capabilities(&self) -> AdapterCapabilities;

    /// Starts one capability-scoped run.
    ///
    /// # Errors
    ///
    /// Fails when authority, sandbox, provider, or process supervision setup fails.
    fn start(
        &mut self,
        spec: &RunSpec,
        sandbox: &Sandbox,
        token: &CapabilityToken,
    ) -> Result<RunHandle, RuntimeError>;

    /// Resumes one provider session in the same sandbox and budgets.
    ///
    /// # Errors
    ///
    /// Fails for unknown runs, invalid capability proofs, or providers without resume.
    fn resume(
        &mut self,
        spec: &RunSpec,
        sandbox: &Sandbox,
        token: &CapabilityToken,
        checkpoint: &str,
    ) -> Result<RunHandle, RuntimeError>;

    /// Interrupts active work and waits for process termination.
    ///
    /// # Errors
    ///
    /// Fails for an unknown run or operating-system termination failure.
    fn interrupt(&mut self, run_id: &str) -> Result<(), RuntimeError>;

    /// Polls budgets and returns an observable snapshot.
    ///
    /// # Errors
    ///
    /// Fails for an unknown run or process/output inspection failure.
    fn snapshot(&mut self, run_id: &str) -> Result<RunSnapshot, RuntimeError>;
}
