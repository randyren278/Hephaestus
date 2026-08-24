# Worlds

A World is the versioned root of evaluation semantics. Its content-derived identity includes Laws, the authority ceiling, mutation scope, promotion policy, objectives, and evaluator artifact references. Changing any normalized field creates a different World.

## Compilation contract

The schema 1 compiler rejects candidate evaluator access, mutation scopes containing Laws or evaluators, confidence outside 1 through 10,000 basis points, empty or blank objectives, and missing or corrupted evaluator artifacts. It sorts and deduplicates mutation targets and objectives before producing canonical JSON and `hephaestus:world:<blake3>`.

Evaluator artifacts are verified through the same content-addressed store used by Genomes. Candidates receive identities and policy outcomes, never evaluator bytes or hidden scoring internals.

The current measurement Arena reserves four evaluator artifact names in a compiled World: `arena.visible_manifest`, `arena.sealed_manifest`, `arena.evaluator`, and `arena.runtime_verifier`. It hashes the supplied canonical manifest bytes before any publication and requires each hash to equal the corresponding World commitment. The verifier artifact contains the 32-byte Ed25519 public key authorized to attest runtime results; the signing seed remains daemon-only. The only implemented evaluator payload is `exact-match-evaluator-v1`; any other evaluator bytes fail closed until their semantics have an independently tested implementation.

```mermaid
flowchart LR
    Laws --> Canonical[Canonical World]
    Ceiling[Authority ceiling] --> Canonical
    Scope[Mutation scope] --> Canonical
    Promotion[Promotion policy] --> Canonical
    Evaluator[Verified evaluator CAS] --> Canonical
    Canonical --> Identity[BLAKE3 World ID]
    Identity --> Compare{Same exact ID?}
    Compare -->|yes| Comparable
    Compare -->|no| Reject
```

## Comparability

Results are directly comparable only when their compiled World identities match exactly. A name match is deliberately insufficient: changed Laws, evaluation artifacts, objectives, or promotion semantics begin a new progress line.
