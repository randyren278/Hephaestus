use hephaestus_experience::{
    ExperienceError, RUN_RESULT_SCHEMA_VERSION, RunCompletionReason, RunResultReceipt,
};
use hephaestus_ledger::{ArtifactId, EventStore};
use tempfile::tempdir;

fn receipt() -> RunResultReceipt {
    let output = ArtifactId::for_bytes(b"output").as_str().to_owned();
    let diagnostics = ArtifactId::for_bytes(b"diagnostics").as_str().to_owned();
    let trace = ArtifactId::for_bytes(b"trace").as_str().to_owned();
    RunResultReceipt::new(
        "run_001",
        format!(
            "hephaestus:genome:{}",
            ArtifactId::for_bytes(b"genome").as_str()
        ),
        format!(
            "hephaestus:world:{}",
            ArtifactId::for_bytes(b"world").as_str()
        ),
        "0123456789abcdef0123456789abcdef01234567",
        RunCompletionReason::Success,
        12,
        0,
        output,
        diagnostics,
        vec![trace],
    )
    .expect("valid receipt")
}

fn stored(receipt: RunResultReceipt) -> hephaestus_ledger::StoredEvent {
    let directory = tempdir().expect("temporary ledger");
    let mut ledger =
        EventStore::open(directory.path().join("events.sqlite3")).expect("open ledger");
    ledger
        .append(
            receipt
                .into_event_input(1_700_000_000_000)
                .expect("event input"),
        )
        .expect("append receipt")
}

#[test]
fn canonical_receipt_round_trips_from_derived_event_envelope() {
    let expected = receipt();
    let event = stored(expected.clone());

    assert_eq!(
        RunResultReceipt::parse_from_event(&event).expect("parse canonical receipt"),
        expected
    );
    assert_eq!(expected.schema_version, RUN_RESULT_SCHEMA_VERSION);
    assert_eq!(event.event_id, "result:run_001");
    assert_eq!(event.aggregate_id, "run:run_001");
    assert_eq!(event.actor, "runtime-plane");
    assert_eq!(event.event_type, "run.result_recorded");
}

#[test]
fn parser_rejects_forged_event_envelopes() {
    let forgeries: [fn(&mut hephaestus_ledger::StoredEvent); 4] = [
        |event: &mut hephaestus_ledger::StoredEvent| event.actor = "operator".to_owned(),
        |event: &mut hephaestus_ledger::StoredEvent| event.event_id = "result:other".to_owned(),
        |event: &mut hephaestus_ledger::StoredEvent| event.aggregate_id = "run:other".to_owned(),
        |event: &mut hephaestus_ledger::StoredEvent| event.event_type = "run.started".to_owned(),
    ];
    for forge in forgeries {
        let mut event = stored(receipt());
        forge(&mut event);
        assert!(matches!(
            RunResultReceipt::parse_from_event(&event),
            Err(ExperienceError::InvalidInput(_))
        ));
    }
}

#[test]
fn parser_rejects_unknown_schema_fields_and_invalid_runtime_claims() {
    let canonical = receipt();
    let mut value = serde_json::to_value(&canonical).expect("serialize receipt");
    value["untrusted"] = serde_json::json!(true);
    let mut unknown = stored(canonical.clone());
    unknown.payload = serde_json::to_vec(&value).expect("encode forged payload");
    assert!(matches!(
        RunResultReceipt::parse_from_event(&unknown),
        Err(ExperienceError::Json(_))
    ));

    for (field, replacement) in [
        ("schema_version", serde_json::json!(2)),
        ("run_id", serde_json::json!("../escape")),
        ("actual_cost_microusd", serde_json::json!(1)),
        ("latency_millis", serde_json::json!(10_001)),
        ("stdout_artifact_id", serde_json::json!("not-an-artifact")),
        ("source_revision", serde_json::json!("HEAD")),
    ] {
        let mut value = serde_json::to_value(&canonical).expect("serialize receipt");
        value[field] = replacement;
        let mut event = stored(canonical.clone());
        event.payload = serde_json::to_vec(&value).expect("encode forged payload");
        assert!(
            RunResultReceipt::parse_from_event(&event).is_err(),
            "{field}"
        );
    }

    let mut oversized = stored(canonical);
    oversized.payload = vec![b' '; 131_073];
    assert!(matches!(
        RunResultReceipt::parse_from_event(&oversized),
        Err(ExperienceError::RecordTooLarge {
            actual: 131_073,
            maximum: 131_072
        })
    ));
}
