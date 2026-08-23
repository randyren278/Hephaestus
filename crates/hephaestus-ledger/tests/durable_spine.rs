use std::{collections::BTreeMap, fs};

use hephaestus_ledger::{ArtifactId, ArtifactStore, EventInput, EventStore, LedgerError};
use rusqlite::Connection;
use tempfile::tempdir;

fn projection(events: &[hephaestus_ledger::StoredEvent]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for event in events {
        *counts.entry(event.event_type.clone()).or_default() += 1;
    }
    counts
}

fn seed_two_events(database: &std::path::Path) {
    let mut store = EventStore::open(database).expect("open ledger");
    for sequence in 1..=2 {
        store
            .append(EventInput::new(
                format!("event-{sequence}"),
                "genome:g0",
                "observed",
                "runtime",
                sequence * 1_000,
                format!(r#"{{"sequence":{sequence}}}"#).as_bytes(),
            ))
            .expect("append event");
    }
}

#[test]
fn sqlite_event_history_survives_restart_and_replays_deterministically() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("ledger.sqlite3");
    let mut store = EventStore::open(&database).expect("open ledger");

    let first = store
        .append(EventInput::new(
            "event-1",
            "genome:g0",
            "genome_created",
            "operator",
            1_000,
            br#"{"genome":"g0"}"#,
        ))
        .expect("append first event");
    let second = store
        .append(EventInput::new(
            "event-2",
            "genome:g0",
            "genome_validated",
            "arena",
            2_000,
            br#"{"genome":"g0"}"#,
        ))
        .expect("append second event");

    assert_eq!(first.sequence, 1);
    assert_eq!(first.previous_hash, [0; 32]);
    assert_eq!(second.sequence, 2);
    assert_eq!(second.previous_hash, first.hash);
    drop(store);

    let mut reopened = EventStore::open(&database).expect("reopen ledger");
    let third = reopened
        .append(EventInput::new(
            "event-3",
            "genome:g0",
            "genome_promoted",
            "policy",
            3_000,
            br#"{"genome":"g0"}"#,
        ))
        .expect("append after restart");
    assert_eq!(third.sequence, 3);

    let first_replay = reopened.replay_verified().expect("verified replay");
    let rebuilt_projection = projection(&first_replay);
    drop(reopened);

    let reopened = EventStore::open(&database).expect("second reopen");
    let second_replay = reopened.replay_verified().expect("second verified replay");
    assert_eq!(second_replay, first_replay);
    assert_eq!(projection(&second_replay), rebuilt_projection);
}

#[test]
fn event_replay_detects_payload_and_chain_tampering() {
    let directory = tempdir().expect("temporary directory");
    let payload_database = directory.path().join("payload.sqlite3");
    seed_two_events(&payload_database);
    let connection = Connection::open(&payload_database).expect("open direct connection");
    connection
        .execute(
            "UPDATE events SET payload = X'00' WHERE global_sequence = 2",
            [],
        )
        .expect("inject corruption");
    drop(connection);
    assert!(matches!(
        EventStore::open(&payload_database),
        Err(LedgerError::EventHashMismatch { sequence: 2 })
    ));

    let live_database = directory.path().join("live-tamper.sqlite3");
    seed_two_events(&live_database);
    let mut live_store = EventStore::open(&live_database).expect("open verified ledger");
    let connection = Connection::open(&live_database).expect("open direct connection");
    connection
        .execute(
            "UPDATE events SET payload = X'00' WHERE global_sequence = 2",
            [],
        )
        .expect("inject live corruption");
    drop(connection);
    assert!(matches!(
        live_store.append(EventInput::new(
            "event-3",
            "genome:g0",
            "observed",
            "runtime",
            3_000,
            b"{}"
        )),
        Err(LedgerError::LedgerHeadChanged)
    ));

    let chain_database = directory.path().join("chain.sqlite3");
    seed_two_events(&chain_database);
    let connection = Connection::open(&chain_database).expect("open direct connection");
    connection
        .execute(
            "UPDATE events SET previous_hash = zeroblob(32) WHERE global_sequence = 2",
            [],
        )
        .expect("break chain link");
    drop(connection);
    assert!(matches!(
        EventStore::open(&chain_database),
        Err(LedgerError::PreviousHashMismatch { sequence: 2 })
    ));

    let sequence_database = directory.path().join("sequence.sqlite3");
    seed_two_events(&sequence_database);
    let connection = Connection::open(&sequence_database).expect("open direct connection");
    connection
        .execute(
            "UPDATE events SET global_sequence = 3 WHERE global_sequence = 2",
            [],
        )
        .expect("create sequence gap");
    drop(connection);
    assert!(matches!(
        EventStore::open(&sequence_database),
        Err(LedgerError::SequenceMismatch {
            expected: 2,
            actual: 3
        })
    ));

    let malformed_hash_database = directory.path().join("malformed-hash.sqlite3");
    seed_two_events(&malformed_hash_database);
    let connection = Connection::open(&malformed_hash_database).expect("open direct connection");
    connection
        .execute_batch("PRAGMA ignore_check_constraints = ON")
        .expect("allow corruption injection");
    connection
        .execute(
            "UPDATE events SET hash = X'00' WHERE global_sequence = 2",
            [],
        )
        .expect("truncate stored hash");
    drop(connection);
    assert!(matches!(
        EventStore::open(&malformed_hash_database),
        Err(LedgerError::InvalidHashLength(1))
    ));
}

#[test]
fn duplicate_event_ids_are_rejected_without_advancing_history() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("ledger.sqlite3");
    let mut store = EventStore::open(&database).expect("open ledger");
    let input = EventInput::new("event-1", "genome:g0", "created", "operator", 1_000, b"{}");

    store.append(input.clone()).expect("first append");
    assert!(matches!(
        store.append(input),
        Err(LedgerError::DuplicateEventId(id)) if id == "event-1"
    ));
    assert_eq!(store.replay_verified().expect("verified history").len(), 1);
    assert!(matches!(
        store.append(EventInput::new(
            "",
            "genome:g0",
            "created",
            "operator",
            2_000,
            b"{}"
        )),
        Err(LedgerError::EmptyEventField("event_id"))
    ));
    assert_eq!(store.replay_verified().expect("unchanged history").len(), 1);
}

#[test]
fn artifact_store_deduplicates_and_detects_substitution() {
    let directory = tempdir().expect("temporary directory");
    let store = ArtifactStore::open(directory.path().join("blobs")).expect("open artifact store");

    let first = store.put(b"candidate patch").expect("store artifact");
    let duplicate = store.put(b"candidate patch").expect("deduplicate artifact");
    let other = store
        .put(b"evaluation receipt")
        .expect("store other artifact");

    assert_eq!(first, duplicate);
    assert_ne!(first, other);
    assert_eq!(
        ArtifactId::parse(first.as_str()).expect("parse canonical address"),
        first
    );
    assert_eq!(
        store.get(&first).expect("read artifact"),
        b"candidate patch"
    );
    assert!(matches!(
        ArtifactId::parse("NOT-A-HASH"),
        Err(LedgerError::InvalidArtifactId(_))
    ));

    fs::write(store.path_for(&first), b"substituted").expect("inject substitution");
    assert!(matches!(
        store.get(&first),
        Err(LedgerError::ArtifactHashMismatch { .. })
    ));
}
