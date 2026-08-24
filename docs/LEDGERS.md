# Ledgers and Artifacts

## Canonical event spine

`crates/hephaestus-ledger/src/event_store.rs` owns the first canonical ledger. SQLite runs in WAL mode with full synchronous durability. Each append uses an immediate transaction, rejects duplicate identifiers and empty canonical fields, compares the database tail with the in-memory verified head, assigns the next global sequence, and hashes every event field together with the predecessor hash.

Opening a store verifies the complete chain before accepting writes. Normal appends compare only the tail, avoiding quadratic ingestion while enforcing the single-writer contract. A restart replays and verifies all history before reconstructing the head.

## Deterministic replay

Replay requires contiguous sequences, exact predecessor links, 32-byte hashes, and a recomputed BLAKE3 event hash. Derived projections are disposable: the integration suite rebuilds a projection twice from the on-disk ledger and requires identical state.

## Artifact CAS

`crates/hephaestus-ledger/src/artifact_store.rs` addresses bytes by canonical lowercase BLAKE3 hash and shards them by the first two hexadecimal characters. Writes use a create-new temporary file, file fsync, atomic rename, and directory fsync. Reads always recompute the address and fail on substitution.

The CAS root is a daemon-owned trust boundary. Before an external API exists, the daemon milestone must create it with owner-only permissions and prevent candidate sandboxes from accessing it.

## Arena evaluation evidence

Arena consumes only authenticated `run.result_recorded` events from verified history. Its evaluator-owned submission evidence commits each signed run event identity and hash, source revision, completion reason, latency, actual cost, and bounded output, diagnostic, and trace artifact identities. The final operator receipt binds those submissions to the exact World, seed, environment, evaluator, hard budget, parent and candidate Genomes, and aggregate scores. The resulting evaluation event hash commits to that exact receipt.

An `OperatorEvaluation` may derive aggregate `SelectionEvidence` from that detailed receipt. The aggregate deliberately removes task identities and order, inputs, expectations, outputs, and artifact addresses while retaining the event binding, correctness outcomes, reliability, cost, and latency needed by the future deterministic selection engine. It is an operator capability, not a replacement for the evaluator-owned receipt or a promotion decision.

## Current failure evidence

Integration tests use actual on-disk SQLite and artifact directories. They reopen after writes, inject payload, link, sequence, and artifact corruption through independent filesystem/database handles, and require deterministic integrity errors. No database or artifact store is mocked.
