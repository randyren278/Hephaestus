# Runtimes and Sandboxes

`crates/hephaestus-runtime/` keeps provider mechanics outside Genome semantics. Every `RunSpec` binds a Genome, World, prompt, source repository, exact resolved Git commit, explicit capabilities, and non-zero wall/output/cost budget. The revision is resolved once when the specification is created, so later branch movement cannot change the evaluated source. The sandbox manager creates a private detached worktree at that commit, a separate execution directory, and a non-cloneable 256-bit capability token bound to the run and an expiry.

```mermaid
flowchart LR
    Spec[Provider-neutral RunSpec] --> Token[Expiring capability token]
    Spec --> Worktree[Detached private worktree]
    Token --> Adapter{Runtime adapter}
    Worktree --> Adapter
    Adapter --> Reference[Deterministic offline]
    Adapter --> Codex[Codex invocation]
    Adapter --> Claude[Claude invocation]
    Reference --> Snapshot[Bounded observable snapshot]
```

## Lifecycle contract

Adapters report enforceable capabilities and implement start, resume, interrupt, and snapshot. Snapshots expose status, an exact supervisor-owned completion reason, measured elapsed time, exit code, bounded stdout/stderr artifact paths, and assigned authority—never hidden chain-of-thought. Adapters expose typed provider-visible observations separately from lifecycle state. The deterministic adapter inventories tracked regular files by path, size, and BLAKE3 hash inside the worktree and emits context, file-read, and response metadata. Resume is accepted only for an existing non-running run and replaces prior state only after capability validation succeeds. The adapter provides a non-billable reference path for CI and fails on network widening, expired or cross-run tokens, and output-budget overflow.

## Hosted drivers

Codex and Claude Code invocation builders are pinned to the locally inspected non-interactive CLI contracts. Prompts travel over stdin rather than the process list. Codex uses ephemeral mode, a workspace-derived sandbox mode, and explicit network policy. Claude uses safe mode, no session persistence, explicit tools, `dontAsk`, streaming JSON, and a six-decimal hard cost ceiling.

The builders are deliberately inert in this checkpoint. Live hosted execution may consume paid quota or credentials and therefore requires a separate explicit execution permit. On macOS, provider commands run through a deny-by-default Seatbelt profile: the active run root is the only writable subtree, sibling worktrees and canonical protected paths are unreadable, and network is denied unless the compiled capability allows it. Hosts without a verified backend fail closed.

The non-billable process supervisor proves the shared lifecycle mechanics with real local helpers. Its constructor requires an `IsolationPolicy`, and the production launch path cannot construct a child command without that policy. Every adapter rejects a specification whose repository or pinned commit differs from the materialized sandbox. It starts each child in its own process group and allowlisted environment, requires complete prompt delivery through stdin, drains stdout and stderr concurrently into one combined byte ceiling, enforces the wall deadline independently of callers, kills descendants on interrupt, and exposes bounded artifact paths plus exact terminal state. A partial or failed stdin write is an I/O failure even when the child exits zero. Credential mediation, verified provider-specific resume, and a hard Codex cost limiter remain prerequisites before hosted builders can become active adapters.
