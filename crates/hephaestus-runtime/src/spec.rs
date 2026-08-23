use std::{path::PathBuf, process::Command, time::Duration};

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
    source_revision: String,
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
        Self::new_at_revision(
            run_id,
            genome_id,
            world_id,
            source_repository,
            "HEAD",
            prompt,
            capabilities,
            budget,
        )
    }

    /// Creates a run request pinned to the commit resolved from `source_revision`.
    ///
    /// The revision is resolved during construction, so later branch or `HEAD`
    /// movement cannot change the source paired with this request.
    ///
    /// # Errors
    ///
    /// In addition to [`Self::new`] validation, rejects revisions that Git cannot
    /// resolve to an immutable commit in the source repository.
    #[allow(clippy::too_many_arguments)]
    pub fn new_at_revision(
        run_id: impl Into<String>,
        genome_id: impl Into<String>,
        world_id: impl Into<String>,
        source_repository: impl Into<PathBuf>,
        source_revision: impl AsRef<str>,
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
        let source_revision = resolve_commit(&source_repository, source_revision.as_ref())?;
        Ok(Self {
            run_id,
            genome_id,
            world_id,
            source_repository,
            source_revision,
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

    /// Exact Git commit paired with this run.
    #[must_use]
    pub fn source_revision(&self) -> &str {
        &self.source_revision
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

fn resolve_commit(repository: &std::path::Path, revision: &str) -> Result<String, RuntimeError> {
    if revision.trim().is_empty() {
        return Err(RuntimeError::InvalidSpec("source revision is required"));
    }
    let commit_expression = format!("{revision}^{{commit}}");
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(["rev-parse", "--verify", "--end-of-options"])
        .arg(commit_expression)
        .output()?;
    if !output.status.success() {
        return Err(RuntimeError::Git(
            "source revision is not a commit".to_owned(),
        ));
    }
    let commit = std::str::from_utf8(&output.stdout)
        .map_err(|_| RuntimeError::Git("resolved source revision is not UTF-8".to_owned()))?
        .trim();
    if !matches!(commit.len(), 40 | 64) || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(RuntimeError::Git(
            "resolved source revision is not an object ID".to_owned(),
        ));
    }
    Ok(commit.to_ascii_lowercase())
}
