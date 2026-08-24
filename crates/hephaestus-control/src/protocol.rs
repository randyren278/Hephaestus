pub use hephaestus_experience::RunCompletionReason;
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
    /// Execute the offline deterministic reference runtime for one registered Genome.
    RunReference {
        /// Content-derived registered Genome identity.
        genome_id: String,
    },
    /// Execute one World-bound task with daemon-owned runtime provenance.
    RunEvaluation {
        /// Content-derived registered Genome identity.
        genome_id: String,
        /// Exact World task identity.
        task_id: String,
        /// Exact provider input committed by the runtime.
        input: String,
        /// Deterministic evaluation seed.
        seed: u64,
        /// Hard wall deadline in milliseconds.
        wall_millis: u64,
        /// Maximum combined output bytes.
        maximum_output_bytes: u64,
        /// Maximum provider spend in micro-US dollars.
        maximum_cost_microusd: u64,
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
    /// Terminal result from the offline deterministic reference runtime.
    Run {
        /// Stable run identity.
        run_id: String,
        /// Immutable Genome identity executed by the runtime.
        genome_id: String,
        /// Immutable registered World identity governing the run.
        world_id: String,
        /// Exact Git commit inventoried by the isolated runtime.
        source_revision: String,
        /// Exact runtime-owned completion reason.
        completion_reason: RunCompletionReason,
        /// Runtime-owned terminal latency.
        latency_millis: u64,
        /// Exact deterministic provider cost in micro-US dollars.
        actual_cost_microusd: u64,
        /// CAS address of the bounded inventory output.
        stdout_artifact_id: String,
        /// CAS address of the bounded diagnostic output.
        stderr_artifact_id: String,
        /// CAS addresses of the redacted trace artifacts.
        trace_artifact_ids: Vec<String>,
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

/// Stable public metadata for an immutable World ledger record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorldRecord {
    /// Content-derived World identity.
    pub world_id: String,
    /// Stable display name.
    pub name: String,
    /// CAS address of canonical World JSON.
    pub artifact_id: String,
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
