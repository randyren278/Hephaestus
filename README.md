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

The repository is currently establishing its trust substrate. The first implemented laws live in `crates/hephaestus-core/`: descendants cannot widen inherited capabilities, and candidates cannot clear an operator freeze. CI deliberately breaks each law and requires the test suite to detect the regression.

## Development

Install the stable Rust toolchain with the `clippy`, `rustfmt`, and `llvm-tools-preview` components, plus `cargo-llvm-cov`. Then run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features
cargo test --workspace
cargo llvm-cov --workspace --all-features --lcov --output-path lcov.info
python3 checks/coverage_gate.py --manifest checks/checks.json --report lcov.info
python3 checks/mutation_guard.py --manifest checks/checks.json --assert-min 2
python3 checks/docs_gate.py --root . --min-diagrams 1 README.md docs/ARCHITECTURE.md
```

The full product and engineering specification is in [HEPHAESTUS_MASTER_PLAN.md](HEPHAESTUS_MASTER_PLAN.md).
