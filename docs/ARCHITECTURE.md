# Architecture

The Rust workspace begins with `crates/hephaestus-core/`, which owns Laws and domain contracts, and `crates/hephaestus-ledger/`, which owns canonical evidence persistence. Other planes will be added only after their invariants have executable evidence.

```mermaid
flowchart TD
    Candidate[Candidate Genome] --> Derive[Derive child capabilities]
    Derive -->|subset| Run[Authorized run]
    Derive -->|widening| Deny[Fail closed]
    Candidate --> Unfreeze[Request unfreeze]
    Unfreeze --> Deny
    Operator[External operator] --> Unfreeze
    Unfreeze -->|operator only| Resume[Resume evolution]
    Fixture[Versioned domain fixture] --> Validate[Fail-closed validation]
    Validate --> Vocabulary[Canonical vocabulary]
    Vocabulary --> Lifecycle[Declared lifecycle]
    Lifecycle --> Event[Hash-linked event]
    Event --> SQLite[(SQLite WAL)]
    Event --> CAS[BLAKE3 artifact CAS]
    SQLite --> Replay[Verified replay]
    CAS --> Replay
```

## Trust boundary

The authority, domain, event-store, and artifact-store modules are production-critical. `checks/checks.json` sets a 95% per-module coverage floor and defines deliberate source mutations for each implemented invariant. The mutation ratchet may only increase.

## Repository map

- `crates/hephaestus-core/` — shared trust primitives.
- `crates/hephaestus-ledger/` — canonical events and artifacts.
- `checks/` — coverage, mutation, and documentation gates.
- `.github/workflows/ci.yml` — deterministic and adversarial CI jobs.
- `HEPHAESTUS_MASTER_PLAN.md` — product and engineering specification.
