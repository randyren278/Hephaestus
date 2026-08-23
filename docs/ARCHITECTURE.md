# Architecture

The Rust workspace separates Laws and domain contracts, canonical evidence persistence, immutable Genome/World compilation, the local daemon boundary, capability-scoped runtimes, and the Experience Plane. New planes are added only with executable invariants.

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
    Source[Versioned JSON or YAML] --> Compile[Fail-closed compiler]
    CAS --> Compile
    Vocabulary --> Compile
    Compile --> World[Content-addressed World]
    World --> Genome[Content-addressed Genome]
    Genome --> Derive
    CLI[Operator CLI] -->|token plus schema v1| Socket[Owner-only Unix socket]
    Socket --> Daemon[Single-writer daemon]
    Daemon --> Event
    Replay --> Daemon
    Daemon --> Runtime[Provider-neutral runtime]
    Runtime --> Recorded[Evidence-required runtime wrapper]
    Recorded --> Experience[Redacted provenance-bound traces]
    Experience --> Event
    Runtime --> Sandbox[Pinned private Git worktree]
    Sandbox --> Codex[Codex driver]
    Sandbox --> Claude[Claude driver]
    Sandbox --> Reference[Offline reference runtime]
```

## Trust boundary

The authority, domain, compiler, Genome, World, event-store, artifact-store, runtime, isolation, control, and Experience modules named by `checks/checks.json` are production-critical. The manifest sets a 95% per-module coverage floor and deliberate source mutations for implemented invariants. The mutation ratchet may only increase.

## Repository map

- `crates/hephaestus-core/` — shared trust primitives.
- `crates/hephaestus-control/` — daemon, versioned local API, and operator CLI.
- `crates/hephaestus-experience/` — redacted traces, runtime evidence integration, and provenance-backed experience.
- `crates/hephaestus-genome/` — canonical Genome and World compilers.
- `crates/hephaestus-ledger/` — canonical events and artifacts.
- `crates/hephaestus-runtime/` — capability-scoped worktrees and runtime adapters.
- `checks/` — coverage, mutation, and documentation gates.
- `.github/workflows/ci.yml` — deterministic and adversarial CI jobs.
- `HEPHAESTUS_MASTER_PLAN.md` — product and engineering specification.
