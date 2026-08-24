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

The current Rust Arena is a measurement-only precursor to that promotion receipt. Its task plans contain only signed `run.result_recorded` event IDs. The compiled World commits the runtime-producer public key, visible and sealed manifests, and evaluator executable bytes. Arena rehydrates manifest artifacts only when their JSON uses the supported schema, survives every trusted constructor check, has the expected visibility, and round-trips to the exact canonical bytes. Its operator scheduling view provides task IDs and inputs for both visible and sealed trials through a type that has no expected-output field; the candidate-facing view remains visible-only. Arena authenticates each result, derives its Genome/output provenance, hash-verifies every referenced CAS artifact, and requires successful same-World trials whose task ID, exact input commitment, seed, environment, complete budget tuple, and paired source revision match. It then sends one canonical, bounded request to the World-bound exact-match evaluator in a separate offline `IsolatedWorker`; the strict aggregate-only response must commit to those exact request bytes. The worker executable is re-hashed before every launch, and every terminal path removes its private execution root. `RecordedEvaluation` is the candidate-facing result and contains only the visible `EvaluationSummary` plus payload-free event metadata; it deliberately omits the operator ledger hash because that hash commits to sealed receipt fields. Raw stores, sealed aggregates, task-level evaluator data, and artifact-backed operator evidence remain exclusively on the separate `OperatorEvaluation` capability wrapper. Evaluation event identity and actor are deterministic, identical retries are idempotent across restart, and conflicting reuse fails before artifact publication.

The trusted scheduler now loads the World-bound visible and sealed tasks itself, pins one source revision, and launches every parent/candidate trial through separate supervised offline processes and private worktrees before invoking the isolated evaluator. The typed paired API receives only evaluation and Genome identities, and identical retries reuse the authenticated run/evaluation events. This proves the deterministic local protected-execution path, but it does not yet establish promotion eligibility: Arena still needs complete selection evidence and a statistical selection receipt. Signing and process separation protect the local candidate boundary; they do not cover a compromised daemon, stolen producer key, malicious operator, or same-UID/root access to the protected data directory.

## Deterministic research analysis

`python/hephaestus_lab/statistics.py` is a read-only research layer: it has no ledger or CAS dependency and therefore no canonical write authority. It computes paired correctness deltas with a version-independent seeded bootstrap, retains correctness, reliability, cost, and latency as separate Pareto dimensions, and applies minimum-effect and regression gates. Rust remains responsible for validating any future analysis artifact before persistence or promotion; the Python result is never promotion authority by itself.
