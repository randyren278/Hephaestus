//! Trusted deterministic evaluation with sealed task boundaries and durable receipts.

mod error;

use std::collections::{BTreeMap, BTreeSet};

pub use error::ArenaError;
use hephaestus_ledger::{ArtifactId, ArtifactStore, EventInput, EventStore, StoredEvent};
use serde::{Deserialize, Serialize};

const EVENT_TYPE: &str = "evaluation.recorded";

/// Exact environment and evaluator provenance shared by every evaluation input.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EvaluationBinding {
    /// Immutable compiled World identity.
    world_id: String,
    /// Explicit deterministic seed.
    seed: u64,
    /// Immutable execution-environment identity.
    environment_id: String,
    /// Immutable evaluator identity.
    evaluator_id: String,
}

impl EvaluationBinding {
    /// Creates and validates an evaluation binding.
    ///
    /// # Errors
    ///
    /// Rejects malformed World, environment, or evaluator identities.
    pub fn new(
        world_id: impl Into<String>,
        seed: u64,
        environment_id: impl Into<String>,
        evaluator_id: impl Into<String>,
    ) -> Result<Self, ArenaError> {
        let binding = Self {
            world_id: world_id.into(),
            seed,
            environment_id: environment_id.into(),
            evaluator_id: evaluator_id.into(),
        };
        validate_world_id(&binding.world_id)?;
        validate_id("environment_id", &binding.environment_id)?;
        validate_id("evaluator_id", &binding.evaluator_id)?;
        Ok(binding)
    }

    /// Returns the immutable compiled World identity.
    #[must_use]
    pub fn world_id(&self) -> &str {
        &self.world_id
    }

    /// Returns the explicit deterministic seed.
    #[must_use]
    pub const fn seed(&self) -> u64 {
        self.seed
    }

    /// Returns the immutable execution-environment identity.
    #[must_use]
    pub fn environment_id(&self) -> &str {
        &self.environment_id
    }

    /// Returns the immutable evaluator identity.
    #[must_use]
    pub fn evaluator_id(&self) -> &str {
        &self.evaluator_id
    }
}

/// Whether a trusted task manifest may be shown to a candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    /// Inputs are available before evaluation.
    Visible,
    /// Inputs and expectations remain evaluator-only.
    Sealed,
}

/// Candidate-safe task input. It contains no expected output.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CandidateTask {
    /// Stable task identity.
    pub task_id: String,
    /// Input presented to the candidate.
    pub input: String,
}

/// Evaluator-owned task definition. Expected outputs have no public accessor.
#[derive(Clone, Eq, PartialEq, Serialize)]
pub struct TrustedTask {
    task_id: String,
    input: String,
    expected_output: String,
}

impl TrustedTask {
    /// Creates an evaluator-only task definition.
    ///
    /// # Errors
    ///
    /// Rejects malformed task identifiers.
    pub fn new(
        task_id: impl Into<String>,
        input: impl Into<String>,
        expected_output: impl Into<String>,
    ) -> Result<Self, ArenaError> {
        let task_id = task_id.into();
        validate_id("task_id", &task_id)?;
        Ok(Self {
            task_id,
            input: input.into(),
            expected_output: expected_output.into(),
        })
    }
}

/// Evaluator-owned immutable task manifest.
#[derive(Clone, Eq, PartialEq, Serialize)]
pub struct TrustedManifest {
    schema_version: u16,
    manifest_id: String,
    binding: EvaluationBinding,
    visibility: Visibility,
    tasks: Vec<TrustedTask>,
}

impl TrustedManifest {
    /// Creates a canonical manifest after sorting tasks by identity.
    ///
    /// # Errors
    ///
    /// Rejects malformed or duplicate task identities and empty manifests.
    pub fn new(
        manifest_id: impl Into<String>,
        binding: EvaluationBinding,
        visibility: Visibility,
        mut tasks: Vec<TrustedTask>,
    ) -> Result<Self, ArenaError> {
        let manifest_id = manifest_id.into();
        validate_id("manifest_id", &manifest_id)?;
        if tasks.is_empty() {
            return Err(ArenaError::EmptyManifest);
        }
        tasks.sort_by(|left, right| left.task_id.cmp(&right.task_id));
        for pair in tasks.windows(2) {
            if pair[0].task_id == pair[1].task_id {
                return Err(ArenaError::DuplicateTaskId(pair[0].task_id.clone()));
            }
        }
        Ok(Self {
            schema_version: 1,
            manifest_id,
            binding,
            visibility,
            tasks,
        })
    }

    /// Returns candidate-safe tasks only for a visible manifest.
    ///
    /// # Errors
    ///
    /// A sealed manifest never releases its inputs through this API.
    pub fn candidate_tasks(&self) -> Result<Vec<CandidateTask>, ArenaError> {
        if self.visibility != Visibility::Visible {
            return Err(ArenaError::VisibilityMismatch);
        }
        Ok(self
            .tasks
            .iter()
            .map(|task| CandidateTask {
                task_id: task.task_id.clone(),
                input: task.input.clone(),
            })
            .collect())
    }
}

/// One candidate's opaque answers bound to an exact evaluation environment.
///
/// Outputs deliberately have no public accessor or serialization implementation.
pub struct Submission {
    id: String,
    binding: EvaluationBinding,
    outputs: BTreeMap<String, String>,
}

impl Submission {
    /// Creates a canonical submission from task/output pairs.
    ///
    /// # Errors
    ///
    /// Rejects malformed or duplicate task identifiers.
    pub fn new(
        submission_id: impl Into<String>,
        binding: EvaluationBinding,
        outputs: impl IntoIterator<Item = (String, String)>,
    ) -> Result<Self, ArenaError> {
        let submission_id = submission_id.into();
        validate_id("submission_id", &submission_id)?;
        let mut canonical = BTreeMap::new();
        for (task_id, output) in outputs {
            validate_id("task_id", &task_id)?;
            if canonical.insert(task_id.clone(), output).is_some() {
                return Err(ArenaError::DuplicateTaskId(task_id));
            }
        }
        Ok(Self {
            id: submission_id,
            binding,
            outputs: canonical,
        })
    }
}

/// Caller-controlled receipt identity and time, explicit for reproducibility.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReceiptContext {
    /// Globally unique event identifier.
    pub event_id: String,
    /// Stable evaluation aggregate identifier.
    pub evaluation_id: String,
    /// Actor initiating the evaluation.
    pub caller_id: String,
    /// Caller-observed Unix timestamp in milliseconds.
    pub timestamp_millis: i64,
}

impl ReceiptContext {
    fn validate(&self) -> Result<(), ArenaError> {
        validate_id("event_id", &self.event_id)?;
        validate_id("evaluation_id", &self.evaluation_id)?;
        validate_id("caller_id", &self.caller_id)
    }
}

/// Deterministic aggregate comparison with visible/sealed separation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EvaluationScores {
    /// Parent correct answers on candidate-visible tasks.
    pub parent_visible_correct: u32,
    /// Candidate correct answers on candidate-visible tasks.
    pub candidate_visible_correct: u32,
    /// Parent correct answers on evaluator-only tasks.
    pub parent_sealed_correct: u32,
    /// Candidate correct answers on evaluator-only tasks.
    pub candidate_sealed_correct: u32,
    /// Tasks the parent passed and candidate failed.
    pub regressions: u32,
    /// Tasks the parent failed and candidate passed.
    pub improvements: u32,
    /// Number of visible tasks.
    pub visible_total: u32,
    /// Number of sealed tasks.
    pub sealed_total: u32,
}

/// Public tamper-evident evaluation receipt. It never contains task text or outputs.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EvaluationReceipt {
    /// Receipt schema version.
    pub schema_version: u16,
    /// Stable evaluation identity.
    pub evaluation_id: String,
    /// Actor initiating the evaluation; explicit for deterministic reproduction.
    pub caller_id: String,
    /// Caller-observed Unix timestamp; explicit for deterministic reproduction.
    pub timestamp_millis: i64,
    /// Exact World identity.
    pub world_id: String,
    /// Exact deterministic seed.
    pub seed: u64,
    /// Exact environment identity.
    pub environment_id: String,
    /// Exact evaluator identity.
    pub evaluator_id: String,
    /// Parent submission identity.
    pub parent_submission_id: String,
    /// Candidate submission identity.
    pub candidate_submission_id: String,
    /// CAS address of the evaluator-only canonical visible manifest.
    pub visible_manifest_artifact_id: String,
    /// CAS address of the evaluator-only canonical sealed manifest.
    pub sealed_manifest_artifact_id: String,
    /// CAS address of candidate-safe visible task inputs.
    pub visible_inputs_artifact_id: String,
    /// Aggregate scores only.
    pub scores: EvaluationScores,
}

/// Durable stores owned by one evaluator composition root.
pub struct EvaluationStores {
    /// Single-writer tamper-evident event store.
    pub events: EventStore,
    /// Content-addressed artifact store.
    pub artifacts: ArtifactStore,
}

impl EvaluationStores {
    /// Opens evaluator-owned durable stores.
    ///
    /// # Errors
    ///
    /// Returns storage or integrity failures.
    pub fn open(
        database: impl AsRef<std::path::Path>,
        artifact_root: impl Into<std::path::PathBuf>,
    ) -> Result<Self, ArenaError> {
        Ok(Self {
            events: EventStore::open(database)?,
            artifacts: ArtifactStore::open(artifact_root)?,
        })
    }
}

/// Completed evaluation plus its still-owned durable stores.
pub struct RecordedEvaluation {
    /// Public aggregate receipt.
    pub receipt: EvaluationReceipt,
    /// Canonical ledger event containing the receipt.
    pub event: StoredEvent,
    /// Stores returned to preserve the single-writer ownership boundary.
    pub stores: EvaluationStores,
}

/// Evaluates paired submissions and durably records exactly one receipt event.
///
/// # Errors
///
/// Fails closed on provenance, manifest, task-set, serialization, or storage errors.
pub fn evaluate_and_record(
    mut owned_stores: EvaluationStores,
    context: ReceiptContext,
    binding: &EvaluationBinding,
    visible: &TrustedManifest,
    sealed: &TrustedManifest,
    parent: &Submission,
    candidate: &Submission,
) -> Result<RecordedEvaluation, ArenaError> {
    context.validate()?;
    validate_inputs(binding, visible, sealed, parent, candidate)?;

    let visible_manifest = serde_json::to_vec(visible)?;
    let sealed_manifest = serde_json::to_vec(sealed)?;
    let visible_inputs = serde_json::to_vec(&visible.candidate_tasks()?)?;
    let visible_manifest_id = owned_stores.artifacts.put(&visible_manifest)?;
    let sealed_manifest_id = owned_stores.artifacts.put(&sealed_manifest)?;
    let visible_inputs_id = owned_stores.artifacts.put(&visible_inputs)?;

    let aggregate = score(visible, sealed, parent, candidate)?;
    let receipt = EvaluationReceipt {
        schema_version: 1,
        evaluation_id: context.evaluation_id.clone(),
        caller_id: context.caller_id.clone(),
        timestamp_millis: context.timestamp_millis,
        world_id: binding.world_id.clone(),
        seed: binding.seed,
        environment_id: binding.environment_id.clone(),
        evaluator_id: binding.evaluator_id.clone(),
        parent_submission_id: parent.id.clone(),
        candidate_submission_id: candidate.id.clone(),
        visible_manifest_artifact_id: visible_manifest_id.as_str().to_owned(),
        sealed_manifest_artifact_id: sealed_manifest_id.as_str().to_owned(),
        visible_inputs_artifact_id: visible_inputs_id.as_str().to_owned(),
        scores: aggregate,
    };
    let payload = serde_json::to_vec(&receipt)?;
    let event = owned_stores.events.append(EventInput::new(
        context.event_id,
        context.evaluation_id,
        EVENT_TYPE,
        context.caller_id,
        context.timestamp_millis,
        payload,
    ))?;
    Ok(RecordedEvaluation {
        receipt,
        event,
        stores: owned_stores,
    })
}

fn validate_inputs(
    binding: &EvaluationBinding,
    visible: &TrustedManifest,
    sealed: &TrustedManifest,
    parent: &Submission,
    candidate: &Submission,
) -> Result<(), ArenaError> {
    if visible.visibility != Visibility::Visible || sealed.visibility != Visibility::Sealed {
        return Err(ArenaError::VisibilityMismatch);
    }
    if visible.manifest_id == sealed.manifest_id {
        return Err(ArenaError::DuplicateManifestId(visible.manifest_id.clone()));
    }
    if parent.id == candidate.id {
        return Err(ArenaError::DuplicateSubmissionId(parent.id.clone()));
    }
    for (name, actual) in [
        ("visible_manifest", &visible.binding),
        ("sealed_manifest", &sealed.binding),
        ("parent_submission", &parent.binding),
        ("candidate_submission", &candidate.binding),
    ] {
        if actual != binding {
            return Err(ArenaError::BindingMismatch(name));
        }
    }
    let mut task_ids = BTreeSet::new();
    for task in visible.tasks.iter().chain(&sealed.tasks) {
        if !task_ids.insert(task.task_id.clone()) {
            return Err(ArenaError::DuplicateTaskId(task.task_id.clone()));
        }
    }
    for submission in [parent, candidate] {
        if submission.outputs.keys().ne(task_ids.iter()) {
            return Err(ArenaError::TaskSetMismatch {
                submission_id: submission.id.clone(),
            });
        }
    }
    Ok(())
}

fn score(
    visible: &TrustedManifest,
    sealed: &TrustedManifest,
    parent: &Submission,
    candidate: &Submission,
) -> Result<EvaluationScores, ArenaError> {
    let visible_total = u32::try_from(visible.tasks.len()).map_err(|_| ArenaError::TooManyTasks)?;
    let sealed_total = u32::try_from(sealed.tasks.len()).map_err(|_| ArenaError::TooManyTasks)?;
    let mut scores = EvaluationScores {
        parent_visible_correct: 0,
        candidate_visible_correct: 0,
        parent_sealed_correct: 0,
        candidate_sealed_correct: 0,
        regressions: 0,
        improvements: 0,
        visible_total,
        sealed_total,
    };
    for manifest in [visible, sealed] {
        for task in &manifest.tasks {
            let parent_correct = parent.outputs[&task.task_id] == task.expected_output;
            let candidate_correct = candidate.outputs[&task.task_id] == task.expected_output;
            match manifest.visibility {
                Visibility::Visible => {
                    scores.parent_visible_correct += u32::from(parent_correct);
                    scores.candidate_visible_correct += u32::from(candidate_correct);
                }
                Visibility::Sealed => {
                    scores.parent_sealed_correct += u32::from(parent_correct);
                    scores.candidate_sealed_correct += u32::from(candidate_correct);
                }
            }
            scores.regressions += u32::from(parent_correct && !candidate_correct);
            scores.improvements += u32::from(!parent_correct && candidate_correct);
        }
    }
    Ok(scores)
}

fn validate_world_id(value: &str) -> Result<(), ArenaError> {
    let Some(hash) = value.strip_prefix("hephaestus:world:") else {
        return Err(ArenaError::InvalidId {
            field: "world_id",
            value: value.to_owned(),
        });
    };
    if hash.len() != 64
        || !hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ArenaError::InvalidId {
            field: "world_id",
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn validate_id(field: &'static str, value: &str) -> Result<(), ArenaError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'))
    {
        return Err(ArenaError::InvalidId {
            field,
            value: value.to_owned(),
        });
    }
    Ok(())
}

/// Parses and verifies an artifact referenced by a receipt.
///
/// # Errors
///
/// Rejects malformed addresses, missing blobs, and hash mismatches.
pub fn verify_receipt_artifact(
    artifacts: &ArtifactStore,
    artifact_id: &str,
) -> Result<Vec<u8>, ArenaError> {
    let id = ArtifactId::parse(artifact_id)?;
    Ok(artifacts.get(&id)?)
}
