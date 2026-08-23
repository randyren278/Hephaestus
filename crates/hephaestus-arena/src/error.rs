use std::{error::Error, fmt};

use hephaestus_ledger::LedgerError;

/// Fail-closed evaluation and persistence errors.
#[derive(Debug)]
pub enum ArenaError {
    /// An identifier is empty, malformed, or non-canonical.
    InvalidId { field: &'static str, value: String },
    /// A task identifier occurs more than once.
    DuplicateTaskId(String),
    /// A manifest has no tasks.
    EmptyManifest,
    /// A manifest exceeds the receipt counter range.
    TooManyTasks,
    /// The visible and sealed manifests use the same identity.
    DuplicateManifestId(String),
    /// Parent and candidate submissions use the same identity.
    DuplicateSubmissionId(String),
    /// Provenance differs between an input and the evaluator binding.
    BindingMismatch(&'static str),
    /// A manifest was supplied in the wrong visibility slot.
    VisibilityMismatch,
    /// A submission is missing or adds task identifiers.
    TaskSetMismatch { submission_id: String },
    /// Durable evidence storage failed.
    Ledger(LedgerError),
    /// Canonical JSON encoding failed.
    Serialization(serde_json::Error),
}

impl fmt::Display for ArenaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl Error for ArenaError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Ledger(error) => Some(error),
            Self::Serialization(error) => Some(error),
            _ => None,
        }
    }
}

impl From<LedgerError> for ArenaError {
    fn from(error: LedgerError) -> Self {
        Self::Ledger(error)
    }
}

impl From<serde_json::Error> for ArenaError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialization(error)
    }
}
