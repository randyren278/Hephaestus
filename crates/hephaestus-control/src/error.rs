use std::{error::Error, fmt, io};

use hephaestus_ledger::LedgerError;

/// Local control-plane startup, transport, persistence, and replay failures.
#[derive(Debug)]
pub enum ControlError {
    /// The operating system rejected a local control-plane operation.
    Io(io::Error),
    /// Canonical ledger verification or append failed.
    Ledger(LedgerError),
    /// A local protocol or canonical projection document was invalid.
    Json(serde_json::Error),
    /// Another process already owns the canonical data directory.
    AlreadyRunning,
    /// A bounded local protocol requirement was violated.
    Protocol(&'static str),
    /// Canonical events could not reproduce the live projection.
    Projection(String),
}

impl fmt::Display for ControlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl Error for ControlError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Ledger(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::AlreadyRunning | Self::Protocol(_) | Self::Projection(_) => None,
        }
    }
}

impl From<io::Error> for ControlError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<LedgerError> for ControlError {
    fn from(error: LedgerError) -> Self {
        Self::Ledger(error)
    }
}

impl From<serde_json::Error> for ControlError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}
