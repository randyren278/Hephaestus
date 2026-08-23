# Architecture

The Rust workspace begins with `crates/hephaestus-core/`, which owns laws and domain contracts that must remain outside evolution. Other planes will be added only after their invariants have executable evidence.

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
```

## Trust boundary

The authority and domain modules are treated as production-critical. `checks/checks.json` sets a 95% per-module coverage floor and defines deliberate source mutations for each implemented invariant. The mutation ratchet may only increase.

## Repository map

- `crates/hephaestus-core/` — shared trust primitives.
- `checks/` — coverage, mutation, and documentation gates.
- `.github/workflows/ci.yml` — deterministic and adversarial CI jobs.
- `HEPHAESTUS_MASTER_PLAN.md` — product and engineering specification.
