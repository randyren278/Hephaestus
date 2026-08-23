use std::{path::PathBuf, time::Duration};

use hephaestus_core::authority::CapabilitySet;

use crate::RuntimeError;

/// Hard per-run resource budget enforced by the supervisor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Budget {
    wall: Duration,
    maximum_output_bytes: usize,
    maximum_cost_microusd: u64,
}

impl Budget {
    /// Creates a validated non-zero execution budget.
    ///
    /// # Errors
    ///
    /// Rejects zero wall time or output capacity.
    pub const fn new(
        wall: Duration,
        maximum_output_bytes: usize,
        maximum_cost_microusd: u64,
    ) -> Result<Self, RuntimeError> {
        if wall.is_zero() {
            return Err(RuntimeError::InvalidSpec("wall budget must be non-zero"));
        }
        if maximum_output_bytes == 0 {
            return Err(RuntimeError::InvalidSpec("output budget must be non-zero"));
        }
        Ok(Self {
            wall,
            maximum_output_bytes,
            maximum_cost_microusd,
        })
    }

    /// Maximum wall-clock duration.
    #[must_use]
    pub const fn wall(self) -> Duration {
        self.wall
    }

    /// Maximum persisted stdout plus stderr bytes.
    #[must_use]
    pub const fn maximum_output_bytes(self) -> usize {
        self.maximum_output_bytes
    }

    /// Maximum provider spend in micro-US dollars.
    #[must_use]
    pub const fn maximum_cost_microusd(self) -> u64 {
        self.maximum_cost_microusd
    }
}

/// Provider-neutral immutable request to execute one Genome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunSpec {
    run_id: String,
    genome_id: String,
    world_id: String,
    source_repository: PathBuf,
    prompt: String,
    capabilities: CapabilitySet,
    budget: Budget,
}

impl RunSpec {
    /// Creates a validated provider-neutral run request.
    ///
    /// # Errors
    ///
    /// Rejects unsafe run identifiers, blank immutable identities or prompts,
    /// and source paths that are not directories.
    pub fn new(
        run_id: impl Into<String>,
        genome_id: impl Into<String>,
        world_id: impl Into<String>,
        source_repository: impl Into<PathBuf>,
        prompt: impl Into<String>,
        capabilities: CapabilitySet,
        budget: Budget,
    ) -> Result<Self, RuntimeError> {
        let run_id = run_id.into();
        if run_id.is_empty()
            || !run_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(RuntimeError::InvalidSpec("run_id is not path safe"));
        }
        let genome_id = genome_id.into();
        let world_id = world_id.into();
        let prompt = prompt.into();
        if genome_id.trim().is_empty() || world_id.trim().is_empty() || prompt.trim().is_empty() {
            return Err(RuntimeError::InvalidSpec(
                "Genome, World, and prompt are required",
            ));
        }
        let source_repository = source_repository.into();
        if !source_repository.is_dir() {
            return Err(RuntimeError::InvalidSpec(
                "source repository is not a directory",
            ));
        }
        Ok(Self {
            run_id,
            genome_id,
            world_id,
            source_repository,
            prompt,
            capabilities,
            budget,
        })
    }

    /// Stable run identity.
    #[must_use]
    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    /// Immutable Genome identity.
    #[must_use]
    pub fn genome_id(&self) -> &str {
        &self.genome_id
    }

    /// Immutable World identity.
    #[must_use]
    pub fn world_id(&self) -> &str {
        &self.world_id
    }

    /// Git repository used to create an isolated worktree.
    #[must_use]
    pub fn source_repository(&self) -> &std::path::Path {
        &self.source_repository
    }

    /// Provider-neutral task prompt.
    #[must_use]
    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    /// Explicit worker capabilities.
    #[must_use]
    pub const fn capabilities(&self) -> CapabilitySet {
        self.capabilities
    }

    /// Hard run budget.
    #[must_use]
    pub const fn budget(&self) -> Budget {
        self.budget
    }
}
