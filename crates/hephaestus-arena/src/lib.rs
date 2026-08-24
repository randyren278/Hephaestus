//! Trusted deterministic evaluation with sealed task boundaries and durable receipts.

mod error;

use std::collections::{BTreeMap, BTreeSet};

pub use error::ArenaError;
use hephaestus_experience::{
    RunBudgetReceipt, RunCompletionReason, RunResultReceipt, RunResultVerifier,
};
use hephaestus_genome::CompiledWorld;
use hephaestus_ledger::{ArtifactId, ArtifactStore, EventInput, EventStore, StoredEvent};
use serde::{Deserialize, Serialize};

const EVENT_TYPE: &str = "evaluation.recorded";
const EVENT_ACTOR: &str = "arena-plane";
const VISIBLE_MANIFEST_KEY: &str = "arena.visible_manifest";
const SEALED_MANIFEST_KEY: &str = "arena.sealed_manifest";
const EVALUATOR_KEY: &str = "arena.evaluator";
const RUN_RESULT_VERIFIER_KEY: &str = "arena.runtime_verifier";
const EXACT_MATCH_EVALUATOR: &[u8] = b"exact-match-evaluator-v1";
const MAX_TASKS: usize = 1_000;
const MAX_TASK_TEXT_BYTES: usize = 64 * 1024;
const MAX_SUBMISSION_OUTPUT_BYTES: usize = 8 * 1024 * 1024;

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
    /// Exact hard budget tuple required for every paired trial.
    budget: RunBudgetReceipt,
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
        budget: RunBudgetReceipt,
    ) -> Result<Self, ArenaError> {
        let binding = Self {
            world_id: world_id.into(),
            seed,
            environment_id: environment_id.into(),
            evaluator_id: evaluator_id.into(),
            budget,
        };
        validate_world_id(&binding.world_id)?;
        validate_id("environment_id", &binding.environment_id)?;
        ArtifactId::parse(binding.evaluator_id.clone())?;
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

    /// Returns the exact hard budget tuple required for paired trials.
    #[must_use]
    pub const fn budget(&self) -> RunBudgetReceipt {
        self.budget
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
        let input = input.into();
        let expected_output = expected_output.into();
        validate_text("task.input", &input)?;
        validate_text("task.expected_output", &expected_output)?;
        Ok(Self {
            task_id,
            input,
            expected_output,
        })
    }
}

/// Evaluator-owned immutable task manifest.
#[derive(Clone, Eq, PartialEq, Serialize)]
pub struct TrustedManifest {
    schema_version: u16,
    manifest_id: String,
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
        visibility: Visibility,
        mut tasks: Vec<TrustedTask>,
    ) -> Result<Self, ArenaError> {
        let manifest_id = manifest_id.into();
        validate_id("manifest_id", &manifest_id)?;
        if tasks.is_empty() {
            return Err(ArenaError::EmptyManifest);
        }
        if tasks.len() > MAX_TASKS {
            return Err(ArenaError::TooManyTasks);
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

/// A caller-visible plan that binds each task to one canonical runtime result event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrialPlan {
    trials: BTreeMap<String, String>,
}

impl TrialPlan {
    /// Creates a canonical task-to-runtime-event plan.
    ///
    /// # Errors
    ///
    /// Rejects malformed or duplicate task and event identifiers.
    pub fn new(trials: impl IntoIterator<Item = (String, String)>) -> Result<Self, ArenaError> {
        let mut canonical = BTreeMap::new();
        let mut run_events = BTreeSet::new();
        for (task_id, event_id) in trials {
            validate_id("task_id", &task_id)?;
            validate_run_event_id(&event_id)?;
            if !run_events.insert(event_id.clone()) {
                return Err(ArenaError::DuplicateRunEvent(event_id));
            }
            if canonical.insert(task_id.clone(), event_id).is_some() {
                return Err(ArenaError::DuplicateTaskId(task_id));
            }
            if canonical.len() > MAX_TASKS {
                return Err(ArenaError::TooManyTasks);
            }
        }
        Ok(Self { trials: canonical })
    }
}

/// Caller-provided evaluation identity and observation metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReceiptContext {
    /// Globally unique event identifier.
    pub event_id: String,
    /// Stable evaluation aggregate identifier.
    pub evaluation_id: String,
    /// Caller metadata recorded only in the receipt payload.
    pub caller_id: String,
    /// Caller-observed Unix timestamp in milliseconds.
    pub timestamp_millis: i64,
}

impl ReceiptContext {
    fn validate(&self) -> Result<(), ArenaError> {
        validate_id("evaluation_id", &self.evaluation_id)?;
        validate_id("caller_id", &self.caller_id)?;
        let expected = canonical_event_id(&self.evaluation_id);
        if self.event_id != expected {
            return Err(ArenaError::InvalidId {
                field: "event_id",
                value: self.event_id.clone(),
            });
        }
        Ok(())
    }
}

/// Deterministic aggregate comparison with visible/sealed separation.
#[derive(Clone, Debug, Eq, PartialEq)]
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OperatorScores {
    parent_visible_correct: u32,
    candidate_visible_correct: u32,
    parent_sealed_correct: u32,
    candidate_sealed_correct: u32,
    regressions: u32,
    improvements: u32,
    visible_total: u32,
    sealed_total: u32,
}

impl From<&OperatorScores> for EvaluationScores {
    fn from(scores: &OperatorScores) -> Self {
        Self {
            parent_visible_correct: scores.parent_visible_correct,
            candidate_visible_correct: scores.candidate_visible_correct,
            parent_sealed_correct: scores.parent_sealed_correct,
            candidate_sealed_correct: scores.candidate_sealed_correct,
            regressions: scores.regressions,
            improvements: scores.improvements,
            visible_total: scores.visible_total,
            sealed_total: scores.sealed_total,
        }
    }
}

/// Candidate-safe, visible-only evaluation result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluationSummary {
    /// Summary schema version.
    pub schema_version: u16,
    /// Stable evaluation identity.
    pub evaluation_id: String,
    /// Exact World identity.
    pub world_id: String,
    /// Immutable parent Genome identity.
    pub parent_genome_id: String,
    /// Immutable candidate Genome identity.
    pub candidate_genome_id: String,
    /// Parent correct answers on candidate-visible tasks.
    pub parent_visible_correct: u32,
    /// Candidate correct answers on candidate-visible tasks.
    pub candidate_visible_correct: u32,
    /// Number of candidate-visible tasks.
    pub visible_total: u32,
}

/// Evaluator-only durable wire format. Never expose or serialize this through public APIs.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OperatorReceipt {
    schema_version: u16,
    evaluation_id: String,
    caller_id: String,
    timestamp_millis: i64,
    world_id: String,
    seed: u64,
    environment_id: String,
    evaluator_id: String,
    parent_submission_id: String,
    parent_genome_id: String,
    candidate_submission_id: String,
    candidate_genome_id: String,
    /// CAS address of the evaluator-only canonical visible manifest.
    visible_manifest_artifact_id: String,
    /// CAS address of the evaluator-only canonical sealed manifest.
    sealed_manifest_artifact_id: String,
    /// CAS address of candidate-safe visible task inputs.
    visible_inputs_artifact_id: String,
    /// CAS address of the evaluator-only canonical parent submission.
    parent_submission_artifact_id: String,
    /// CAS address of the evaluator-only canonical candidate submission.
    candidate_submission_artifact_id: String,
    scores: OperatorScores,
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

/// Payload-free ledger metadata safe to expose to callers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvaluationEvent {
    /// Canonical global ledger sequence.
    pub sequence: u64,
    /// Deterministic event identity.
    pub event_id: String,
    /// Deterministic evaluation aggregate identity.
    pub aggregate_id: String,
    /// Stable event type.
    pub event_type: String,
    /// Fixed trusted actor.
    pub actor: String,
    /// Caller-observed time.
    pub timestamp_millis: i64,
    /// Tamper-evident event hash.
    pub hash: [u8; 32],
}

impl From<&StoredEvent> for EvaluationEvent {
    fn from(event: &StoredEvent) -> Self {
        Self {
            sequence: event.sequence,
            event_id: event.event_id.clone(),
            aggregate_id: event.aggregate_id.clone(),
            event_type: event.event_type.clone(),
            actor: event.actor.clone(),
            timestamp_millis: event.timestamp_millis,
            hash: event.hash,
        }
    }
}

/// Candidate-facing evaluation result containing no raw stores or sealed evidence.
///
/// Raw ledger and artifact access is deliberately owned by [`OperatorEvaluation`].
/// Candidate-facing code cannot reach it through this type:
///
/// ```compile_fail
/// use hephaestus_arena::RecordedEvaluation;
///
/// fn raw_stores_are_not_candidate_visible(result: RecordedEvaluation) {
///     let _ = result.stores;
/// }
/// ```
///
/// Evaluator-only evidence access is absent from this type as well:
///
/// ```compile_fail
/// use hephaestus_arena::RecordedEvaluation;
///
/// fn sealed_evidence_is_not_candidate_visible(result: &RecordedEvaluation) {
///     let _ = result.operator_parent_submission();
/// }
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordedEvaluation {
    /// Candidate-safe visible-only result.
    pub summary: EvaluationSummary,
    /// Payload-free canonical ledger metadata.
    pub event: EvaluationEvent,
}

/// Trusted evaluator-owned result, receipt, and durable-store capability.
///
/// Only this operator-side wrapper can inspect sealed aggregates or reclaim the
/// stores required for subsequent single-writer operations. Candidate-facing
/// adapters should return [`Self::into_candidate_result`] instead.
pub struct OperatorEvaluation {
    recorded: RecordedEvaluation,
    stores: EvaluationStores,
    operator_receipt: OperatorReceipt,
}

impl OperatorEvaluation {
    /// Borrows the candidate-safe result without exposing operator capabilities.
    #[must_use]
    pub const fn candidate_result(&self) -> &RecordedEvaluation {
        &self.recorded
    }

    /// Consumes the operator wrapper and returns only the candidate-safe result.
    #[must_use]
    pub fn into_candidate_result(self) -> RecordedEvaluation {
        self.recorded
    }

    /// Reclaims the evaluator-owned stores for the next trusted operation.
    #[must_use]
    pub fn into_stores(self) -> EvaluationStores {
        self.stores
    }

    /// Returns aggregate evaluator-only scores to a trusted operator.
    #[must_use]
    pub fn operator_scores(&self) -> EvaluationScores {
        EvaluationScores::from(&self.operator_receipt.scores)
    }

    /// Reads and verifies the evaluator-only parent submission evidence.
    ///
    /// # Errors
    ///
    /// Rejects missing, malformed, or corrupted receipt evidence.
    pub fn operator_parent_submission(&self) -> Result<Vec<u8>, ArenaError> {
        verify_operator_artifact(
            &self.stores.artifacts,
            &self.operator_receipt.parent_submission_artifact_id,
        )
    }

    /// Reads and verifies candidate-safe visible task inputs.
    ///
    /// # Errors
    ///
    /// Rejects missing, malformed, or corrupted receipt evidence.
    pub fn operator_visible_inputs(&self) -> Result<Vec<u8>, ArenaError> {
        verify_operator_artifact(
            &self.stores.artifacts,
            &self.operator_receipt.visible_inputs_artifact_id,
        )
    }
}

/// Exact evaluator-owned inputs for one paired comparison.
#[derive(Clone, Copy)]
pub struct EvaluationInputs<'a> {
    /// World, seed, environment, and evaluator provenance.
    pub binding: &'a EvaluationBinding,
    /// Candidate-visible task manifest.
    pub visible: &'a TrustedManifest,
    /// Evaluator-only sealed task manifest.
    pub sealed: &'a TrustedManifest,
    /// Baseline task-to-runtime-event plan.
    pub parent: &'a TrialPlan,
    /// Proposed replacement task-to-runtime-event plan.
    pub candidate: &'a TrialPlan,
}

#[derive(Serialize)]
struct SubmissionEvidence {
    schema_version: u16,
    genome_id: String,
    trials: BTreeMap<String, TrialEvidence>,
}

#[derive(Serialize)]
struct TrialEvidence {
    run_result_event_id: String,
    source_revision: String,
    stdout_artifact_id: String,
    stderr_artifact_id: String,
    trace_artifact_ids: Vec<String>,
}

struct ResolvedSubmission {
    id: String,
    genome_id: String,
    outputs: BTreeMap<String, String>,
    revisions: BTreeMap<String, String>,
    evidence: Vec<u8>,
}

struct ResolvedPair {
    parent: ResolvedSubmission,
    candidate: ResolvedSubmission,
}

struct PreparedArtifacts {
    visible_manifest: Vec<u8>,
    sealed_manifest: Vec<u8>,
    visible_inputs: Vec<u8>,
    parent_submission: Vec<u8>,
    candidate_submission: Vec<u8>,
    visible_manifest_id: ArtifactId,
    sealed_manifest_id: ArtifactId,
    visible_inputs_id: ArtifactId,
    parent_submission_id: ArtifactId,
    candidate_submission_id: ArtifactId,
}

impl PreparedArtifacts {
    fn publish(&self, artifacts: &ArtifactStore) -> Result<(), ArenaError> {
        for bytes in [
            &self.visible_manifest,
            &self.sealed_manifest,
            &self.visible_inputs,
            &self.parent_submission,
            &self.candidate_submission,
        ] {
            artifacts.put(bytes)?;
        }
        Ok(())
    }
}

/// Evaluates paired submissions and durably records exactly one receipt event.
///
/// # Errors
///
/// Fails closed on provenance, manifest, task-set, serialization, or storage errors.
pub fn evaluate_and_record(
    mut owned_stores: EvaluationStores,
    context: ReceiptContext,
    world: &CompiledWorld,
    inputs: EvaluationInputs<'_>,
) -> Result<OperatorEvaluation, ArenaError> {
    let EvaluationInputs {
        binding,
        visible,
        sealed,
        parent,
        candidate,
    } = inputs;
    context.validate()?;
    let task_inputs = validate_evaluation_inputs(world, binding, visible, sealed)?;
    validate_world_evaluator_artifacts(world, &owned_stores.artifacts, binding, visible, sealed)?;
    let run_result_verifier = run_result_verifier(world, &owned_stores.artifacts)?;
    let history = owned_stores.events.replay_verified()?;
    let ResolvedPair { parent, candidate } = resolve_pair(
        parent,
        candidate,
        &task_inputs,
        binding,
        &history,
        &owned_stores.artifacts,
        &run_result_verifier,
    )?;

    let prepared = prepare_artifacts(visible, sealed, &parent, &candidate)?;

    let aggregate = score(visible, sealed, &parent, &candidate)?;
    let summary = EvaluationSummary {
        schema_version: 1,
        evaluation_id: context.evaluation_id.clone(),
        world_id: binding.world_id.clone(),
        parent_genome_id: parent.genome_id.clone(),
        candidate_genome_id: candidate.genome_id.clone(),
        parent_visible_correct: aggregate.parent_visible_correct,
        candidate_visible_correct: aggregate.candidate_visible_correct,
        visible_total: aggregate.visible_total,
    };
    let receipt = OperatorReceipt {
        schema_version: 1,
        evaluation_id: context.evaluation_id.clone(),
        caller_id: context.caller_id.clone(),
        timestamp_millis: context.timestamp_millis,
        world_id: binding.world_id.clone(),
        seed: binding.seed,
        environment_id: binding.environment_id.clone(),
        evaluator_id: binding.evaluator_id.clone(),
        parent_submission_id: parent.id.clone(),
        parent_genome_id: parent.genome_id.clone(),
        candidate_submission_id: candidate.id.clone(),
        candidate_genome_id: candidate.genome_id.clone(),
        visible_manifest_artifact_id: prepared.visible_manifest_id.as_str().to_owned(),
        sealed_manifest_artifact_id: prepared.sealed_manifest_id.as_str().to_owned(),
        visible_inputs_artifact_id: prepared.visible_inputs_id.as_str().to_owned(),
        parent_submission_artifact_id: prepared.parent_submission_id.as_str().to_owned(),
        candidate_submission_artifact_id: prepared.candidate_submission_id.as_str().to_owned(),
        scores: aggregate,
    };
    let expected_event_id = canonical_event_id(&context.evaluation_id);
    let expected_aggregate_id = canonical_aggregate_id(&context.evaluation_id);

    if let Some(existing) = history
        .into_iter()
        .find(|event| event.event_id == expected_event_id)
    {
        let existing_receipt = rehydrate_operator_receipt(&owned_stores.artifacts, &existing)?;
        if existing.aggregate_id != expected_aggregate_id
            || !receipts_match_except_timestamp(&existing_receipt, &receipt)
        {
            return Err(ArenaError::EvaluationConflict(context.evaluation_id));
        }
        return Ok(OperatorEvaluation {
            recorded: RecordedEvaluation {
                summary: summary_from_receipt(&existing_receipt),
                event: EvaluationEvent::from(&existing),
            },
            stores: owned_stores,
            operator_receipt: existing_receipt,
        });
    }

    // All validation and conflict detection is complete. CAS publication can now begin.
    prepared.publish(&owned_stores.artifacts)?;
    let payload = serde_json::to_vec(&receipt)?;
    let event = owned_stores.events.append(EventInput::new(
        expected_event_id,
        expected_aggregate_id,
        EVENT_TYPE,
        EVENT_ACTOR,
        context.timestamp_millis,
        payload,
    ))?;
    Ok(OperatorEvaluation {
        recorded: RecordedEvaluation {
            summary,
            event: EvaluationEvent::from(&event),
        },
        stores: owned_stores,
        operator_receipt: receipt,
    })
}

fn resolve_pair(
    parent_plan: &TrialPlan,
    candidate_plan: &TrialPlan,
    task_inputs: &BTreeMap<String, String>,
    binding: &EvaluationBinding,
    history: &[StoredEvent],
    artifacts: &ArtifactStore,
    run_result_verifier: &RunResultVerifier,
) -> Result<ResolvedPair, ArenaError> {
    let parent = resolve_plan(
        "parent",
        parent_plan,
        task_inputs,
        binding,
        history,
        artifacts,
        run_result_verifier,
    )?;
    let candidate = resolve_plan(
        "candidate",
        candidate_plan,
        task_inputs,
        binding,
        history,
        artifacts,
        run_result_verifier,
    )?;
    if parent.genome_id == candidate.genome_id {
        return Err(ArenaError::DuplicateGenomeId(parent.genome_id));
    }
    for task_id in task_inputs.keys() {
        if parent.revisions[task_id] != candidate.revisions[task_id] {
            return Err(ArenaError::SourceRevisionMismatch(task_id.clone()));
        }
    }

    Ok(ResolvedPair { parent, candidate })
}

fn prepare_artifacts(
    visible: &TrustedManifest,
    sealed: &TrustedManifest,
    parent: &ResolvedSubmission,
    candidate: &ResolvedSubmission,
) -> Result<PreparedArtifacts, ArenaError> {
    let visible_manifest = serde_json::to_vec(visible)?;
    let sealed_manifest = serde_json::to_vec(sealed)?;
    let visible_inputs = serde_json::to_vec(&visible.candidate_tasks()?)?;
    let parent_submission = parent.evidence.clone();
    let candidate_submission = candidate.evidence.clone();
    let prepared = PreparedArtifacts {
        visible_manifest_id: ArtifactId::for_bytes(&visible_manifest),
        sealed_manifest_id: ArtifactId::for_bytes(&sealed_manifest),
        visible_inputs_id: ArtifactId::for_bytes(&visible_inputs),
        parent_submission_id: ArtifactId::for_bytes(&parent_submission),
        candidate_submission_id: ArtifactId::for_bytes(&candidate_submission),
        visible_manifest,
        sealed_manifest,
        visible_inputs,
        parent_submission,
        candidate_submission,
    };
    Ok(prepared)
}

fn validate_world_evaluator_artifacts(
    world: &CompiledWorld,
    artifacts: &ArtifactStore,
    binding: &EvaluationBinding,
    visible: &TrustedManifest,
    sealed: &TrustedManifest,
) -> Result<(), ArenaError> {
    verify_world_artifact(
        world,
        artifacts,
        VISIBLE_MANIFEST_KEY,
        &ArtifactId::for_bytes(&serde_json::to_vec(visible)?),
    )?;
    verify_world_artifact(
        world,
        artifacts,
        SEALED_MANIFEST_KEY,
        &ArtifactId::for_bytes(&serde_json::to_vec(sealed)?),
    )?;
    let evaluator_id = required_world_artifact(world, EVALUATOR_KEY)?;
    if evaluator_id != binding.evaluator_id {
        return Err(ArenaError::WorldArtifactMismatch(EVALUATOR_KEY));
    }
    if artifacts.get(&ArtifactId::parse(evaluator_id)?)? != EXACT_MATCH_EVALUATOR {
        return Err(ArenaError::UnsupportedEvaluator);
    }
    Ok(())
}

fn canonical_event_id(evaluation_id: &str) -> String {
    format!("arena:evaluation:{evaluation_id}:recorded")
}

fn canonical_aggregate_id(evaluation_id: &str) -> String {
    format!("arena:evaluation:{evaluation_id}")
}

fn summary_from_receipt(receipt: &OperatorReceipt) -> EvaluationSummary {
    EvaluationSummary {
        schema_version: receipt.schema_version,
        evaluation_id: receipt.evaluation_id.clone(),
        world_id: receipt.world_id.clone(),
        parent_genome_id: receipt.parent_genome_id.clone(),
        candidate_genome_id: receipt.candidate_genome_id.clone(),
        parent_visible_correct: receipt.scores.parent_visible_correct,
        candidate_visible_correct: receipt.scores.candidate_visible_correct,
        visible_total: receipt.scores.visible_total,
    }
}

fn receipts_match_except_timestamp(left: &OperatorReceipt, right: &OperatorReceipt) -> bool {
    let mut normalized = right.clone();
    normalized.timestamp_millis = left.timestamp_millis;
    left == &normalized
}

fn rehydrate_operator_receipt(
    artifacts: &ArtifactStore,
    event: &StoredEvent,
) -> Result<OperatorReceipt, ArenaError> {
    let receipt: OperatorReceipt = serde_json::from_slice(&event.payload)?;
    if receipt.schema_version != 1
        || event.event_id != canonical_event_id(&receipt.evaluation_id)
        || event.aggregate_id != canonical_aggregate_id(&receipt.evaluation_id)
        || event.event_type != EVENT_TYPE
        || event.actor != EVENT_ACTOR
        || event.timestamp_millis != receipt.timestamp_millis
    {
        return Err(ArenaError::InvalidStoredReceipt("event metadata"));
    }
    for artifact_id in [
        &receipt.visible_manifest_artifact_id,
        &receipt.sealed_manifest_artifact_id,
        &receipt.visible_inputs_artifact_id,
        &receipt.parent_submission_artifact_id,
        &receipt.candidate_submission_artifact_id,
    ] {
        verify_operator_artifact(artifacts, artifact_id)?;
    }
    Ok(receipt)
}

fn validate_evaluation_inputs(
    world: &CompiledWorld,
    binding: &EvaluationBinding,
    visible: &TrustedManifest,
    sealed: &TrustedManifest,
) -> Result<BTreeMap<String, String>, ArenaError> {
    if binding.world_id != world.id() {
        return Err(ArenaError::BindingMismatch("world"));
    }
    if visible.visibility != Visibility::Visible || sealed.visibility != Visibility::Sealed {
        return Err(ArenaError::VisibilityMismatch);
    }
    if visible.manifest_id == sealed.manifest_id {
        return Err(ArenaError::DuplicateManifestId(visible.manifest_id.clone()));
    }
    let mut task_inputs = BTreeMap::new();
    for task in visible.tasks.iter().chain(&sealed.tasks) {
        if task_inputs
            .insert(
                task.task_id.clone(),
                ArtifactId::for_bytes(task.input.as_bytes())
                    .as_str()
                    .to_owned(),
            )
            .is_some()
        {
            return Err(ArenaError::DuplicateTaskId(task.task_id.clone()));
        }
        if task_inputs.len() > MAX_TASKS {
            return Err(ArenaError::TooManyTasks);
        }
    }
    Ok(task_inputs)
}

fn resolve_plan(
    plan_name: &'static str,
    plan: &TrialPlan,
    expected_tasks: &BTreeMap<String, String>,
    binding: &EvaluationBinding,
    history: &[StoredEvent],
    artifacts: &ArtifactStore,
    run_result_verifier: &RunResultVerifier,
) -> Result<ResolvedSubmission, ArenaError> {
    if plan.trials.keys().ne(expected_tasks.keys()) {
        return Err(ArenaError::TaskSetMismatch {
            submission_id: plan_name.to_owned(),
        });
    }
    let events = history
        .iter()
        .map(|event| (event.event_id.as_str(), event))
        .collect::<BTreeMap<_, _>>();
    let mut genome_id = None;
    let mut outputs = BTreeMap::new();
    let mut revisions = BTreeMap::new();
    let mut trials = BTreeMap::new();
    let mut output_bytes = 0_usize;
    for (task_id, event_id) in &plan.trials {
        let event = events
            .get(event_id.as_str())
            .ok_or_else(|| ArenaError::UnknownRunEvent(event_id.clone()))?;
        let receipt = RunResultReceipt::parse_from_event(event, run_result_verifier)?;
        if receipt.completion_reason != RunCompletionReason::Success {
            return Err(ArenaError::RunNotSuccessful(event_id.clone()));
        }
        if receipt.world_id != binding.world_id {
            return Err(ArenaError::RunWorldMismatch(event_id.clone()));
        }
        if receipt.task_id != *task_id
            || receipt.input_commitment != expected_tasks[task_id]
            || receipt.seed != binding.seed
            || receipt.environment_id != binding.environment_id
            || receipt.budget != binding.budget
        {
            return Err(ArenaError::BindingMismatch("runtime experiment context"));
        }
        if genome_id
            .as_ref()
            .is_some_and(|genome| genome != &receipt.genome_id)
        {
            return Err(ArenaError::MixedSubmissionGenome);
        }
        genome_id.get_or_insert_with(|| receipt.genome_id.clone());
        let stdout = verify_operator_artifact(artifacts, &receipt.stdout_artifact_id)?;
        verify_operator_artifact(artifacts, &receipt.stderr_artifact_id)?;
        for artifact_id in &receipt.trace_artifact_ids {
            verify_operator_artifact(artifacts, artifact_id)?;
        }
        output_bytes = output_bytes
            .checked_add(stdout.len())
            .ok_or(ArenaError::TextTooLarge {
                field: "submission.outputs",
                limit: MAX_SUBMISSION_OUTPUT_BYTES,
            })?;
        if stdout.len() > MAX_TASK_TEXT_BYTES {
            return Err(ArenaError::TextTooLarge {
                field: "submission.output",
                limit: MAX_TASK_TEXT_BYTES,
            });
        }
        if output_bytes > MAX_SUBMISSION_OUTPUT_BYTES {
            return Err(ArenaError::TextTooLarge {
                field: "submission.outputs",
                limit: MAX_SUBMISSION_OUTPUT_BYTES,
            });
        }
        let output = String::from_utf8(stdout)
            .map_err(|_| ArenaError::InvalidRunOutput(event_id.clone()))?;
        outputs.insert(task_id.clone(), output);
        revisions.insert(task_id.clone(), receipt.source_revision.clone());
        trials.insert(
            task_id.clone(),
            TrialEvidence {
                run_result_event_id: event_id.clone(),
                source_revision: receipt.source_revision,
                stdout_artifact_id: receipt.stdout_artifact_id,
                stderr_artifact_id: receipt.stderr_artifact_id,
                trace_artifact_ids: receipt.trace_artifact_ids,
            },
        );
    }
    let evidence = serde_json::to_vec(&SubmissionEvidence {
        schema_version: 1,
        genome_id: genome_id.clone().ok_or(ArenaError::EmptyManifest)?,
        trials,
    })?;
    Ok(ResolvedSubmission {
        id: ArtifactId::for_bytes(&evidence).as_str().to_owned(),
        genome_id: genome_id.ok_or(ArenaError::EmptyManifest)?,
        outputs,
        revisions,
        evidence,
    })
}

fn required_world_artifact<'a>(
    world: &'a CompiledWorld,
    name: &'static str,
) -> Result<&'a str, ArenaError> {
    world
        .evaluator_artifact(name)
        .ok_or(ArenaError::MissingWorldArtifact(name))
}

fn run_result_verifier(
    world: &CompiledWorld,
    artifacts: &ArtifactStore,
) -> Result<RunResultVerifier, ArenaError> {
    let id = required_world_artifact(world, RUN_RESULT_VERIFIER_KEY)?;
    let bytes = artifacts.get(&ArtifactId::parse(id)?)?;
    let public_key: [u8; 32] = bytes
        .try_into()
        .map_err(|_| ArenaError::InvalidStoredReceipt("runtime verifier"))?;
    RunResultVerifier::from_public_key_bytes(public_key)
        .map_err(|_| ArenaError::InvalidStoredReceipt("runtime verifier"))
}

fn verify_world_artifact(
    world: &CompiledWorld,
    artifacts: &ArtifactStore,
    name: &'static str,
    actual: &ArtifactId,
) -> Result<(), ArenaError> {
    let expected = required_world_artifact(world, name)?;
    if expected != actual.as_str() {
        return Err(ArenaError::WorldArtifactMismatch(name));
    }
    artifacts.get(&ArtifactId::parse(expected)?)?;
    Ok(())
}

fn score(
    visible: &TrustedManifest,
    sealed: &TrustedManifest,
    parent: &ResolvedSubmission,
    candidate: &ResolvedSubmission,
) -> Result<OperatorScores, ArenaError> {
    let visible_total = u32::try_from(visible.tasks.len()).map_err(|_| ArenaError::TooManyTasks)?;
    let sealed_total = u32::try_from(sealed.tasks.len()).map_err(|_| ArenaError::TooManyTasks)?;
    let mut scores = OperatorScores {
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

fn validate_text(field: &'static str, value: &str) -> Result<(), ArenaError> {
    if value.len() > MAX_TASK_TEXT_BYTES {
        return Err(ArenaError::TextTooLarge {
            field,
            limit: MAX_TASK_TEXT_BYTES,
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

fn validate_run_event_id(value: &str) -> Result<(), ArenaError> {
    let Some(run_id) = value.strip_prefix("result:") else {
        return Err(ArenaError::InvalidId {
            field: "run_event_id",
            value: value.to_owned(),
        });
    };
    if run_id.is_empty()
        || run_id.len() > 128
        || !run_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(ArenaError::InvalidId {
            field: "run_event_id",
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn verify_operator_artifact(
    artifacts: &ArtifactStore,
    artifact_id: &str,
) -> Result<Vec<u8>, ArenaError> {
    let id = ArtifactId::parse(artifact_id)?;
    Ok(artifacts.get(&id)?)
}
