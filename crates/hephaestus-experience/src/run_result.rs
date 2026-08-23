use hephaestus_ledger::{ArtifactId, EventInput, StoredEvent};
use hephaestus_runtime::CompletionReason;
use serde::{Deserialize, Serialize};

use crate::ExperienceError;

/// Current schema for canonical runtime result receipts.
pub const RUN_RESULT_SCHEMA_VERSION: u16 = 1;
const RUN_RESULT_EVENT_TYPE: &str = "run.result_recorded";
const RUNTIME_ACTOR: &str = "runtime-plane";
const MAX_RUN_ID_BYTES: usize = 128;
const MAX_TRACE_ARTIFACTS: usize = 1_024;
const MAX_DETERMINISTIC_LATENCY_MILLIS: u64 = 10_000;
const MAX_RUN_RESULT_PAYLOAD_BYTES: usize = 131_072;

/// Stable terminal reasons stored in canonical history and exposed by the API.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunCompletionReason {
    /// The reference runtime completed successfully.
    Success,
    /// The provider reported an ordinary failure.
    ProviderFailure,
    /// The operator interrupted execution.
    OperatorInterrupt,
    /// The wall-clock budget expired.
    WallBudgetExceeded,
    /// The output byte budget was exceeded.
    OutputBudgetExceeded,
    /// Provider input or output could not be delivered durably.
    IoFailure,
}

impl From<CompletionReason> for RunCompletionReason {
    fn from(reason: CompletionReason) -> Self {
        match reason {
            CompletionReason::Success => Self::Success,
            CompletionReason::ProviderFailure => Self::ProviderFailure,
            CompletionReason::OperatorInterrupt => Self::OperatorInterrupt,
            CompletionReason::WallBudgetExceeded => Self::WallBudgetExceeded,
            CompletionReason::OutputBudgetExceeded => Self::OutputBudgetExceeded,
            CompletionReason::IoFailure => Self::IoFailure,
        }
    }
}

/// Versioned, self-validating receipt for one deterministic reference run.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunResultReceipt {
    /// Receipt schema version.
    pub schema_version: u16,
    /// Stable path-safe run identity.
    pub run_id: String,
    /// Immutable Genome identity executed by the runtime.
    pub genome_id: String,
    /// Immutable World identity governing the run.
    pub world_id: String,
    /// Exact Git object ID inventoried by the isolated runtime.
    pub source_revision: String,
    /// Runtime-owned terminal reason.
    pub completion_reason: RunCompletionReason,
    /// Runtime-owned terminal latency.
    pub latency_millis: u64,
    /// Exact deterministic provider cost in micro-US dollars.
    pub actual_cost_microusd: u64,
    /// CAS address of bounded standard output.
    pub stdout_artifact_id: String,
    /// CAS address of bounded diagnostic output.
    pub stderr_artifact_id: String,
    /// CAS addresses of redacted trace artifacts.
    pub trace_artifact_ids: Vec<String>,
}

impl RunResultReceipt {
    /// Builds a validated receipt for the offline deterministic runtime.
    ///
    /// # Errors
    ///
    /// Rejects malformed identities, non-zero cost, excessive latency or trace
    /// cardinality, unsupported terminal reasons, and malformed artifact IDs.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        run_id: impl Into<String>,
        genome_id: impl Into<String>,
        world_id: impl Into<String>,
        source_revision: impl Into<String>,
        completion_reason: RunCompletionReason,
        latency_millis: u64,
        actual_cost_microusd: u64,
        stdout_artifact_id: impl Into<String>,
        stderr_artifact_id: impl Into<String>,
        trace_artifact_ids: Vec<String>,
    ) -> Result<Self, ExperienceError> {
        let receipt = Self {
            schema_version: RUN_RESULT_SCHEMA_VERSION,
            run_id: run_id.into(),
            genome_id: genome_id.into(),
            world_id: world_id.into(),
            source_revision: source_revision.into(),
            completion_reason,
            latency_millis,
            actual_cost_microusd,
            stdout_artifact_id: stdout_artifact_id.into(),
            stderr_artifact_id: stderr_artifact_id.into(),
            trace_artifact_ids,
        };
        receipt.validate()?;
        Ok(receipt)
    }

    /// Parses and verifies a receipt directly from one canonical stored event.
    ///
    /// # Errors
    ///
    /// Rejects malformed payloads and events whose envelope does not match the
    /// runtime-owned type, actor, or IDs derived from the receipt's run ID.
    pub fn parse_from_event(event: &StoredEvent) -> Result<Self, ExperienceError> {
        if event.payload.len() > MAX_RUN_RESULT_PAYLOAD_BYTES {
            return Err(ExperienceError::RecordTooLarge {
                actual: event.payload.len(),
                maximum: MAX_RUN_RESULT_PAYLOAD_BYTES,
            });
        }
        let receipt: Self = serde_json::from_slice(&event.payload)?;
        receipt.validate()?;
        if event.event_type != RUN_RESULT_EVENT_TYPE
            || event.actor != RUNTIME_ACTOR
            || event.event_id != receipt.event_id()
            || event.aggregate_id != receipt.aggregate_id()
        {
            return Err(ExperienceError::InvalidInput(
                "run result crossed its provenance boundary",
            ));
        }
        Ok(receipt)
    }

    /// Creates the only canonical ledger envelope valid for this receipt.
    ///
    /// # Errors
    ///
    /// Returns a serialization error if canonical JSON encoding fails.
    pub fn into_event_input(self, timestamp_millis: i64) -> Result<EventInput, ExperienceError> {
        self.validate()?;
        let event_id = self.event_id();
        let aggregate_id = self.aggregate_id();
        let payload = serde_json::to_vec(&self)?;
        Ok(EventInput::new(
            event_id,
            aggregate_id,
            RUN_RESULT_EVENT_TYPE,
            RUNTIME_ACTOR,
            timestamp_millis,
            payload,
        ))
    }

    /// Canonical event identifier derived from the run identity.
    #[must_use]
    pub fn event_id(&self) -> String {
        format!("result:{}", self.run_id)
    }

    /// Canonical run aggregate identifier.
    #[must_use]
    pub fn aggregate_id(&self) -> String {
        format!("run:{}", self.run_id)
    }

    fn validate(&self) -> Result<(), ExperienceError> {
        if self.schema_version != RUN_RESULT_SCHEMA_VERSION {
            return Err(ExperienceError::InvalidInput(
                "unsupported run result schema version",
            ));
        }
        validate_run_id(&self.run_id)?;
        validate_content_id(&self.genome_id, "genome")?;
        validate_content_id(&self.world_id, "world")?;
        validate_source_revision(&self.source_revision)?;
        if self.actual_cost_microusd != 0 {
            return Err(ExperienceError::InvalidInput(
                "deterministic run result cost must be zero",
            ));
        }
        if self.latency_millis > MAX_DETERMINISTIC_LATENCY_MILLIS {
            return Err(ExperienceError::InvalidInput(
                "deterministic run result latency exceeds its bound",
            ));
        }
        if !matches!(
            self.completion_reason,
            RunCompletionReason::Success | RunCompletionReason::OutputBudgetExceeded
        ) {
            return Err(ExperienceError::InvalidInput(
                "deterministic run result has an unsupported completion reason",
            ));
        }
        if self.trace_artifact_ids.len() > MAX_TRACE_ARTIFACTS {
            return Err(ExperienceError::InvalidInput(
                "run result contains too many trace artifacts",
            ));
        }
        validate_artifact_id(&self.stdout_artifact_id)?;
        validate_artifact_id(&self.stderr_artifact_id)?;
        for artifact_id in &self.trace_artifact_ids {
            validate_artifact_id(artifact_id)?;
        }
        Ok(())
    }
}

fn validate_run_id(run_id: &str) -> Result<(), ExperienceError> {
    if run_id.is_empty()
        || run_id.len() > MAX_RUN_ID_BYTES
        || !run_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(ExperienceError::InvalidInput(
            "run result run_id is not path safe",
        ));
    }
    Ok(())
}

fn validate_content_id(value: &str, namespace: &'static str) -> Result<(), ExperienceError> {
    let prefix = format!("hephaestus:{namespace}:");
    let hash = value
        .strip_prefix(&prefix)
        .ok_or(ExperienceError::InvalidInput(
            "run result content identity is malformed",
        ))?;
    validate_artifact_id(hash)
}

fn validate_source_revision(revision: &str) -> Result<(), ExperienceError> {
    if !matches!(revision.len(), 40 | 64)
        || !revision
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ExperienceError::InvalidInput(
            "run result source revision is not a canonical object ID",
        ));
    }
    Ok(())
}

fn validate_artifact_id(value: &str) -> Result<(), ExperienceError> {
    ArtifactId::parse(value.to_owned())?;
    Ok(())
}
