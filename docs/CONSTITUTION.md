# Hephaestus Constitution

Version 1 defines the trust rules that every runtime, storage backend, evaluator, and interface must obey. Product behavior may evolve only inside these boundaries.

## Purpose

Hephaestus exists to prove that an agent harness became better. A candidate's claim, a higher unpaired score, or a successful self-modification is not proof. Improvement requires attributable observations, an explicit hypothesis, protected evaluation, deterministic selection, and a reproducible evidence receipt.

## The Laws

1. Released Genomes and their content-addressed artifacts are immutable.
2. A child may inherit equal or narrower authority, never broader authority.
3. Candidates cannot modify Laws, evaluators, sealed holdouts, ledger integrity, budget enforcement, promotion policy, rollback, artifact hashes, World definitions, or operator controls.
4. Models may recommend promotion but cannot execute it.
5. Every accepted state transition is ledgered and replayable.
6. Incompatible Worlds are never presented as directly comparable.
7. Freeze and kill remain under external operator control.
8. Malformed or unsupported requests fail closed.

The first executable forms of these Laws are in `crates/hephaestus-core/src/authority.rs` and `crates/hephaestus-core/src/domain.rs`. CI deliberately weakens them and requires the suite to fail.

## Evolvable surface

Prompts, model routing, context, memory, retrieval, tools, planning, topology, verification, recovery, budgets below their ceilings, and the improvement strategy may evolve when a World permits it.

## Change control

A change to a Law creates a new constitutional and World version. Historical evidence remains attached to its original version. No migration may rewrite old events to make them appear governed by new Laws.
