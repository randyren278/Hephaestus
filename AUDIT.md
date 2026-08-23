# Hephaestus Feature Audit

Audit date: 2026-08-24
Branch: **full-reign/2026-08-23**
Baseline commit: `a48c4a5`

## Baseline evidence

- `cargo test --workspace`: 2 behavior tests passed.
- `cargo clippy --workspace --all-targets --all-features`: passed with all and pedantic lints denied.
- `cargo llvm-cov --workspace --all-features --lcov --output-path lcov.info`: authority module reported 100% branch coverage.
- `python3 checks/mutation_guard.py --manifest checks/checks.json --assert-min 2`: 2 mutations killed; 0 survived, stale, or timed out.
- `python3 checks/docs_gate.py --root . --min-diagrams 1 README.md docs/ARCHITECTURE.md`: passed.
- GitHub Actions run `32648979874`: deterministic and mutation jobs passed on the remote.

## Baseline feature inventory

| Feature | Wired | Works | Tested | Guarded | Documented | Verdict |
|---|---|---|---|---|---|---|
| Rust workspace and `hephaestus-core` library | Yes, through Cargo and integration tests; no runnable product entry point exists | Yes; build, test, format, and Clippy were executed | Yes; compilation and integration suite | Indirectly; both implemented laws are mutated | Yes | Keep and extend |
| Child capability derivation | Yes; public `authority` module | Yes; narrowing succeeds and widening fails closed | Behavior assertions cover both outcomes | Yes; bypassing the escalation check makes the suite red | Yes | Keep |
| Persistent evolution freeze law | Yes; public `authority` module | Yes; candidates are denied and operators can clear it | Behavior assertions cover both actors and resulting state | Yes; bypassing the operator check makes the suite red | Yes | Keep |
| Per-critical-module coverage gate | Yes; called by CI | Yes; locally measured the authority module at 100% and passed remotely | No direct tests of the Python parser/gate itself | No mutation entry for gate internals | Yes | Keep; add self-tests before extending its parser surface |
| Mutation guard and ratchet | Yes; CI blocks on two named invariants | Yes; both local and remote runs killed every mutation | Baseline, apply/restore, and verdict paths are exercised operationally, not unit-tested | It is the guard; its own integrity is not yet independently mutated | Yes | Keep; harden as part of production tooling |
| Documentation gate | Yes; CI checks README and architecture docs | Yes; local and remote runs passed | No direct unit tests | No | Yes | Keep; add claim/link regression fixtures when docs expand |
| GitHub Actions CI | Yes; push and pull-request triggers | Yes; run `32648979874` passed both jobs | Proven by a real hosted run | Enforces the mutation ratchet after deterministic checks | Yes | Keep |
| Master plan | Linked from the README; it is a specification, not executable behavior | Internally coherent, but almost all described product behavior is unimplemented | No executable acceptance mapping before this roadmap | No | Self-documenting | Keep as product source; implementation claims must remain separate |

## Progress since baseline

- The domain vocabulary and lifecycle are executable, versioned, and fail closed.
- The canonical SQLite event ledger and BLAKE3 artifact store persist and verify real on-disk state.
- Strict Genome and World compilers normalize JSON/YAML, resolve ancestry and artifacts, constrain authority, and enforce exact World comparability.
- The runnable daemon and CLI enforce a single writer, owner-only authenticated IPC, persistent operator freeze/kill state, Genome inspection, and verified replay.
- The offline reference runtime exercises isolated worktrees, lifecycle operations, expiring capability tokens, and hard output/authority failure paths. A provider-neutral local process supervisor independently enforces allowlisted environments, stdin prompts, process-group interrupt, and hard wall/output limits. macOS hosted commands are wrapped in a live-tested deny-by-default Seatbelt policy; unsupported hosts fail closed. Hosted provider invocations are built but not executed without a billable permit.
- The Experience Plane records the full observable trace vocabulary as bounded redacted CAS artifacts with safe ledger receipts. `RecordedRuntime` binds real adapter lifecycle, checkpoints, failures, provider-visible events, latency, and known deterministic cost to immutable run/Genome/World provenance, failing closed when required start or resume evidence cannot persist. Structured observations, hypotheses, evidence, and contradictions require verified source events and exact provenance; every lesson remains explicitly unverified and cannot become a Gene here.
- The runtime trust checkpoint requires every supervised external launch to traverse the detected OS isolation policy, records adapter-owned completion reasons and elapsed time, reserves terminal evidence capacity, and retains failed containment and unpersisted observations for retry. The current local gate holds all nineteen critical modules above 95% and kills 106 deliberate mutations.
- The production daemon now composes the registered World/Genome, read-only deterministic runtime, sandbox, evidence wrapper, ledger, and CAS behind `hephaestus run <genome-id>`. A real daemon/CLI test verifies freeze gating, exact provenance/reason/latency/zero cost, authority narrowing, output and trace artifact hashes, sandbox cleanup, terminal projection, and replay after restart.

## Remaining product loop

There is currently no active hosted-provider adapter, TUI, provider-native tool/context/memory event parser, Arena, evaluator isolation, mutation engine, selection engine, lineage, promotion/rollback, validated Gene Bank, drift response, canary deployment, web console, public benchmark, or autonomous evolution loop. The deterministic reference run is the production tracer bullet for L4-L6, not a hosted-model claim.

The repository is therefore a runnable local trust substrate with one real non-billable execution path, not yet the evolutionary product described by the full plan. Later application layers remain unclaimed until their own product-path evidence exists.

## Audit conclusion

Keep every existing implementation feature. The immediate priority is a trustworthy local Arena and evaluator boundary, followed by one complete local evolution loop before population intelligence, distributed execution, or web surfaces.
