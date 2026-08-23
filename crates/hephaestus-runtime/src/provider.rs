use std::path::PathBuf;

use crate::{Provider, RunSpec, RuntimeError, Sandbox};

/// Immutable, inspectable provider process contract. The prompt is supplied on stdin.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderInvocation {
    provider: Provider,
    program: PathBuf,
    arguments: Vec<String>,
    stdin: Vec<u8>,
}

impl ProviderInvocation {
    /// Builds the current non-interactive Codex CLI contract without invoking it.
    ///
    /// # Errors
    ///
    /// Rejects an empty executable path or a sandbox that cannot enforce the `RunSpec`.
    pub fn codex(
        executable: impl Into<PathBuf>,
        spec: &RunSpec,
        sandbox: &Sandbox,
    ) -> Result<Self, RuntimeError> {
        let executable = executable.into();
        validate_executable(&executable)?;
        validate_sandbox_authority(spec, sandbox)?;
        let network = spec.capabilities().allows_network();
        Ok(Self {
            provider: Provider::Codex,
            program: executable,
            arguments: vec![
                "exec".to_owned(),
                "--ignore-user-config".to_owned(),
                "--ephemeral".to_owned(),
                "--json".to_owned(),
                "--sandbox".to_owned(),
                if spec.capabilities().allows_workspace_write() {
                    "workspace-write".to_owned()
                } else {
                    "read-only".to_owned()
                },
                "--cd".to_owned(),
                sandbox.worktree().to_string_lossy().into_owned(),
                "--config".to_owned(),
                format!("sandbox_workspace_write.network_access={network}"),
                "-".to_owned(),
            ],
            stdin: spec.prompt().as_bytes().to_vec(),
        })
    }

    /// Builds the current non-interactive Claude Code CLI contract without invoking it.
    ///
    /// # Errors
    ///
    /// Rejects an empty executable path or a sandbox that cannot enforce the `RunSpec`.
    pub fn claude(
        executable: impl Into<PathBuf>,
        spec: &RunSpec,
        sandbox: &Sandbox,
    ) -> Result<Self, RuntimeError> {
        let executable = executable.into();
        validate_executable(&executable)?;
        validate_sandbox_authority(spec, sandbox)?;
        let maximum_microusd = spec.budget().maximum_cost_microusd();
        let maximum_dollars = maximum_microusd / 1_000_000;
        let remaining_microusd = maximum_microusd % 1_000_000;
        Ok(Self {
            provider: Provider::Claude,
            program: executable,
            arguments: vec![
                "--print".to_owned(),
                "--output-format".to_owned(),
                "stream-json".to_owned(),
                "--no-session-persistence".to_owned(),
                "--safe-mode".to_owned(),
                "--permission-mode".to_owned(),
                "dontAsk".to_owned(),
                "--tools".to_owned(),
                if spec.capabilities().allows_workspace_write() {
                    "Read,Edit,Write".to_owned()
                } else {
                    "Read".to_owned()
                },
                "--max-budget-usd".to_owned(),
                format!("{maximum_dollars}.{remaining_microusd:06}"),
            ],
            stdin: spec.prompt().as_bytes().to_vec(),
        })
    }

    /// Provider represented by this invocation.
    #[must_use]
    pub const fn provider(&self) -> Provider {
        self.provider
    }

    /// Absolute or PATH-resolved provider executable.
    #[must_use]
    pub fn program(&self) -> &std::path::Path {
        &self.program
    }

    /// Exact non-secret provider arguments.
    #[must_use]
    pub fn arguments(&self) -> &[String] {
        &self.arguments
    }

    /// Prompt bytes written to child stdin rather than exposed in the process list.
    #[must_use]
    pub fn stdin(&self) -> &[u8] {
        &self.stdin
    }
}

fn validate_executable(path: &std::path::Path) -> Result<(), RuntimeError> {
    if path.as_os_str().is_empty() {
        return Err(RuntimeError::InvalidSpec("provider executable is empty"));
    }
    Ok(())
}

fn validate_sandbox_authority(spec: &RunSpec, sandbox: &Sandbox) -> Result<(), RuntimeError> {
    if sandbox.capabilities() != spec.capabilities() {
        return Err(RuntimeError::CapabilityDenied);
    }
    Ok(())
}
