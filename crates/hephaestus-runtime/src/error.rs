use std::{error::Error, fmt, io};

/// Sandbox, budget, provider, and supervised-process failures.
#[derive(Debug)]
pub enum RuntimeError {
    /// The operating system rejected an isolation or process operation.
    Io(io::Error),
    /// A stable identifier or budget was invalid.
    InvalidSpec(&'static str),
    /// Git could not create or remove an isolated worktree.
    Git(String),
    /// Sandbox cleanup attempted every step but one or more steps failed.
    CleanupFailed {
        git_failed: bool,
        filesystem_failed: bool,
    },
    /// Sandbox creation failed and its compensating cleanup was incomplete.
    RollbackFailed {
        operation: Box<Self>,
        git_failed: bool,
        filesystem_failed: bool,
    },
    /// A capability token was missing, expired, or did not match the run.
    CapabilityDenied,
    /// A requested provider or lifecycle action is unsupported.
    Unsupported(&'static str),
    /// Required runtime evidence could not be persisted.
    Evidence(String),
    /// Evidence failed and the runtime could not confirm containment.
    ContainmentFailed { evidence: String, interrupt: String },
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl Error for RuntimeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::RollbackFailed { operation, .. } => Some(operation),
            Self::InvalidSpec(_)
            | Self::Git(_)
            | Self::CleanupFailed { .. }
            | Self::CapabilityDenied
            | Self::Unsupported(_)
            | Self::Evidence(_)
            | Self::ContainmentFailed { .. } => None,
        }
    }
}

impl From<io::Error> for RuntimeError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
