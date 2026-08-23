use std::fs;

use hephaestus_arena::{
    ArenaError, EvaluationBinding, EvaluationStores, ReceiptContext, Submission, TrustedManifest,
    TrustedTask, Visibility, evaluate_and_record, verify_receipt_artifact,
};
use hephaestus_ledger::{ArtifactId, LedgerError};
use tempfile::TempDir;

fn make_binding(seed: u64) -> EvaluationBinding {
    EvaluationBinding::new(
        format!("hephaestus:world:{}", "a".repeat(64)),
        seed,
        "environment-v1",
        "exact-match-v1",
    )
    .expect("valid binding")
}

fn manifests(
    binding: &EvaluationBinding,
) -> (TrustedManifest, TrustedManifest, &'static str, &'static str) {
    let visible_secret = "visible-expected-never-in-candidate-input";
    let sealed_secret = "sealed-expected-9f67c2";
    let visible = TrustedManifest::new(
        "visible-suite-v1",
        binding.clone(),
        Visibility::Visible,
        vec![
            TrustedTask::new("task-visible-b", "input visible b", "B").expect("task"),
            TrustedTask::new("task-visible-a", "input visible a", visible_secret).expect("task"),
        ],
    )
    .expect("visible manifest");
    let sealed = TrustedManifest::new(
        "sealed-suite-v1",
        binding.clone(),
        Visibility::Sealed,
        vec![
            TrustedTask::new("task-sealed-b", "sealed prompt 517", "Z").expect("task"),
            TrustedTask::new("task-sealed-a", "sealed prompt 204", sealed_secret).expect("task"),
        ],
    )
    .expect("sealed manifest");
    (visible, sealed, visible_secret, sealed_secret)
}

fn submission(
    id: &str,
    binding: &EvaluationBinding,
    visible_a: &str,
    visible_b: &str,
    sealed_a: &str,
    sealed_b: &str,
) -> Submission {
    Submission::new(
        id,
        binding.clone(),
        [
            ("task-sealed-b".to_owned(), sealed_b.to_owned()),
            ("task-visible-a".to_owned(), visible_a.to_owned()),
            ("task-sealed-a".to_owned(), sealed_a.to_owned()),
            ("task-visible-b".to_owned(), visible_b.to_owned()),
        ],
    )
    .expect("valid submission")
}

fn context() -> ReceiptContext {
    ReceiptContext {
        event_id: "evaluation-event-001".to_owned(),
        evaluation_id: "evaluation-001".to_owned(),
        caller_id: "arena-test".to_owned(),
        timestamp_millis: 1_788_000_123_456,
    }
}

fn stores(directory: &TempDir) -> EvaluationStores {
    EvaluationStores::open(
        directory.path().join("events.sqlite3"),
        directory.path().join("blobs"),
    )
    .expect("open stores")
}

#[test]
fn records_one_reproducible_sealed_safe_evaluation() {
    let binding = make_binding(42);
    let (visible, sealed, visible_secret, sealed_secret) = manifests(&binding);
    let public_tasks = visible.candidate_tasks().expect("visible inputs");
    let public_json = serde_json::to_string(&public_tasks).expect("public JSON");
    assert!(!public_json.contains(visible_secret));
    assert!(!public_json.contains(sealed_secret));
    assert!(matches!(
        sealed.candidate_tasks(),
        Err(ArenaError::VisibilityMismatch)
    ));

    let parent = submission(
        "parent-1",
        &binding,
        visible_secret,
        "wrong",
        sealed_secret,
        "Z",
    );
    let candidate = submission("candidate-1", &binding, visible_secret, "B", "wrong", "Z");
    let first_dir = TempDir::new().expect("temporary directory");
    let first = evaluate_and_record(
        stores(&first_dir),
        context(),
        &binding,
        &visible,
        &sealed,
        &parent,
        &candidate,
    )
    .expect("record evaluation");

    assert_eq!(first.event.sequence, 1);
    assert_eq!(first.event.event_type, "evaluation.recorded");
    assert_eq!(first.receipt.caller_id, "arena-test");
    assert_eq!(first.receipt.timestamp_millis, 1_788_000_123_456);
    assert_eq!(
        first.event.payload,
        serde_json::to_vec(&first.receipt).unwrap()
    );
    assert_eq!(first.receipt.scores.parent_visible_correct, 1);
    assert_eq!(first.receipt.scores.candidate_visible_correct, 2);
    assert_eq!(first.receipt.scores.parent_sealed_correct, 2);
    assert_eq!(first.receipt.scores.candidate_sealed_correct, 1);
    assert_eq!(first.receipt.scores.regressions, 1);
    assert_eq!(first.receipt.scores.improvements, 1);

    let receipt_json = serde_json::to_string(&first.receipt).expect("receipt JSON");
    let event_json = String::from_utf8(first.event.payload.clone()).expect("event UTF-8");
    for secret in [
        visible_secret,
        sealed_secret,
        "sealed prompt 204",
        "sealed prompt 517",
    ] {
        assert!(!receipt_json.contains(secret));
        assert!(!event_json.contains(secret));
    }

    let visible_inputs = verify_receipt_artifact(
        &first.stores.artifacts,
        &first.receipt.visible_inputs_artifact_id,
    )
    .expect("verified visible inputs");
    let visible_manifest = verify_receipt_artifact(
        &first.stores.artifacts,
        &first.receipt.visible_manifest_artifact_id,
    )
    .expect("verified visible manifest");
    for bytes in [&visible_inputs, &visible_manifest] {
        let text = String::from_utf8(bytes.clone()).expect("artifact UTF-8");
        assert!(!text.contains(sealed_secret));
        assert!(!text.contains("sealed prompt"));
    }
    let sealed_manifest = verify_receipt_artifact(
        &first.stores.artifacts,
        &first.receipt.sealed_manifest_artifact_id,
    )
    .expect("verified sealed manifest");
    assert!(
        String::from_utf8(sealed_manifest)
            .unwrap()
            .contains(sealed_secret)
    );
    assert_eq!(first.stores.events.replay_verified().unwrap().len(), 1);

    let second_dir = TempDir::new().expect("temporary directory");
    let second = evaluate_and_record(
        stores(&second_dir),
        context(),
        &binding,
        &visible,
        &sealed,
        &parent,
        &candidate,
    )
    .expect("repeat evaluation");
    assert_eq!(first.receipt, second.receipt);
    assert_eq!(first.event, second.event);
}

#[test]
fn rejects_binding_manifest_and_task_set_mismatches_before_writing() {
    let binding = make_binding(42);
    let other_binding = make_binding(43);
    let (visible, sealed, visible_secret, sealed_secret) = manifests(&binding);
    let parent = submission(
        "parent-1",
        &binding,
        visible_secret,
        "B",
        sealed_secret,
        "Z",
    );
    let candidate = submission(
        "candidate-1",
        &other_binding,
        visible_secret,
        "B",
        sealed_secret,
        "Z",
    );
    let directory = TempDir::new().expect("temporary directory");
    let failure = evaluate_and_record(
        stores(&directory),
        context(),
        &binding,
        &visible,
        &sealed,
        &parent,
        &candidate,
    );
    assert!(matches!(
        failure,
        Err(ArenaError::BindingMismatch("candidate_submission"))
    ));
    assert!(
        hephaestus_ledger::EventStore::open(directory.path().join("events.sqlite3"))
            .unwrap()
            .replay_verified()
            .unwrap()
            .is_empty()
    );

    let incomplete = Submission::new(
        "candidate-2",
        binding.clone(),
        [("task-visible-a".to_owned(), visible_secret.to_owned())],
    )
    .unwrap();
    let directory = TempDir::new().expect("temporary directory");
    let failure = evaluate_and_record(
        stores(&directory),
        context(),
        &binding,
        &visible,
        &sealed,
        &parent,
        &incomplete,
    );
    assert!(matches!(failure, Err(ArenaError::TaskSetMismatch { .. })));

    let wrong_visibility = TrustedManifest::new(
        "wrong-slot",
        binding.clone(),
        Visibility::Sealed,
        vec![TrustedTask::new("different-task", "input", "expected").unwrap()],
    )
    .unwrap();
    let directory = TempDir::new().expect("temporary directory");
    let failure = evaluate_and_record(
        stores(&directory),
        context(),
        &binding,
        &wrong_visibility,
        &sealed,
        &parent,
        &incomplete,
    );
    assert!(matches!(failure, Err(ArenaError::VisibilityMismatch)));

    let duplicate_id_sealed = TrustedManifest::new(
        "visible-suite-v1",
        binding.clone(),
        Visibility::Sealed,
        vec![TrustedTask::new("other-task", "hidden", "expected").unwrap()],
    )
    .unwrap();
    let directory = TempDir::new().expect("temporary directory");
    let failure = evaluate_and_record(
        stores(&directory),
        context(),
        &binding,
        &visible,
        &duplicate_id_sealed,
        &parent,
        &incomplete,
    );
    assert!(matches!(failure, Err(ArenaError::DuplicateManifestId(_))));
}

#[test]
fn rejects_malformed_and_duplicate_identities() {
    assert!(matches!(
        EvaluationBinding::new("world:no", 1, "environment", "evaluator"),
        Err(ArenaError::InvalidId {
            field: "world_id",
            ..
        })
    ));
    assert!(matches!(
        TrustedTask::new("not valid", "input", "expected"),
        Err(ArenaError::InvalidId {
            field: "task_id",
            ..
        })
    ));

    let binding = make_binding(42);
    let duplicate_manifest = TrustedManifest::new(
        "duplicate-tasks",
        binding.clone(),
        Visibility::Visible,
        vec![
            TrustedTask::new("same-task", "a", "a").unwrap(),
            TrustedTask::new("same-task", "b", "b").unwrap(),
        ],
    );
    assert!(matches!(
        duplicate_manifest,
        Err(ArenaError::DuplicateTaskId(id)) if id == "same-task"
    ));

    let duplicate_submission = Submission::new(
        "submission",
        binding,
        [
            ("same-task".to_owned(), "a".to_owned()),
            ("same-task".to_owned(), "b".to_owned()),
        ],
    );
    assert!(matches!(
        duplicate_submission,
        Err(ArenaError::DuplicateTaskId(id)) if id == "same-task"
    ));
}

#[test]
fn artifact_verification_detects_tampering() {
    let binding = make_binding(42);
    let (visible, sealed, visible_secret, sealed_secret) = manifests(&binding);
    let parent = submission(
        "parent-1",
        &binding,
        visible_secret,
        "B",
        sealed_secret,
        "Z",
    );
    let candidate = submission(
        "candidate-1",
        &binding,
        visible_secret,
        "B",
        sealed_secret,
        "Z",
    );
    let directory = TempDir::new().expect("temporary directory");
    let recorded = evaluate_and_record(
        stores(&directory),
        context(),
        &binding,
        &visible,
        &sealed,
        &parent,
        &candidate,
    )
    .expect("record evaluation");
    let id = ArtifactId::parse(&recorded.receipt.visible_inputs_artifact_id).unwrap();
    fs::write(recorded.stores.artifacts.path_for(&id), b"tampered").unwrap();
    assert!(matches!(
        verify_receipt_artifact(&recorded.stores.artifacts, id.as_str()),
        Err(ArenaError::Ledger(LedgerError::ArtifactHashMismatch { .. }))
    ));
}
