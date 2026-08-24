//! Strict wire protocol for the trusted exact-match evaluator worker.

use std::collections::BTreeSet;

use hephaestus_ledger::ArtifactId;
use serde::{Deserialize, Serialize};

use crate::ArenaError;

/// Maximum canonical evaluator request size accepted across the process boundary.
pub const MAX_EVALUATOR_REQUEST_BYTES: usize = 16 * 1024 * 1024;
/// Maximum canonical evaluator response size accepted across the process boundary.
pub const MAX_EVALUATOR_RESPONSE_BYTES: usize = 64 * 1024;

/// One evaluator-only exact-match trial.
#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluatorTrial {
    /// Stable task identity.
    pub task_id: String,
    /// Trusted expected output, never returned by the worker.
    pub expected_output: String,
    /// Immutable parent output.
    pub parent_output: String,
    /// Immutable candidate output.
    pub candidate_output: String,
}

/// Evaluator-only request passed over bounded stdin.
#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluatorRequest {
    /// Wire schema version.
    pub schema_version: u16,
    /// Stable evaluation identity used as a freshness and replay binding.
    pub evaluation_id: String,
    /// World-bound evaluator artifact identity.
    pub evaluator_id: String,
    /// Candidate-visible trials.
    pub visible: Vec<EvaluatorTrial>,
    /// Evaluator-only trials.
    pub sealed: Vec<EvaluatorTrial>,
}

/// Aggregate scores returned by the evaluator worker.
#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluatorScores {
    /// Parent correct answers on visible tasks.
    pub parent_visible_correct: u32,
    /// Candidate correct answers on visible tasks.
    pub candidate_visible_correct: u32,
    /// Parent correct answers on sealed tasks.
    pub parent_sealed_correct: u32,
    /// Candidate correct answers on sealed tasks.
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

/// Candidate-safe evaluator response. It contains no task-level or expected data.
#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluatorResponse {
    /// Wire schema version.
    pub schema_version: u16,
    /// Content address of the exact canonical request bytes consumed by the worker.
    pub request_artifact_id: String,
    /// Aggregate exact-match scores.
    pub scores: EvaluatorScores,
}

/// Parses, validates, and evaluates one canonical worker request.
///
/// # Errors
///
/// Rejects oversized, malformed, non-canonical, duplicated, or unsupported requests.
#[doc(hidden)]
pub fn evaluate_request(bytes: &[u8]) -> Result<Vec<u8>, ArenaError> {
    if bytes.len() > MAX_EVALUATOR_REQUEST_BYTES {
        return Err(ArenaError::EvaluatorProtocol("request exceeds byte limit"));
    }
    let request: EvaluatorRequest = serde_json::from_slice(bytes)?;
    validate_request(&request)?;
    let canonical = serde_json::to_vec(&request)?;
    if canonical != bytes {
        return Err(ArenaError::EvaluatorProtocol(
            "request is not canonical JSON",
        ));
    }
    let scores = score(&request)?;
    let response = serde_json::to_vec(&EvaluatorResponse {
        schema_version: 1,
        request_artifact_id: ArtifactId::for_bytes(bytes).as_str().to_owned(),
        scores,
    })?;
    if response.len() > MAX_EVALUATOR_RESPONSE_BYTES {
        return Err(ArenaError::EvaluatorProtocol("response exceeds byte limit"));
    }
    Ok(response)
}

fn validate_request(request: &EvaluatorRequest) -> Result<(), ArenaError> {
    if request.schema_version != 1 {
        return Err(ArenaError::EvaluatorProtocol("unsupported request schema"));
    }
    super::validate_id("evaluation_id", &request.evaluation_id)?;
    ArtifactId::parse(request.evaluator_id.clone())?;
    if request.visible.is_empty() || request.sealed.is_empty() {
        return Err(ArenaError::EvaluatorProtocol("task sets must be non-empty"));
    }
    if request.visible.len().saturating_add(request.sealed.len()) > super::MAX_TASKS {
        return Err(ArenaError::TooManyTasks);
    }
    let mut task_ids = BTreeSet::new();
    for trial in request.visible.iter().chain(&request.sealed) {
        super::validate_id("task_id", &trial.task_id)?;
        for (field, value) in [
            ("task.expected_output", &trial.expected_output),
            ("submission.output", &trial.parent_output),
            ("submission.output", &trial.candidate_output),
        ] {
            super::validate_text(field, value)?;
        }
        if !task_ids.insert(&trial.task_id) {
            return Err(ArenaError::DuplicateTaskId(trial.task_id.clone()));
        }
    }
    Ok(())
}

fn score(request: &EvaluatorRequest) -> Result<EvaluatorScores, ArenaError> {
    let visible_total =
        u32::try_from(request.visible.len()).map_err(|_| ArenaError::TooManyTasks)?;
    let sealed_total = u32::try_from(request.sealed.len()).map_err(|_| ArenaError::TooManyTasks)?;
    let mut scores = EvaluatorScores {
        parent_visible_correct: 0,
        candidate_visible_correct: 0,
        parent_sealed_correct: 0,
        candidate_sealed_correct: 0,
        regressions: 0,
        improvements: 0,
        visible_total,
        sealed_total,
    };
    for (visible, trials) in [
        (true, request.visible.as_slice()),
        (false, request.sealed.as_slice()),
    ] {
        for trial in trials {
            let parent_correct = trial.parent_output == trial.expected_output;
            let candidate_correct = trial.candidate_output == trial.expected_output;
            if visible {
                scores.parent_visible_correct += u32::from(parent_correct);
                scores.candidate_visible_correct += u32::from(candidate_correct);
            } else {
                scores.parent_sealed_correct += u32::from(parent_correct);
                scores.candidate_sealed_correct += u32::from(candidate_correct);
            }
            scores.regressions += u32::from(parent_correct && !candidate_correct);
            scores.improvements += u32::from(!parent_correct && candidate_correct);
        }
    }
    Ok(scores)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> EvaluatorRequest {
        EvaluatorRequest {
            schema_version: 1,
            evaluation_id: "evaluation-1".to_owned(),
            evaluator_id: ArtifactId::for_bytes(b"worker").as_str().to_owned(),
            visible: vec![EvaluatorTrial {
                task_id: "visible".to_owned(),
                expected_output: "yes".to_owned(),
                parent_output: "no".to_owned(),
                candidate_output: "yes".to_owned(),
            }],
            sealed: vec![EvaluatorTrial {
                task_id: "sealed".to_owned(),
                expected_output: "secret".to_owned(),
                parent_output: "secret".to_owned(),
                candidate_output: "wrong".to_owned(),
            }],
        }
    }

    #[test]
    fn response_is_bound_and_contains_only_aggregate_scores() {
        let bytes = serde_json::to_vec(&request()).unwrap();
        let response = evaluate_request(&bytes).unwrap();
        let parsed: EvaluatorResponse = serde_json::from_slice(&response).unwrap();
        assert_eq!(
            parsed.request_artifact_id,
            ArtifactId::for_bytes(&bytes).as_str()
        );
        assert_eq!(parsed.scores.candidate_visible_correct, 1);
        assert_eq!(parsed.scores.parent_sealed_correct, 1);
        let text = String::from_utf8(response).unwrap();
        for forbidden in ["secret", "\"yes\"", "\"no\"", "task_id", "expected_output"] {
            assert!(!text.contains(forbidden));
        }
    }

    #[test]
    fn protocol_rejects_noncanonical_unknown_duplicate_and_unsupported_requests() {
        let canonical = serde_json::to_vec(&request()).unwrap();
        let mut spaced = canonical.clone();
        spaced.push(b' ');
        assert!(matches!(
            evaluate_request(&spaced),
            Err(ArenaError::EvaluatorProtocol(
                "request is not canonical JSON"
            ))
        ));

        let mut unknown = serde_json::to_value(request()).unwrap();
        unknown["unexpected"] = serde_json::json!(true);
        assert!(evaluate_request(&serde_json::to_vec(&unknown).unwrap()).is_err());

        let mut duplicate = request();
        duplicate.sealed[0].task_id = duplicate.visible[0].task_id.clone();
        assert!(matches!(
            evaluate_request(&serde_json::to_vec(&duplicate).unwrap()),
            Err(ArenaError::DuplicateTaskId(_))
        ));

        let mut unsupported = request();
        unsupported.schema_version = 2;
        assert!(matches!(
            evaluate_request(&serde_json::to_vec(&unsupported).unwrap()),
            Err(ArenaError::EvaluatorProtocol("unsupported request schema"))
        ));
    }
}
