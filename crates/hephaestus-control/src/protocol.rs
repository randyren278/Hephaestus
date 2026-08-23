use serde::{Deserialize, Serialize};

/// The only local operator API version accepted by this release.
pub const API_VERSION: u16 = 1;

/// One authenticated request over the owner-only local socket.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApiRequest {
    /// Protocol version.
    pub version: u16,
    /// Caller-generated correlation identifier.
    pub request_id: String,
    /// Secret read from the owner-only daemon token file.
    pub token: String,
    /// Typed operator command.
    pub command: Command,
}

/// Operator actions implemented by the first control plane.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    /// Inspect the current canonical projection.
    Status,
    /// Stop new evolution work.
    Freeze,
    /// Resume evolution through the external operator boundary.
    Unfreeze,
    /// Terminate all active work represented by canonical events.
    KillAll,
    /// Inspect one immutable Genome record.
    GenomeShow {
        /// Content-derived Genome identity.
        genome_id: String,
    },
    /// Verify and replay canonical history into a fresh projection.
    Replay,
    /// Stop the local daemon after acknowledging the audited request.
    DaemonStop,
}

/// Stable machine-readable local API response.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApiResponse {
    /// Protocol version emitted by the daemon.
    pub version: u16,
    /// Correlation identifier copied from the request when available.
    pub request_id: String,
    /// Successful response body.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<ResponseData>,
    /// Stable failure body with no internal error disclosure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError>,
}

impl ApiResponse {
    pub(crate) fn success(request_id: String, data: ResponseData) -> Self {
        Self {
            version: API_VERSION,
            request_id,
            data: Some(data),
            error: None,
        }
    }

    pub(crate) fn failure(
        request_id: impl Into<String>,
        code: ApiErrorCode,
        message: impl Into<String>,
    ) -> Self {
        Self {
            version: API_VERSION,
            request_id: request_id.into(),
            data: None,
            error: Some(ApiError {
                code,
                message: message.into(),
            }),
        }
    }
}

/// Successful typed response data.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ResponseData {
    /// Current desired and observed control state.
    Status {
        /// Whether evolution is frozen.
        frozen: bool,
        /// Number of canonically active runs.
        active_runs: usize,
        /// Number of canonical events after auditing this request.
        event_count: u64,
        /// Number of registered immutable Genomes.
        genome_count: usize,
    },
    /// Deterministic acknowledgement of a consequential operator action.
    Acknowledged {
        /// Freeze state after the action.
        frozen: bool,
        /// Runs terminated by this action.
        killed_runs: usize,
    },
    /// One registered immutable Genome.
    Genome {
        /// Canonical projection record.
        genome: GenomeRecord,
    },
    /// Result of a fresh verified replay.
    Replay {
        /// Number of verified canonical events.
        event_count: u64,
        /// Freeze state reconstructed from history.
        frozen: bool,
        /// Active run count reconstructed from history.
        active_runs: usize,
        /// Stable BLAKE3 hash of the reconstructed projection.
        projection_hash: String,
    },
}

/// Stable public metadata for an immutable Genome ledger record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenomeRecord {
    /// Content-derived Genome identity.
    pub genome_id: String,
    /// Stable display name.
    pub name: String,
    /// World identity under which this Genome was compiled.
    pub world_id: String,
    /// CAS address of canonical Genome JSON.
    pub artifact_id: String,
    /// Content-derived declared parents.
    pub parent_ids: Vec<String>,
}

/// Safe local API failure body.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApiError {
    /// Stable error category.
    pub code: ApiErrorCode,
    /// Human-readable message without internal storage detail.
    pub message: String,
}

/// Stable fail-closed local API categories.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiErrorCode {
    /// The request used an unsupported protocol version.
    UnsupportedVersion,
    /// Authentication failed.
    Unauthorized,
    /// A required identifier was empty or absent.
    InvalidRequest,
    /// The requested canonical record does not exist.
    NotFound,
    /// Canonical persistence or projection verification failed.
    Internal,
}
