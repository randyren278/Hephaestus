# Evaluation Philosophy

## Evidence before autonomy

Evaluation exists to answer whether a candidate is better under a specific World, not whether it can produce a persuasive explanation. The optimizer is downstream of the objective and cannot modify it.

## Required comparison shape

- Parent and candidate receive paired tasks, environment, budget, provider version, and seed where applicable.
- Visible evaluation supports development; sealed evaluation supports selection.
- Correctness, reliability, cost, latency, tool errors, intervention, recovery, and regressions remain separate dimensions until a World defines utility.
- Stochastic results include repeated trials, uncertainty, and a minimum effect size.
- A higher mean cannot override an invariant regression.

## World boundaries

Evaluator semantics, tasks, Laws, and budget policy are versioned as a World. A material change creates a new World. Cross-World results may be shown side by side but never as a direct progress delta.

## Receipts

Every promotion receipt names the parent, candidate, World, evaluator versions, tasks or sealed-set identity, seeds, environment, cost, latency, regression result, uncertainty, decision policy, and artifact hashes needed to reproduce the claim.

## Deterministic research analysis

`python/hephaestus_lab/statistics.py` is a read-only research layer: it has no ledger or CAS dependency and therefore no canonical write authority. It computes paired correctness deltas with a version-independent seeded bootstrap, retains correctness, reliability, cost, and latency as separate Pareto dimensions, and applies minimum-effect and regression gates. Rust remains responsible for validating any future analysis artifact before persistence or promotion; the Python result is never promotion authority by itself.
