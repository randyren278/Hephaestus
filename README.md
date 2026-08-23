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

The repository now has an executable constitution plus a durable evidence spine. `crates/hephaestus-core/` owns fail-closed authority and domain Laws. `crates/hephaestus-ledger/` owns a WAL-backed, hash-linked SQLite event stream and a BLAKE3 content-addressed artifact store. CI deliberately breaks each invariant and requires the suite to detect the regression.

## Development

Install the stable Rust toolchain with the `clippy`, `rustfmt`, and `llvm-tools-preview` components, plus `cargo-llvm-cov`. Then run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features
cargo test --workspace
cargo llvm-cov --workspace --all-features --lcov --output-path lcov.info
python3 checks/coverage_gate.py --manifest checks/checks.json --report lcov.info
python3 checks/mutation_guard.py --manifest checks/checks.json --assert-min 16
python3 checks/docs_gate.py --root . --min-diagrams 1 README.md docs/ARCHITECTURE.md
```

The full product and engineering specification is in [HEPHAESTUS_MASTER_PLAN.md](HEPHAESTUS_MASTER_PLAN.md).

The executable vocabulary is grounded by the [Constitution](docs/CONSTITUTION.md), [Threat Model](docs/THREAT_MODEL.md), [Terminology](docs/TERMINOLOGY.md), and [Evaluation Philosophy](docs/EVALUATION_PHILOSOPHY.md).
