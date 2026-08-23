use std::collections::BTreeMap;

use hephaestus_experience::{
    EvidenceRecorder, ExperienceError, ExperienceInput, ExperienceKind, Provenance,
    RedactionPolicy, RetentionLimits, TraceInput, TraceKind,
};
use hephaestus_ledger::{EventInput, EventStore};
use tempfile::tempdir;

#[test]
fn traces_cover_observable_runtime_events_and_redact_before_persistence() {
    let directory = tempdir().expect("evidence directory");
    let mut recorder = recorder(&directory, 32, 16_384);
    let provenance = provenance();
    let kinds = [
        TraceKind::LifecycleStarted,
        TraceKind::LifecycleResumed,
        TraceKind::LifecycleCompleted,
        TraceKind::ToolCalled,
        TraceKind::ToolResult,
        TraceKind::ContextComposed,
        TraceKind::MemoryRetrieved,
        TraceKind::SubagentSpawned,
        TraceKind::FileRead,
        TraceKind::FileChanged,
        TraceKind::TestExecuted,
        TraceKind::CapabilityDenied,
        TraceKind::CostObserved,
        TraceKind::CheckpointCreated,
        TraceKind::Error,
        TraceKind::Retry,
        TraceKind::ModelResponse,
    ];

    for (index, kind) in kinds.into_iter().enumerate() {
        let mut fields = BTreeMap::from([
            ("summary".to_owned(), format!("event {index}")),
            ("api_token".to_owned(), "unguarded-token".to_owned()),
            ("diagnostic".to_owned(), "known-secret".to_owned()),
            (
                "provider_output".to_owned(),
                "Bearer provider-token and sk-inline-token".to_owned(),
            ),
        ]);
        if kind == TraceKind::LifecycleCompleted {
            fields.insert("completion_reason".to_owned(), "success".to_owned());
        }
        let receipt = recorder
            .record_trace(
                TraceInput::new(
                    format!("trace-{index}"),
                    provenance.clone(),
                    kind,
                    i64::try_from(index).expect("trace index fits i64"),
                    fields,
                )
                .expect("trace input"),
            )
            .expect("record trace");
        assert_eq!(receipt.provenance, provenance);
        assert_eq!(receipt.redacted_fields, 3);
        let artifact = recorder
            .artifact(&receipt.artifact_id)
            .expect("trace artifact");
        let text = String::from_utf8(artifact).expect("UTF-8 trace artifact");
        assert!(!text.contains("known-secret"));
        assert!(!text.contains("unguarded-token"));
        assert!(!text.contains("provider-token"));
        assert!(!text.contains("sk-inline-token"));
        assert!(text.contains("[REDACTED]"));
    }

    let history = recorder.replay_verified().expect("verified trace history");
    assert_eq!(history.len(), 17);
    assert!(history.iter().all(|event| {
        event.event_type == "trace.recorded"
            && event.aggregate_id == "run:run-1"
            && !String::from_utf8_lossy(&event.payload).contains("known-secret")
    }));
}

#[test]
fn retention_and_record_size_limits_survive_restart() {
    let directory = tempdir().expect("evidence directory");
    let input = || {
        TraceInput::new(
            "trace-one",
            provenance(),
            TraceKind::LifecycleStarted,
            1,
            BTreeMap::new(),
        )
        .expect("trace input")
    };
    {
        let mut recorder = recorder(&directory, 1, 1_024);
        recorder.record_trace(input()).expect("first trace");
    }
    let mut reopened = recorder(&directory, 1, 1_024);
    assert!(matches!(
        reopened.record_trace(
            TraceInput::new(
                "trace-two",
                provenance(),
                TraceKind::Retry,
                2,
                BTreeMap::new()
            )
            .expect("second trace input")
        ),
        Err(ExperienceError::RetentionExceeded { maximum: 1 })
    ));
    assert!(matches!(
        reopened.record_experience(
            ExperienceInput::new(
                "experience-over-limit",
                provenance(),
                ExperienceKind::Observation,
                2,
                vec!["trace-one".to_owned()],
                Vec::new(),
                5_000,
                BTreeMap::new()
            )
            .expect("experience over limit input")
        ),
        Err(ExperienceError::RetentionExceeded { maximum: 1 })
    ));

    let other = tempdir().expect("oversize evidence directory");
    let mut bounded = recorder(&other, 2, 128);
    assert!(matches!(
        bounded.record_trace(
            TraceInput::new(
                "oversize",
                provenance(),
                TraceKind::ModelResponse,
                3,
                BTreeMap::from([("response".to_owned(), "x".repeat(1_000))])
            )
            .expect("oversize input")
        ),
        Err(ExperienceError::RecordTooLarge { .. })
    ));
    assert!(bounded.replay_verified().expect("empty history").is_empty());
}

#[test]
fn experiences_require_verified_sources_and_keep_contradictions_explicit() {
    let directory = tempdir().expect("evidence directory");
    let mut recorder = recorder(&directory, 10, 16_384);
    let first = recorder
        .record_trace(trace("trace-source-1", 1))
        .expect("first source trace");
    recorder
        .record_trace(trace("trace-source-2", 2))
        .expect("second source trace");

    let observation = recorder
        .record_experience(
            ExperienceInput::new(
                "experience-observation",
                provenance(),
                ExperienceKind::Observation,
                3,
                vec!["trace-source-1".to_owned()],
                vec![first.artifact_id.clone()],
                8_000,
                BTreeMap::from([
                    (
                        "observation".to_owned(),
                        "tool retries increased".to_owned(),
                    ),
                    ("password".to_owned(), "known-secret".to_owned()),
                ]),
            )
            .expect("observation input"),
        )
        .expect("record observation");
    assert_eq!(observation.status, "unverified");
    assert_eq!(observation.redacted_fields, 1);
    assert!(
        !String::from_utf8(
            recorder
                .artifact(&observation.artifact_id)
                .expect("experience artifact")
        )
        .expect("UTF-8 experience artifact")
        .contains("known-secret")
    );

    let contradiction = recorder
        .record_experience(
            ExperienceInput::new(
                "experience-contradiction",
                provenance(),
                ExperienceKind::Contradiction,
                4,
                vec![
                    "experience-observation".to_owned(),
                    "trace-source-2".to_owned(),
                ],
                Vec::new(),
                6_000,
                BTreeMap::from([(
                    "conflict".to_owned(),
                    "retry evidence disagrees across tasks".to_owned(),
                )]),
            )
            .expect("contradiction input"),
        )
        .expect("record contradiction");
    assert_eq!(contradiction.status, "unverified");
    assert_eq!(contradiction.source_event_ids.len(), 2);

    assert!(matches!(
        recorder.record_experience(
            ExperienceInput::new(
                "unknown-source",
                provenance(),
                ExperienceKind::Hypothesis,
                5,
                vec!["missing-event".to_owned()],
                Vec::new(),
                5_000,
                BTreeMap::new()
            )
            .expect("unknown source input")
        ),
        Err(ExperienceError::UnknownSourceEvent(source)) if source == "missing-event"
    ));
    assert!(
        recorder
            .record_experience(
                ExperienceInput::new(
                    "missing-evidence",
                    provenance(),
                    ExperienceKind::Evidence,
                    6,
                    vec!["trace-source-1".to_owned()],
                    vec!["0".repeat(64)],
                    5_000,
                    BTreeMap::new()
                )
                .expect("missing evidence input")
            )
            .is_err()
    );
}

#[test]
fn experience_cannot_claim_provenance_different_from_its_source() {
    let directory = tempdir().expect("evidence directory");
    let mut recorder = recorder(&directory, 10, 16_384);
    recorder
        .record_trace(
            TraceInput::new(
                "other-provenance",
                Provenance::new("run-2", "genome-2", "world-2").expect("other provenance"),
                TraceKind::Error,
                1,
                BTreeMap::new(),
            )
            .expect("other provenance trace"),
        )
        .expect("record other provenance trace");
    assert!(matches!(
        recorder.record_experience(
            ExperienceInput::new(
                "forged-provenance",
                provenance(),
                ExperienceKind::Hypothesis,
                2,
                vec!["other-provenance".to_owned()],
                Vec::new(),
                5_000,
                BTreeMap::new()
            )
            .expect("forged provenance input")
        ),
        Err(ExperienceError::InvalidInput(_))
    ));
}

#[test]
fn experience_cannot_cite_unrelated_ledger_events() {
    let directory = tempdir().expect("evidence directory");
    let database = directory.path().join("events.sqlite3");
    {
        let mut events = EventStore::open(&database).expect("open event store");
        events
            .append(EventInput::new(
                "control-source",
                "control:global",
                "control.freeze",
                "operator",
                1,
                br#"{"reason":"maintenance"}"#,
            ))
            .expect("append unrelated event");
    }

    let mut recorder = recorder(&directory, 10, 16_384);
    assert!(matches!(
        recorder.record_experience(
            ExperienceInput::new(
                "unrelated-source",
                provenance(),
                ExperienceKind::Hypothesis,
                2,
                vec!["control-source".to_owned()],
                Vec::new(),
                5_000,
                BTreeMap::new(),
            )
            .expect("unrelated source input")
        ),
        Err(ExperienceError::InvalidInput(_))
    ));
}

#[test]
fn invalid_provenance_experience_shapes_and_limits_fail_closed() {
    assert!(Provenance::new("", "genome", "world").is_err());
    assert!(Provenance::new("run", "g".repeat(257), "world").is_err());
    let valid = provenance();
    assert_eq!(valid.run_id(), "run-1");
    assert_eq!(valid.genome_id(), "genome-1");
    assert_eq!(valid.world_id(), "world-1");
    assert!(RetentionLimits::new(0, 1).is_err());
    assert!(RetentionLimits::new(1, 0).is_err());
    assert!(TraceInput::new("", provenance(), TraceKind::Error, 1, BTreeMap::new()).is_err());
    let too_many_fields: BTreeMap<_, _> = (0..65)
        .map(|index| (format!("field-{index}"), String::new()))
        .collect();
    assert!(
        TraceInput::new(
            "too-many-fields",
            provenance(),
            TraceKind::Error,
            1,
            too_many_fields
        )
        .is_err()
    );
    assert!(
        TraceInput::new(
            "empty-field-key",
            provenance(),
            TraceKind::Error,
            1,
            BTreeMap::from([(String::new(), "value".to_owned())])
        )
        .is_err()
    );
    assert!(
        ExperienceInput::new(
            "experience",
            provenance(),
            ExperienceKind::Observation,
            1,
            Vec::new(),
            Vec::new(),
            1,
            BTreeMap::new()
        )
        .is_err()
    );
    assert!(
        ExperienceInput::new(
            "contradiction",
            provenance(),
            ExperienceKind::Contradiction,
            1,
            vec!["only-one".to_owned()],
            Vec::new(),
            1,
            BTreeMap::new()
        )
        .is_err()
    );
    assert!(
        ExperienceInput::new(
            "confidence",
            provenance(),
            ExperienceKind::Hypothesis,
            1,
            vec!["source".to_owned()],
            Vec::new(),
            0,
            BTreeMap::new()
        )
        .is_err()
    );
    assert!(
        ExperienceInput::new(
            "",
            provenance(),
            ExperienceKind::Hypothesis,
            1,
            vec!["source".to_owned()],
            Vec::new(),
            1,
            BTreeMap::new()
        )
        .is_err()
    );
    assert!(
        ExperienceInput::new(
            "blank-source",
            provenance(),
            ExperienceKind::Hypothesis,
            1,
            vec![String::new()],
            Vec::new(),
            1,
            BTreeMap::new()
        )
        .is_err()
    );
}

fn recorder(
    directory: &tempfile::TempDir,
    maximum_records: usize,
    maximum_bytes: usize,
) -> EvidenceRecorder {
    EvidenceRecorder::open(
        directory.path().join("events.sqlite3"),
        directory.path().join("artifacts"),
        RedactionPolicy::new(["known-secret".to_owned()]),
        RetentionLimits::new(maximum_records, maximum_bytes).expect("retention limits"),
    )
    .expect("open evidence recorder")
}

fn provenance() -> Provenance {
    Provenance::new("run-1", "genome-1", "world-1").expect("provenance")
}

fn trace(event_id: &str, timestamp_millis: i64) -> TraceInput {
    TraceInput::new(
        event_id,
        provenance(),
        TraceKind::Error,
        timestamp_millis,
        BTreeMap::from([("error".to_owned(), "injected failure".to_owned())]),
    )
    .expect("trace input")
}
