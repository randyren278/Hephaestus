# Hephaestus

Hephaestus is an evidence-first evolutionary control plane for AI agents.

```mermaid
flowchart LR
    Operator --> Authority[Authority laws]
    Authority --> Genome[Agent Genome]
    Genome --> Arena[Protected Arena]
    Arena --> Evidence[Evidence receipt]
    Evidence --> Promotion[Deterministic promotion]
```

The repository now has an executable constitution, durable evidence spine, fail-closed Genome and World compilers, runnable local control plane, provider-neutral runtime substrate, and provenance-backed Experience Plane. `crates/hephaestus-core/` owns authority and domain Laws. `crates/hephaestus-ledger/` owns canonical events and artifacts. `crates/hephaestus-genome/` derives immutable identities. `crates/hephaestus-control/` provides the single-writer daemon and operator CLI. `crates/hephaestus-runtime/` creates private Git worktrees, expiring run-bound capabilities, independently supervised local processes with hard wall/output limits, fail-closed macOS Seatbelt isolation, and inert Codex/Claude invocation contracts. `crates/hephaestus-experience/` wraps runtimes with required redacted lifecycle evidence, bounds observable traces before CAS persistence, then links structured observations, hypotheses, evidence, and contradictions to verified source events. CI deliberately breaks each invariant and requires the suite to detect the regression.

## Development

Install the stable Rust toolchain with the `clippy`, `rustfmt`, and `llvm-tools-preview` components, plus `cargo-llvm-cov`. Then run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features
cargo test --workspace
cargo llvm-cov --workspace --all-features --lcov --output-path lcov.info
python3 checks/coverage_gate.py --manifest checks/checks.json --report lcov.info
python3 checks/mutation_guard.py --manifest checks/checks.json --assert-min 84
python3 checks/docs_gate.py --root . --min-diagrams 1 README.md docs/ARCHITECTURE.md
```

The full product and engineering specification is in [HEPHAESTUS_MASTER_PLAN.md](HEPHAESTUS_MASTER_PLAN.md).

The executable vocabulary is grounded by the [Constitution](docs/CONSTITUTION.md), [Threat Model](docs/THREAT_MODEL.md), [Terminology](docs/TERMINOLOGY.md), and [Evaluation Philosophy](docs/EVALUATION_PHILOSOPHY.md). Runtime behavior is specified in [Runtimes and Sandboxes](docs/RUNTIMES.md), evidence behavior in [Traces and Experience](docs/EXPERIENCE.md), operator behavior in the [Control Plane](docs/CONTROL_PLANE.md), and compiler contracts in [Genomes](docs/GENOMES.md) and [Worlds](docs/WORLDS.md).
