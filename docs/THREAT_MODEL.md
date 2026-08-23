# Threat Model

## Assets

Hephaestus protects canonical ledger history, Genome and World identity, sealed evaluators, artifact integrity, operator authority, credentials, budgets, promotion state, rollback state, and the confidentiality of candidate siblings.

## Trust boundaries

- The operator boundary authenticates people and issues short-lived authority proofs.
- The daemon is the sole canonical writer.
- Candidate runtimes and their outputs are untrusted.
- Repositories, tool results, retrieved experience, model output, and remote workers are untrusted inputs.
- Evaluators and sealed holdouts are isolated from candidates.
- The artifact store is verified by content hash on every read.

## STRIDE analysis

| Category | Concrete threat | Required control |
|---|---|---|
| Spoofing | A candidate labels itself as the operator and clears freeze | Operator actions require an authority-root-issued proof; caller-supplied role labels are insufficient |
| Tampering | A candidate edits a World, evaluator, event, or artifact to improve its score | Deny candidate write access; hash-link events; content-address artifacts; verify hashes before use |
| Repudiation | A worker denies a tool call, denial, cost, or promotion recommendation | Ledger actor, capability, input/output hashes, parent event, and decision receipt |
| Information disclosure | A candidate reads sealed tasks, sibling workspaces, credentials, or raw secrets in traces | Separate sandboxes and evaluator identities; no ambient credentials; redact before persistence |
| Denial of service | Unbounded input, recursion, retries, processes, tokens, or disk exhausts the daemon | Bound request size, concurrency, time, tokens, cost, output, and storage; kill descendants on expiry |
| Elevation of privilege | A child widens capabilities or reuses an expired token | Parent-subset derivation, scoped opaque tokens, expiry, audience binding, and deny-by-default checks |

## Current implementation review

The initial capability subset check is fail-closed. The original freeze API accepted a freely constructible role enum; the item-1 implementation replaces that design with a non-loggable opaque operator token matched against canonical state. Transport authentication, token generation, expiry, and request-size enforcement remain mandatory before the daemon exposes an API.

## Security acceptance

Every critical control path must have positive, negative, and mutation tests. Later milestones add sandbox escape, evaluator leakage, budget bypass, event tamper, corruption, stale-token, sibling-isolation, and partial-promotion fault tests.
