# HEPHAESTUS
## The Forge for Self-Improving AI Agents

**Document type:** Product + Engineering Master Plan  
**Status:** Foundational specification  
**Product name:** Hephaestus  
**Category:** Evolutionary runtime / control plane for AI agents  
**Primary operator experience:** TUI-first, web control room second  
**Core implementation:** Rust control plane + Python research/evaluation layer  
**Design principle:** **Evidence before autonomy. Improvement must be measurable, replayable, attributable, and reversible.**

---

# 0. Executive Summary

Hephaestus is an **evolutionary runtime for AI agents**.

It observes how an agent performs, diagnoses why it fails, generates improved descendants by modifying the agent's harness, evaluates those descendants against protected tests they cannot inspect or modify, and promotes a new version only when the evidence shows that it is better.

The underlying foundation model does **not** need to change.

Instead, Hephaestus evolves the surrounding intelligence stack:

- system instructions
- context construction
- context eviction and compaction
- memory
- retrieval
- tool definitions
- tool-selection policy
- model routing
- planning strategy
- subagent topology
- verification policy
- retry strategy
- recovery behavior
- budgets
- permissions
- runtime middleware
- reusable skills
- eventually, the improvement mechanism itself

The product promise is:

> **Give Hephaestus an agent. It runs it, finds where it fails, forges better descendants, tests them against evaluations they cannot cheat, and promotes the strongest proven version.**

Hephaestus is not merely an agent orchestrator, a prompt optimizer, or an AI SRE.

It is intended to become:

> **Git + Kubernetes + experimental evolution for agent intelligence.**

Git contributes versions, diffs, ancestry, branches, and rollback.

Kubernetes contributes desired state, reconciliation, health, deployment, canaries, and recovery.

Evolution contributes variation, selection, inheritance, transfer, adaptation, and speciation.

Hephaestus applies those primitives to **agent harnesses themselves**.

---

# 1. Product Thesis

Today, an agent is usually deployed as a loosely coupled collection of:

```text
model
+ prompt
+ tools
+ memory
+ retrieval
+ context policy
+ workflow
+ permissions
```

The agent runs.

If it underperforms, a human changes something.

Hephaestus closes that loop:

```text
               ┌────────────────────┐
               │    AGENT G42       │
               └─────────┬──────────┘
                         │
                        RUNS
                         │
                         ▼
                  EXECUTION TRACES
                         │
                         ▼
                  FAILURE ANALYSIS
                         │
                "why did it lose?"
                         │
       ┌─────────────────┼─────────────────┐
       ▼                 ▼                 ▼
 descendant A      descendant B      descendant C
 memory change      tool change       planner change
       │                 │                 │
       └─────────────────┼─────────────────┘
                         ▼
                       ARENA
                         │
                protected evaluations
                         │
                         ▼
                       winner
                         │
                         ▼
                    CHAMPION G43
                         │
                         └───────────────┐
                                         │
                                         ↺
```

The key question Hephaestus asks is:

> **Can the system discover a measurably better intelligence configuration than the one a human originally designed?**

The more ambitious question is:

> **Can Hephaestus eventually become better at discovering better agents?**

---

# 2. Why This Project Exists

The frontier in agent engineering is moving away from "better prompts" toward:

- harness engineering
- tool and middleware design
- context architecture
- durable memory
- model routing
- runtime adaptation
- source-level self-modification
- protected evaluation
- long-running autonomous execution
- multi-agent specialization
- experience compilation
- recursive improvement

Simply demonstrating that an agent can rewrite its own prompt or source code is no longer enough.

Hephaestus must solve the harder problem:

> **How do you make self-improvement trustworthy?**

The system therefore treats self-modification as an experiment governed by evidence.

A candidate cannot become better merely because it says it is better.

A candidate survives only if it passes a protected arena under immutable rules.

---

# 3. Core Differentiator

Hephaestus combines the following into one coherent system:

1. immutable agent genomes
2. complete ancestry
3. protected evaluators
4. explicit mutation hypotheses
5. trace-based failure attribution
6. evidence-backed promotions
7. population-level experimentation
8. reusable proven genes
9. cross-lineage transfer
10. specialist species
11. environmental drift detection
12. self-healing adaptation branches
13. canary deployment
14. automatic rollback
15. operator-controlled authority
16. recursive improvement with immutable safety laws
17. TUI-first observability
18. reproducible evolutionary history

The governing principle:

> **Self-modification is easy. Trustworthy self-improvement is the product.**

---

# 4. Relationship to Iris and Hera

Hephaestus should not pretend Iris and Hera never existed.

They form two of the strongest conceptual foundations for the new system.

```text
                       HEPHAESTUS
                evolutionary control plane
                         /       \
                        /         \
                       /           \
             authority/runtime     memory/experience
                    /                   \
                   ▼                     ▼
                IRIS                    HERA
```

Hephaestus sits **above** them conceptually.

It generalizes lessons from both projects into a broader agent infrastructure layer.

---

## 4.1 Iris → Hephaestus

Iris contributes the philosophy:

> **The model may decide what it wants to do. Deterministic infrastructure decides whether it is allowed to do it.**

Iris already establishes several important design ideas:

- explicit authority boundaries
- local daemon ownership of consequential action
- approval-gated transitions
- fail-closed behavior
- allowlisted origins
- persistent disarm state
- tool mediation
- no ambient authority
- local-first runtime
- process/session supervision
- runtime health reporting
- deterministic safety checks
- mutation testing around critical control paths

Hephaestus generalizes these into the **Authority Plane**.

### Iris concept → Hephaestus evolution

| Iris | Hephaestus |
|---|---|
| Slack approval before coding session | policy gate before mutation/promotion/deployment |
| allowlisted Slack user | operator identity / authority principal |
| start_coding capability | scoped experiment/runtime capability |
| coding autonomy toggle | World / Genome authority policy |
| local Unix socket control boundary | daemon-owned capability broker |
| persistent disarm marker | global `freeze` / `kill` control |
| launchd supervision | Hephaestus daemon supervision |
| runtime heartbeat | health + liveness ledger |
| fail closed on malformed requests | fail closed on invalid Genome / mutation / promotion |
| tool catalog owned by Iris | mediated MCP/tool gateway owned by Hephaestus |
| session start approval | experiment/canary/promotion approval |
| exact project validation | exact World/Genome/Sandbox validation |

### Reuse principle

Hephaestus should not copy Iris source blindly.

Instead:

1. preserve the **authority philosophy**
2. formalize it as capability-based runtime policy
3. generalize from "can this session start?" to:
   - can this Genome run?
   - can this candidate access this tool?
   - can this mutation touch this layer?
   - can this experiment spend this budget?
   - can this descendant enter hidden evaluation?
   - can this candidate be canaried?
   - can this promotion happen?
4. make all decisions ledgered and replayable

### Hard rule inherited from Iris

> **No agent can grant itself additional authority.**

A child Genome may have equal or narrower permissions than its parent unless an external operator or immutable policy explicitly grants more.

---

## 4.2 Hera → Hephaestus

Hera contributes the philosophy:

> **Experience should become structured, provenance-aware knowledge rather than an ever-growing transcript dump.**

Hera already explores:

- automatic session ingestion
- hybrid retrieval
- BM25 + vector retrieval
- Reciprocal Rank Fusion
- provenance
- atomic knowledge pages
- contradiction detection
- conflict resolution
- citation feedback
- knowledge survival based on usefulness
- archiving rather than destructive deletion
- team knowledge separation
- local-first memory
- knowledge promotion based on actual use

Hephaestus generalizes these into the **Experience Plane** and **Gene Bank**.

### Hera concept → Hephaestus evolution

| Hera | Hephaestus |
|---|---|
| session transcript ingestion | execution-trace ingestion |
| atomic wiki pages | atomic failure/mutation/gene records |
| citations | evidence links |
| contradiction detection | conflicting repair/gene/effect detection |
| page usefulness score | mutation/gene empirical effect |
| prune/archive | retire/archive losing Genomes |
| hybrid retrieval | retrieve relevant prior incidents/mutations/genes |
| team knowledge | cross-lineage Gene Bank |
| public/private knowledge | lineage-local vs transferable knowledge |
| source provenance | run/evaluator/world provenance |
| hot context | task-specific experience injection |
| conflict resolution | reconcile contradictory adaptation evidence |

### Crucial difference

Hephaestus memory cannot simply be:

```text
"things the model remembers"
```

It must distinguish:

```text
OBSERVATION
HYPOTHESIS
MUTATION
EVIDENCE
RESULT
GENE
CONTRADICTION
```

A previous repair is not considered reusable merely because it is semantically similar.

It must have experimental evidence attached.

### Example

```text
Gene:
evidence-before-completion-v3

Origin:
coding lineage G42

Hypothesis:
premature completion occurs because unit-test success is
treated as sufficient evidence.

Observed effect:
coding       +8.1pp
security     +6.7pp
research     +0.4pp

Confidence:
0.94

Contradictions:
R19 found +0.2pp with +23% latency.

Status:
transferable with domain-specific cost warning
```

This is Hera's provenance/citation philosophy evolved into **experimental intelligence memory**.

---

## 4.3 Iris + Hera together

The combination becomes one of Hephaestus's defining architectural ideas:

```text
          HERA                         IRIS
   "what have we learned?"     "what are we allowed to do?"
             \                       /
              \                     /
               \                   /
                ▼                 ▼
                 HEPHAESTUS FORGE
                        │
                        ▼
                 "what should change?"
                        │
                        ▼
                     ARENA
                        │
                        ▼
                 "did it improve?"
```

The system must always answer four different questions separately:

1. **What happened?** — execution ledger
2. **What have we learned?** — Hera-inspired experience/evidence layer
3. **What are we allowed to change?** — Iris-inspired authority layer
4. **Did the change actually improve the agent?** — protected Arena

---

# 5. Branding and Product Language

# HEPHAESTUS

### Primary tagline

> **The forge for self-improving AI agents.**

Alternative technical positioning:

> **An evolutionary control plane for AI agents.**

The mythology maps naturally:

Hephaestus is not the agent.

Hephaestus is **the forge that makes stronger agents**.

---

# 6. Core Product Vocabulary

| Term | Meaning |
|---|---|
| **Agent** | Running instance of a Genome |
| **Genome** | Immutable specification of an agent intelligence configuration |
| **Mutation** | One hypothesized change to a Genome |
| **Descendant** | Genome produced from one or more parents |
| **Generation** | Evolutionary depth |
| **Lineage** | Ancestry DAG of related Genomes |
| **Champion** | Currently promoted Genome for a lineage/world |
| **World** | Versioned evaluation environment |
| **Law** | Non-evolvable rule governing a World |
| **Arena** | Environment in which candidates compete |
| **Gene** | Mutation proven reusable beyond its original candidate |
| **Gene Bank** | Registry of experimentally supported Genes |
| **Drift** | Material environmental or workload distribution change |
| **Forge** | Mutation and descendant-generation subsystem |
| **Promotion** | Evidence-backed Champion replacement |
| **Rollback** | Reversion to a prior Champion |
| **Species** | Specialist lineage maintained for a distinct niche |
| **Extinction** | Retirement/quarantine of a dominated or invalid branch |
| **Evidence Receipt** | Machine-readable proof supporting a claim/promotion |

---

# 7. Fundamental Unit: The Agent Genome

Hephaestus must never treat an agent as an amorphous directory of prompts.

Each agent is represented as a normalized immutable Genome.

Example:

```yaml
genome: hephaestus://coding/g42

model:
  provider: openai
  family: codex
  strategy: adaptive

context:
  discovery_budget: 12000
  compaction: evidence_preserving
  eviction: least_referenced

memory:
  episodic: true
  procedural: true
  retrieval: hybrid_rrf

tools:
  - shell
  - filesystem
  - git
  - ast
  - browser

planning:
  strategy: hypothesis_first
  max_replans: 3

topology:
  type: planner_executor_verifier
  parallelism: 3

verification:
  require:
    - unit_tests
    - integration_tests
    - mutation_guard

recovery:
  tool_failure: retry_then_replan
  context_failure: reconstruct
  agent_crash: resume_checkpoint

authority:
  filesystem: workspace
  network: allowlisted
  secrets: none

budget:
  tokens: 180000
  wall_seconds: 1800
  cost_usd: 8
```

Normalize and hash:

```text
BLAKE3(normalized_genome)

hephaestus:g42:7f28bc91...
```

### Hard requirements

- released Genome content can never change
- mutations create new Genome IDs
- each Genome references its parent(s)
- all referenced artifacts are content-addressed
- a Genome must be reproducible
- incompatible schema versions must fail closed

---

# 8. What May Evolve

The evolvable surface is deliberately broad.

## 8.1 Model genes

- provider
- model family
- stage-specific routing
- fallback routing
- cheap/expensive model escalation
- local vs hosted choice

## 8.2 Context genes

- discovery strategy
- context budget
- evidence retention
- compaction algorithm
- eviction policy
- codebase map format
- transcript retention
- tool-result compression

## 8.3 Memory genes

- episodic memory
- procedural memory
- semantic retrieval
- failure memory
- provenance requirements
- contradiction handling
- retention policies

## 8.4 Tool genes

- tool availability
- tool descriptions
- tool schemas
- tool wrappers
- AST utilities
- repo search
- browser strategy
- testing tools
- debugger tools

## 8.5 Planning genes

- direct execution
- plan/execute
- hypothesis-first
- planner/executor/verifier
- parallel candidate reasoning
- replan thresholds

## 8.6 Topology genes

- single agent
- supervisor/worker
- planner/executor
- planner/executor/verifier
- adversarial reviewer
- dynamically created specialist subagents

## 8.7 Verification genes

- unit test requirements
- integration requirements
- mutation tests
- visual regression
- static analysis
- hidden evaluator strategy
- completion criteria

## 8.8 Recovery genes

- retry thresholds
- alternative-tool fallback
- alternate-model fallback
- replan strategy
- checkpoint restore
- branch creation on repeated failure

## 8.9 Authority genes

Within policy ceilings:

- filesystem scope
- network scope
- tool access
- deployment authority
- secret scope
- human-approval thresholds

## 8.10 Improvement genes

Eventually:

- failure clustering
- mutation prioritization
- experiment allocation
- candidate count
- crossover policy
- gene-transfer policy
- exploration/exploitation balance

---

# 9. The Laws

Hephaestus can only be trusted if some parts are outside evolution.

Two universes exist.

## Evolvable World

Candidates may alter:

```text
models
routing
prompts
tools
memory
context
planning
topology
recovery
skills
middleware
runtime implementation
improvement strategy
```

## The Laws

Candidates may **not** alter:

```text
sealed holdouts
evaluator implementation
security boundary
capability ceiling
ledger integrity
experiment budget enforcement
promotion rules
rollback mechanism
artifact hashes
World definitions
operator kill controls
```

Architecture:

```text
┌───────────────────────────────────────┐
│            EVOLVABLE WORLD            │
│                                       │
│ agents / tools / memory / topology    │
│ context / routing / recovery          │
│ improvement strategy                  │
└──────────────────┬────────────────────┘
                   │
                   │ cannot modify
                   ▼
┌───────────────────────────────────────┐
│               THE LAWS                │
│                                       │
│ sealed evals                          │
│ permissions                           │
│ cost enforcement                      │
│ lineage ledger                        │
│ evaluator isolation                   │
│ promotion policy                      │
│ rollback                              │
│ operator sovereignty                  │
└───────────────────────────────────────┘
```

A candidate cannot improve its score by weakening the physics of the world.

---

# 10. Worlds

A **World** establishes the evaluation environment and its laws.

Example:

```yaml
world: hephaestus-world-code-v1

tasks:
  training: 80
  validation: 40
  hidden_holdout: 50

laws:
  candidate_network: restricted
  evaluator_access: none
  maximum_cost_usd: 500

mutation_scope:
  evaluator: forbidden
  laws: forbidden

promotion:
  minimum_delta: 0.03
  maximum_regressions: 0
  confidence: 0.95

objectives:
  - correctness
  - cost
  - latency
  - reliability
```

If evaluator semantics materially change:

```text
WORLD 1 ends
WORLD 2 begins
```

Results from incompatible Worlds must never be displayed as directly comparable.

---

# 11. Hephaestus as a Ledger System

Hephaestus is fundamentally an **evidence system**.

The database is not incidental storage.

The product revolves around explicit ledgers.

---

# 12. Ledger 01 — Execution Ledger

Records everything observable during an agent run.

Examples:

```text
run_started
model_invoked
context_assembled
memory_retrieved
tool_requested
tool_result
file_read
file_changed
test_executed
subagent_spawned
context_compacted
checkpoint_created
agent_completed
run_failed
```

Event fields:

```text
event_id
run_id
genome_id
world_id
timestamp
actor
event_type
input_hash
output_hash
cost
duration
parent_event
metadata
```

### Exit requirement

A complete run can be reconstructed without relying on unstructured terminal logs.

### CLI

```bash
hephaestus runs show <id>
hephaestus runs replay <id>
```

---

# 13. Ledger 02 — Genome Ledger

Stores every Genome ever created.

Fields:

```text
genome_id
parent_ids
generation
lineage_id
mutation_ids
artifact_hashes
created_at
created_by
creation_reason
status
```

Statuses:

```text
experimental
validated
champion
retired
quarantined
```

### Hard rule

No Genome mutates in place.

### CLI

```bash
hephaestus genome show G42
hephaestus genome diff G41 G42
hephaestus genome ancestry G42
```

---

# 14. Ledger 03 — Mutation Ledger

Every proposed change must include a hypothesis.

Example:

```text
Mutation M283

Observation:
18/50 failures stopped after unit tests.

Hypothesis:
completion policy overweights unit-test success.

Target:
verification.completion_policy

Change:
require integration evidence when integration suite exists.

Expected:
reduce premature completion failures.

Parent:
G41

Candidate:
G42
```

### Requirement

Every mutation must state:

1. observed failure
2. suspected cause
3. target layer
4. proposed change
5. expected measurable effect

This enables causal analysis later.

---

# 15. Ledger 04 — Evidence Ledger

Every claim must point to evidence.

Example:

```text
CLAIM:
G42 is better than G41.

EVIDENCE:

paired tasks        50
G41 successes       34
G42 successes       39

delta              +10.0pp
cost delta          -4.7%
latency delta       +1.2%

bootstrap CI        [+3.1,+16.4]
regression tests     PASS
hidden holdout       PASS
```

This is where Hera's provenance philosophy becomes central.

A claim without evidence cannot support a promotion.

---

# 16. Ledger 05 — Experiment Ledger

Tracks complete experiments.

```text
experiment E0182

parent         G41
children       G42 G43 G44 G45

training set   CODE-V1:A
validation     CODE-V1:B
holdout        SEALED

budget         $32
spent          $27.16

status         complete
winner         G42
```

Required for reproducibility.

---

# 17. Ledger 06 — Evaluation Ledger

Records evaluator execution separately from general evidence.

Fields:

```text
evaluator_id
evaluator_version
world_id
task_id
seed
environment_image
candidate_genome
score
stderr_hash
artifact_hashes
duration
cost
```

### Isolation requirements

Candidate agents must never receive direct access to:

- hidden test source
- expected output
- scoring implementation
- other candidates' hidden scores
- promotion thresholds when hiding them is part of the World

---

# 18. Ledger 07 — Promotion Ledger

Promotion must be explicit.

```text
PROMOTION P029

G41
 ↓
G42

because:

+10.0pp correctness
-4.7% cost
0 invariant regressions
hidden holdout positive
reliability gate passed
```

Rollback:

```text
ROLLBACK P030

G42
 ↓
G41

reason:
live tool-error rate +19%
```

Models may recommend promotion.

Models may **not execute promotion authority themselves**.

---

# 19. Ledger 08 — Gene Ledger

A successful mutation may become a reusable Gene.

Example:

```yaml
gene: evidence-before-completion-v3

class:
  verification

requires:
  - test_runner
  - filesystem

origin:
  lineage: coding
  genome: G42
  mutation: M283

observed_effects:
  coding:
    correctness: +0.10

  security:
    correctness: +0.07

  research:
    correctness: +0.01

confidence: 0.94
```

This is the **Gene Bank**.

It is not generic RAG.

It stores reusable harness adaptations with measured effects.

---

# 20. Ledger 09 — Transfer Ledger

Records movement of Genes between lineages.

Example success:

```text
coding:G42
     │
     │ gene M283
     ▼
security:S17

result:
+7.1pp
```

Example failure:

```text
coding:M283
    ↓
research:R21

result:
-3.4pp

TRANSFER REJECTED
```

Negative transfer is as important as positive transfer.

---

# 21. Ledger 10 — Drift Ledger

Distinguishes:

```text
agent is bad
```

from:

```text
environment changed
```

Track:

- provider API changes
- tool schema changes
- model version changes
- repository distribution shift
- cost changes
- latency changes
- new failure clusters
- runtime dependency changes
- benchmark distribution shift

Example:

```text
DRIFT D018

github.create_pull_request

tool_schema_error
baseline: 0.3%
current: 19.7%

detected:
11:04:12

affected lineages:
coding
security

adaptation branch:
G87.*
```

---

# 22. Ledger 11 — Recovery Ledger

Self-healing becomes a consequence of evolution.

```text
drift detected
 ↓
diagnosis
 ↓
recovery branch
 ↓
candidate variants
 ↓
shadow evaluation
 ↓
canary
 ↓
promotion
```

Hephaestus retains the last known-good Champion during adaptation.

---

# 23. Ledger 12 — Authority Ledger

This is the strongest direct Iris tie-in.

Every worker receives explicit capabilities.

Example:

```text
G42 experiment worker

read source       ✓
write worktree    ✓
run tests         ✓
network           restricted
view hidden eval  ✕
modify Laws       ✕
modify ledger     ✕
promote self      ✕
```

Every denied operation is ledgered.

Capabilities expire with the run/experiment.

No child inherits ambient parent credentials.

---

# 24. Ledger 13 — Cost Ledger

Every unit of spend must be attributable.

Example:

```text
experiment E182

Mutation generation       $2.81
Candidate G42             $6.12
Candidate G43             $5.94
Candidate G44             $5.81
Evaluator                 $0.63

TOTAL                     $21.31

performance gained        +5.8pp

$/percentage-point        $3.67
```

Future objectives:

```text
performance
performance / $
performance / token
performance / second
```

---

# 25. Ledger 14 — Artifact Ledger

Everything significant becomes content-addressed.

Examples:

- patches
- workspace snapshots
- traces
- model responses exposed to runtime
- context snapshots
- evaluator outputs
- binaries
- container images
- prompts
- tool schemas
- generated skills

Address:

```text
blake3:e612...
```

This prevents a Genome from silently changing underneath its ID.

---

# 26. Ledger 15 — World Ledger

Records the root evaluation context.

```text
world W1 created
laws v1
benchmarks v1
budget model v1

world W2 created
laws v2
```

The World Ledger is part of the root of trust.

---

# 27. Additional Ledger — Knowledge / Experience Ledger

This is the explicit Hera-derived layer.

Store structured lessons:

```text
experience_id
source_run_ids
failure_cluster
hypothesis
applicable_domains
confidence
contradictions
supersedes
gene_links
evidence_links
retrieval_tags
```

Use cases:

- retrieve prior failure classes
- retrieve prior successful mutations
- identify contradictory fixes
- warn when a Gene historically failed in a domain
- seed Forge mutation generation
- reduce repeated exploration

### Rule

Raw experience is never promoted into a reusable Gene merely because an LLM says it is useful.

The Arena must validate it.

---

# 28. High-Level Architecture

V1 should be local-first.

Do **not** start with Kubernetes or Temporal.

The novel part is experimentally defensible evolution.

```text
                         HEPHAESTUS
                evolutionary control plane
                          │
          ┌───────────────┼────────────────┐
          │               │                │
          ▼               ▼                ▼
      OPERATOR          ARENA            FORGE
       PLANE            PLANE            PLANE
          │               │                │
        TUI          evaluators        mutation
        CLI          holdouts          selection
        API          experiments       transfer
          │               │                │
          └───────────────┼────────────────┘
                          │
                          ▼
                  EVENT + LEDGER SPINE
                          │
             ┌────────────┴────────────┐
             ▼                         ▼
           SQLite                   Artifact CAS
                                      BLAKE3
             │
             ▼
                         RUNTIME
                            │
         ┌──────────────────┼──────────────────┐
         ▼                  ▼                  ▼
       Codex              Claude             Other
      adapter             adapter           adapters
         │                  │                  │
         └──────────────────┼──────────────────┘
                            ▼
                         SANDBOXES
                            │
                    git worktrees
                    containers
                    isolated env
```

---

# 29. Major Planes

## 29.1 Operator Plane

Owns:

- TUI
- CLI
- API
- freeze/kill
- policy management
- approvals
- live inspection
- lineage navigation

## 29.2 Forge Plane

Owns:

- failure clustering
- mutation proposals
- candidate generation
- crossover
- Gene retrieval
- transfer experiments
- descendant construction

## 29.3 Arena Plane

Owns:

- visible evaluation
- hidden evaluation
- paired trials
- statistical comparison
- regression checks
- evidence receipt generation

## 29.4 Runtime Plane

Owns:

- model adapters
- subprocess control
- agent lifecycle
- sandboxes
- checkpoints
- resource limits
- tool mediation

## 29.5 Authority Plane

Iris-inspired.

Owns:

- capabilities
- ceilings
- scopes
- approval policy
- deny logging
- operator emergency controls

## 29.6 Experience Plane

Hera-inspired.

Owns:

- trace distillation
- structured experience
- contradiction detection
- retrieval
- Gene Bank
- negative transfer memory
- provenance

## 29.7 Deployment Plane

Later.

Owns:

- shadow
- canary
- promotion
- rollback
- fleet deployment

---

# 30. Technology Choices

## Core control plane: Rust

Use Rust for:

- daemon
- scheduler
- event ledger
- state machines
- process supervision
- capability enforcement
- CLI
- TUI
- streaming API
- artifact store
- runtime adapters

Suggested libraries:

```text
tokio
axum
ratatui
crossterm
serde
sqlx
blake3
tracing
opentelemetry
clap
ulid
```

### Why Rust

- one distributable binary
- strong typed state machines
- reliable concurrency
- excellent process supervision
- strong TUI ecosystem
- predictable daemon behavior
- systems-level portfolio value
- appropriate for control-plane code

---

# 31. Python Research Layer

Python remains useful for:

```text
hephaestus-lab/
  statistics
  clustering
  benchmark adapters
  experiment analysis
  embedding analysis
  notebooks
  research prototypes
```

The Rust daemon remains canonical.

Python never receives direct authority to mutate canonical state outside defined protocols.

---

# 32. Storage

V1:

### SQLite + WAL

Canonical structured state.

### Content-addressed filesystem

```text
~/.hephaestus/blobs/
  e6/
    e612912...
```

Later:

```text
SQLite → Postgres
local CAS → S3/R2/object storage
```

The domain model should not depend on storage backend.

---

# 33. Runtime Adapter Interface

Create provider neutrality early.

Conceptual Rust interface:

```rust
trait AgentRuntime {
    async fn start(&self, run: RunSpec) -> Result<RunHandle>;
    async fn resume(&self, checkpoint: Checkpoint) -> Result<RunHandle>;
    async fn interrupt(&self, run_id: RunId) -> Result<()>;
    async fn snapshot(&self, run_id: RunId) -> Result<Checkpoint>;
    async fn capabilities(&self) -> RuntimeCapabilities;
}
```

Initial adapters:

1. Codex
2. Claude

Later:

- OpenAI Agents SDK
- Gemini CLI
- OpenCode
- local models
- remote workers

Do not build abstractions for five providers before two really work.

---

# 34. MCP and A2A

## MCP

Use for tool mediation.

Candidate agents should consume tools through a Hephaestus-owned gateway.

Benefits:

- unified schemas
- auditable calls
- capability enforcement
- provider neutrality
- tool telemetry
- dynamic tool sets

## A2A

Later.

Use when lineage members become independently addressable services/agents.

Do not add A2A in V0.1 purely for buzzword coverage.

---

# 35. Sandbox Model

Every candidate must receive:

```text
new worktree
new execution directory
new environment
new capability token
bounded network scope
bounded budget
```

Candidate siblings cannot inspect each other.

Candidate processes cannot mutate canonical Hephaestus data.

Future backends:

```text
local process
Docker
Podman
Firecracker
remote sandbox
Kubernetes Job
```

Abstract behind:

```rust
trait SandboxBackend {}
```

---

# 36. Trace Model

Capture operational cognition without requiring hidden chain-of-thought.

Record:

- task
- available tools
- selected tools
- tool inputs
- tool outputs
- context composition metadata
- context sizes
- memory retrieval IDs
- subagent graph
- errors
- retries
- model responses available to the runtime
- files accessed
- files changed
- tests
- cost
- latency
- completion reason
- checkpoints
- capability denials

Do **not** depend on hidden private reasoning.

Hephaestus must work from observable execution evidence.

---

# 37. TUI-First Product Strategy

The TUI is a flagship feature, not a debug shell.

Architecture:

```text
hephaestusd
 │
 ├── hephaestus CLI
 ├── hephaestus TUI
 ├── web console
 └── API clients
```

The daemon owns truth.

Every UI is a projection.

---

# 38. TUI — Home

Launch:

```bash
hephaestus
```

Example:

```text
╭─ HEPHAESTUS ────────────────────────────────────────────────────────╮
│ WORLD code-v1                     STATUS ● EVOLVING                 │
│ CHAMPION G42                      GENERATION 42                     │
│ POPULATION 18                     BUDGET $73.18 / $100              │
╰─────────────────────────────────────────────────────────────────────╯

 LINEAGES                      LIVE

 coding      G42 ★ 78.6%       E-184  ● evaluating G47
 security    S18 ★ 84.1%       E-185  ● generating variants
 research    R27 ★ 91.3%       D-019  ▲ drift detected

──────────────────────────────────────────────────────────────────────

 RECENT EVOLUTION

 G41 ──────► G42 ★

 + evidence-preserving compaction
 + integration-test completion gate
 - raw test log retention

 correctness     +7.2pp
 cost            -4.7%
 latency         +1.2%

──────────────────────────────────────────────────────────────────────

 [L] Lineages  [A] Arena  [R] Runs  [G] Genes  [D] Drift  [/] Search
```

---

# 39. TUI — Lineage View

```text
                    G14
                  ╱     ╲
               G17       G18 ✕
                │
               G23
             ╱     ╲
          G27       G28
           │          ✕
          G31
           │
          G42 ★

Selected: G42

Generation       42
Parent           G31
Fitness          78.6
Cost/task        $1.17
Tokens/task      73,412
Reliability      97.1%

Mutation:
M-192 evidence-first completion

[e] evidence
[d] diff
[r] replay
[a] ancestry
```

This should become the README hero screenshot.

---

# 40. TUI — Arena

```text
EVOLUTION E184

                       G42
                        │
         ┌──────────────┼──────────────┐
         │              │              │
        G46            G47            G48
      74.1%          81.3%          79.2%
        ✕              ●              ✓

TARGETED
████████████████████ 100%

REGRESSION
█████████████████░░░  84%

SEALED
██████████░░░░░░░░░░  48%

G47 current leader
```

---

# 41. TUI — Genome Diff

```text
G42 → G47

context
- max_raw_tool_tokens: 16000
+ max_raw_tool_tokens: 8000

context.compaction
- summary
+ evidence_preserving

verification
+ failed_test_retention: true

planning
- replan_after_tool_failure: 3
+ replan_after_tool_failure: 2
```

This makes improvement legible.

---

# 42. TUI — Evidence View

```text
CLAIM

G47 improves coding reliability.

PUBLIC EVAL
  41 / 50 → 44 / 50

HIDDEN EVAL
  36 / 50 → 41 / 50

DELTA
  +10.0pp

95% CI
  +3.2pp ───────────────── +16.1pp

REGRESSIONS
  0

COST
  -7.4%

VERDICT

██████████████  PROMOTE
```

No vibes.

Receipts.

---

# 43. TUI — Gene Bank

```text
GENE BANK

NAME                           ORIGIN       TRANSFERS     EFFECT
──────────────────────────────────────────────────────────────
evidence-before-complete       coding       3/3           +8.1
compact-test-output-v2         coding       2/3           +4.7
tool-schema-repair             ops          4/4          +13.4
parallel-hypothesis            research     2/5           +1.9
mutation-guard                 security     4/4           +7.6
```

---

# 44. TUI — Drift / Self-Healing

```text
⚠ DRIFT D019

github.create_pull_request

Schema failure

baseline           0.4%
current           18.7%

started            22:41:17
affected runs      27
affected lineages   3

ADAPTATION

D019
 │
 ├── G53  failed
 ├── G54  evaluating
 └── G55  shadowing ★

Current champion remains G42.
No promotion yet.
```

This is the Hephaestus self-healing story made visible.

---

# 45. Operator Controls

Essential commands:

```bash
hephaestus status
hephaestus freeze
hephaestus unfreeze
hephaestus kill --all
hephaestus run <genome>
hephaestus evolve <lineage>
hephaestus arena <experiment>
hephaestus genome show <id>
hephaestus genome diff <a> <b>
hephaestus lineage <id>
hephaestus genes
hephaestus drift
hephaestus rollback <promotion>
```

### Operator sovereignty

`freeze` stops evolution.

`kill --all` stops active execution.

Evolution may never re-enable itself.

This is directly inherited from Iris's persistent disarm philosophy.

---

# 46. Web Console — Later

Do not build the web UI before the TUI proves which visualizations matter.

Eventually:

```text
/overview
/lineages
/lineages/:id
/genomes/:id
/experiments
/arena
/runs
/genes
/drift
/evidence
/cost
/worlds
/settings
```

The browser talks only to `hephaestusd`.

It never talks directly to agents.

---

# 47. Web Lineage Visualization

The signature visualization should feel like:

```text
Git history
×
family tree
×
evolutionary tree
×
agent architecture inspector
```

Features:

- pan/zoom ancestry
- branch by specialization
- node size by survival / evaluation count
- badge for Champion
- color by species/domain
- inspect mutation edge
- scrub through generation timeline
- overlay fitness/cost/latency
- identify Gene transfers
- show extinction/revival

---

# 48. Master Build Ledger

| Ledger | Build | Exit condition |
|---|---|---|
| **L0** | Product Constitution | scope, Laws, threat model, terminology frozen |
| **L1** | Repository + CI | green lint/test/coverage/mutation smoke |
| **L2** | Event Spine | append-only event ledger + deterministic replay |
| **L3** | Genome Model | immutable content-addressed Genomes |
| **L4** | Runtime Adapter | Codex/Claude run through strict interface |
| **L5** | Sandbox | isolated worktrees + bounded execution |
| **L6** | Tracing | complete operational trace/cost/context capture |
| **L7** | Worlds | versioned Laws and evaluator definitions |
| **L8** | Arena | reproducible parent-vs-candidate evaluation |
| **L9** | Sealed Evaluation | candidate cannot inspect holdout/evaluator |
| **L10** | Mutation Engine | trace → failure → hypothesis → mutation |
| **L11** | Selection Engine | statistical candidate comparison |
| **L12** | Lineage Engine | ancestry, branching, retirement, rollback |
| **L13** | TUI v1 | runs + lineage + genome diff + controls |
| **L14** | Evidence System | every promotion has reproducible receipts |
| **L15** | Autonomous Evolution | unattended multi-generation improvement |
| **L16** | Gene Bank | extract reusable successful mutations |
| **L17** | Transfer Engine | test Genes across lineages |
| **L18** | Speciation | maintain specialist populations |
| **L19** | Drift Engine | detect environment/workload changes |
| **L20** | Self-Healing | automatically spawn adaptation branches |
| **L21** | Canary Engine | shadow → canary → promote → rollback |
| **L22** | Recursive Evolution | Evolver becomes evolvable |
| **L23** | Web Console | lineage/evidence/experiment visualization |
| **L24** | Public Gauntlet | reproducible evolution benchmark |
| **L25** | Production Hardening | security, chaos, crash recovery, release |
| **L26** | Hephaestus Evolves Hephaestus | dogfood own engineering lineage |

---

# 49. L0 — Product Constitution

Create before runtime code:

```text
docs/CONSTITUTION.md
docs/THREAT_MODEL.md
docs/TERMINOLOGY.md
docs/EVALUATION_PHILOSOPHY.md
docs/IRIS_INHERITANCE.md
docs/HERA_INHERITANCE.md
```

Freeze definitions for:

- Agent
- Genome
- Mutation
- Descendant
- Lineage
- Gene
- World
- Law
- Champion
- Arena
- Evolution
- Drift

### Exit condition

No major subsystem uses conflicting vocabulary.

---

# 50. L1 — CI Before Intelligence

Required from the beginning:

```text
cargo fmt
cargo clippy
cargo test
cargo llvm-cov
Python tests
integration tests
DB migration tests
event replay tests
property tests
```

Mutation checks must protect:

- evaluator isolation
- promotion boundary
- World integrity
- capability enforcement
- ledger append-only behavior
- rollback
- operator freeze/kill

A deliberate mutation that weakens one of these must make CI red.

---

# 51. L2 — Event Spine

Build before agents.

Conceptual event:

```rust
struct Event {
    id: Ulid,
    aggregate_id: String,
    sequence: u64,
    event_type: EventType,
    actor: Actor,
    timestamp: DateTime<Utc>,
    payload: Value,
    previous_hash: Hash,
    hash: Hash,
}
```

Benefits:

- crash recovery
- audit
- replay
- TUI streaming
- web streaming
- debugging
- complete history

### Exit condition

Delete all derived projections, replay the event stream, reproduce identical state.

---

# 52. L3 — Genome Compiler

Pipeline:

```text
genome.yaml
       ↓
validate
       ↓
normalize
       ↓
resolve references
       ↓
capability check
       ↓
hash
       ↓
immutable Genome
```

Invalid Genome:

```text
REJECT
```

Never "best effort."

---

# 53. L4 — First Runtime

Support one provider first.

Recommended starting runtime: Codex.

Command:

```bash
hephaestus run G0 tasks/hello.yaml
```

Must produce:

```text
run ID
trace
cost
workspace diff
artifacts
result
checkpoint
```

Then add Claude.

---

# 54. L5 — Isolation

Every candidate:

```text
new worktree
new process environment
new capability token
new execution directory
bounded resources
```

### Acceptance tests

- sibling cannot inspect sibling workspace
- candidate cannot modify canonical database
- candidate cannot inspect hidden evaluator files
- candidate cannot exceed filesystem scope
- candidate cannot reuse expired capability
- process kill leaves canonical state consistent

---

# 55. L6 — Trace Capture

Capture enough information to attribute failures.

Acceptance:

- every tool call visible
- every context assembly visible as metadata
- every memory retrieval ID visible
- subagent graph reconstructable
- costs attributable
- completion reason explicit
- no dependence on hidden chain-of-thought

---

# 56. L7–L9 — Scientific Arena

Build trustworthy measurement before automatic mutation.

Initial World:

```text
20 training tasks
20 visible validation tasks
20 sealed tasks
```

Later integrations may include:

- SWE-bench
- Terminal-Bench
- custom coding harness tasks
- security tasks
- research tasks
- operations tasks

### Rule

The optimizer must exist **after** the objective is trustworthy.

---

# 57. L10 — Mutation Engine V1

Version one should be constrained.

Input:

```text
failed runs
trace clusters
relevant experience
```

Output:

```json
{
  "failure_class": "premature_completion",
  "evidence": ["run:..."],
  "suspected_layer": "verification",
  "confidence": 0.83,
  "proposed_change": "..."
}
```

Then generate:

```text
ONE minimal candidate mutation
```

Minimal changes make causal attribution easier.

---

# 58. L11 — Selection Engine

Never say:

```text
78 > 76 therefore promote
```

Agent evaluation is stochastic.

Use paired trials.

Same:

- task
- environment
- budget
- provider version
- seed where applicable

Calculate:

- paired delta
- confidence interval
- severity-weighted failure difference
- cost delta
- latency delta
- reliability delta

Maintain Pareto frontiers where appropriate.

---

# 59. L12 — Lineage Engine

Initial:

```text
G0
 ↓
G1
 ↓
G2
 ↓
G3
```

Then:

```text
      G0
    /    \
  G1      G2
   │       ✕
  G3
```

Losers are archived rather than deleted.

Old branches may later become useful under changed environments.

---

# 60. L13 — TUI v1

Ship TUI before autonomous evolution.

Requirements:

- live experiments
- current Champion
- Genome viewer
- Genome diff
- lineage DAG
- run trace
- cost view
- pause
- freeze
- terminate
- rollback
- authority-denial inspection

---

# 61. L14 — Evidence Receipts

Every promotion emits machine-readable proof.

Example:

```json
{
  "promotion": "P29",
  "parent": "G41",
  "candidate": "G42",
  "world": "W1",
  "evidence_hash": "blake3:...",
  "fitness_delta": 0.072,
  "regressions": 0,
  "decision": "promote"
}
```

README/project claims should eventually point at receipts.

---

# 62. L15 — Autonomous Evolution

Defining V0.1 demonstration:

```bash
hephaestus evolve coding --budget 100
```

No manual harness edits during the run.

Example result:

```text
Started        G0       51.2%
Generation               14
Candidates tested       127
Current Champion        G37
Current score           68.4%
Improvement            +17.2pp
Spend                  $93.41
```

### Exit condition

At least 3 unattended generations and statistically supported improvement on unseen tasks.

---

# 63. L16 — Gene Bank

Extract reusable mutations.

A Gene cannot be created merely from one positive trial.

Require:

- origin evidence
- minimum evaluation count
- confidence threshold
- applicability metadata
- contradiction tracking

Hera's contradiction philosophy should be reused directly here.

---

# 64. L17 — Transfer Engine

For each Gene:

```text
source lineage
 ↓
target candidate
 ↓
paired arena
 ↓
effect measurement
```

Track negative transfer.

A Gene may be:

```text
general
domain-specific
harmful-in-domain
obsolete
superseded
```

---

# 65. L18 — Speciation

Do not implement species as decorative labels.

Create a species when empirical performance shows specialization.

Example:

```text
                      CODING ANCESTOR
                            │
            ┌───────────────┼────────────────┐
            │               │                │
            ▼               ▼                ▼
        FRONTEND          BACKEND         SECURITY
        lineage           lineage          lineage
```

Criteria could include:

- statistically significant domain advantage
- persistent cross-domain regression
- stable distinct Genome features
- enough task volume to justify niche maintenance

---

# 66. L19 — Drift Engine

Monitor live task distributions and runtime environment.

Signals:

- success-rate changes
- tool error changes
- provider response changes
- cost shift
- latency shift
- task embedding distribution shift
- failure-cluster emergence

Drift creates:

```text
adaptation branch
```

not immediate Champion replacement.

---

# 67. L20 — Self-Healing

When drift is detected:

```text
Champion G42 remains active
       │
       ├── adaptation G53
       ├── adaptation G54
       └── adaptation G55
```

Candidates enter Arena.

Best candidate enters shadow.

Only then can canary start.

This is the evolution of the earlier Hephaestus "self-healing software" concept into a more general self-healing **agent lineage**.

---

# 68. L21 — Canary Engine

Promotion path:

```text
offline candidate
 ↓
shadow
 ↓
5%
 ↓
25%
 ↓
50%
 ↓
100%
```

Any meaningful regression triggers:

```text
rollback
```

The previous Champion remains reconstructable.

---

# 69. L22 — Recursive Evolution

Only after the base loop is proven.

Represent the Evolver as its own Genome.

The Evolver may include:

```text
failure mining
mutation prioritization
experiment allocation
candidate count
Gene selection
exploration strategy
```

Evaluate Evolvers in a meta-World.

Question:

> Can Evolver E8 reach equal or better Champions with less compute than E7?

The Evolver can evolve.

The Laws cannot.

---

# 70. L23 — Web Console

Build after TUI usage reveals what matters.

Required:

- lineage graph
- Genome diff
- experiment timeline
- Arena status
- evidence receipts
- Gene Bank
- drift timeline
- cost analysis
- World history
- promotion history
- authority audit

---

# 71. L24 — Hephaestus Gauntlet

Bundle a public benchmark suite specifically designed to force harness adaptation.

Examples:

| Failure | Expected adaptation |
|---|---|
| Context overflow | improved compaction |
| Repeated file reads | caching/context memory |
| Premature completion | verification gate |
| Tool schema drift | adapter repair |
| Bad model selection | routing |
| Expensive simple tasks | cheaper routing |
| Subagent duplication | topology repair |
| Lost earlier evidence | evidence memory |
| Over-retry | failure classifier |
| Under-exploration | repo discovery |
| Hallucinated test status | deterministic verifier |
| Poisoned memory | provenance policy |

Public output must be reproducible.

---

# 72. L25 — Production Hardening

Required:

- daemon crash recovery
- DB corruption tests
- event hash validation
- sandbox escape tests
- capability mutation tests
- hidden-evaluator leakage tests
- budget-enforcement tests
- provider outage tests
- model timeout tests
- kill/restart during promotion
- duplicate-event idempotency
- rollback under partial failure
- chaos suite
- supply-chain checks
- signed release artifacts

---

# 73. L26 — Hephaestus Evolves Hephaestus

The ultimate dogfood milestone.

Create a Hephaestus engineering lineage.

Give it a sandboxed copy of the Hephaestus repository.

Allow it to propose improvements to:

- Forge heuristics
- TUI performance
- runtime adapters
- trace processing
- mutation classification
- test coverage
- operational reliability

But still require:

- protected CI
- mutation guards
- sealed evaluation
- human-controlled merge/promotion boundary initially

The project becomes its own managed subject.

---

# 74. Hard Requirements

## R1 — Immutable ancestry

Released Genomes never change.

## R2 — Protected evaluators

Candidate processes cannot inspect hidden evaluator content.

## R3 — Replayability

Every promotion has sufficient evidence to reproduce it.

## R4 — Crash recovery

Kill Hephaestus mid-experiment.

Restart.

No lost canonical state.

No duplicate promotion.

## R5 — No ambient authority

Workers receive explicit capabilities.

## R6 — Hard budgets

Token/time/cost limits are runtime-enforced.

## R7 — Deterministic promotion

Models may recommend.

Models cannot self-promote.

## R8 — Automatic rollback

Promotion retains a recoverable ancestor.

## R9 — World versioning

Changing Laws creates a new World.

## R10 — Honest comparisons

Incompatible Worlds cannot be presented as direct progress.

## R11 — Provider neutrality

Genome semantics cannot fundamentally depend on one vendor.

## R12 — Operator sovereignty

```bash
hephaestus freeze
hephaestus kill --all
```

cannot be overridden by evolution.

## R13 — Provenance

Every reusable lesson links to originating evidence.

## R14 — Contradictions are first-class

Conflicting evidence is surfaced rather than silently overwritten.

## R15 — Fail closed

Malformed Genome, capability, evaluator, promotion, or mutation requests are rejected.

---

# 75. Explicit Non-Goals for V0.1

Do not start with:

- Kubernetes
- billing
- multi-tenancy
- SaaS accounts
- mobile app
- marketplace
- hundreds of MCP integrations
- agent social network
- fine-tuning
- RL
- decentralized infrastructure
- enterprise SSO
- giant A2A swarm
- self-owning company narrative

The first question is:

> **Can Hephaestus produce an objectively better agent than the one we gave it, and prove why that descendant deserves to survive?**

Everything serves that thesis.

---

# 76. Suggested Repository Structure

```text
hephaestus/
│
├── crates/
│   ├── hephaestus-core/
│   ├── hephaestus-ledger/
│   ├── hephaestus-genome/
│   ├── hephaestus-runtime/
│   ├── hephaestus-sandbox/
│   ├── hephaestus-arena/
│   ├── hephaestus-policy/
│   ├── hephaestus-experience/
│   ├── hephaestus-evolution/
│   ├── hephaestus-api/
│   ├── hephaestus-cli/
│   └── hephaestus-tui/
│
├── python/
│   └── hephaestus_lab/
│       ├── statistics/
│       ├── clustering/
│       ├── benchmarks/
│       └── analysis/
│
├── genomes/
│   └── seed/
│
├── worlds/
│   ├── code-v1/
│   └── harness-v1/
│
├── gauntlet/
│
├── web/
│
├── tests/
│   ├── integration/
│   ├── mutation/
│   ├── chaos/
│   └── acceptance/
│
├── docs/
│   ├── CONSTITUTION.md
│   ├── ARCHITECTURE.md
│   ├── THREAT_MODEL.md
│   ├── EVALUATION.md
│   ├── GENOME.md
│   ├── LEDGERS.md
│   ├── WORLDS.md
│   ├── IRIS_INHERITANCE.md
│   └── HERA_INHERITANCE.md
│
└── README.md
```

---

# 77. V0.1 Definition

Do not call the project operational until all exist:

```text
✓ immutable Genomes
✓ two runtime adapters
✓ execution ledger
✓ artifact store
✓ isolated candidates
✓ one World
✓ visible evaluator
✓ sealed evaluator
✓ parent-vs-child Arena
✓ mutation generation
✓ statistical selection
✓ promotion
✓ rollback
✓ ancestry DAG
✓ TUI
✓ replay
✓ hard budgets
✓ authority boundary
✓ experience/provenance layer
✓ 3+ unattended generations
✓ measurable unseen-task improvement
```

---

# 78. V0.2 Definition

```text
✓ population
✓ Gene Bank
✓ cross-lineage transfer
✓ multiple task domains
✓ Pareto fitness
✓ drift detection
✓ shadow deployment
✓ automatic recovery branch
✓ contradiction-aware experience retrieval
```

---

# 79. V0.3 Definition

```text
✓ speciation
✓ evolvable mutation controller
✓ meta-evaluation
✓ web console
✓ remote workers
✓ MCP gateway
✓ distributed artifact backend
```

---

# 80. V1.0 Definition

V1.0 should meet an intentionally difficult standard:

> **Hephaestus has operated continuously for 30 days, produced multiple independently verified lineage improvements, recovered from deliberate environmental drift without a human editing the affected harness, survived process crashes without corrupting evolutionary history, and can reproduce every Champion from its Genome and Evidence Ledger.**

---

# 81. Headline Demonstration 1 — Evolution

Start with deliberately weak G0.

```text
G0     52%
```

Run Hephaestus unattended.

End with:

```text
G0 ── G4 ── G12 ── G19 ── G42 ★

52%                     74%
```

Click every edge.

Show:

- mutation
- hypothesis
- run traces
- evaluation
- cost
- confidence
- receipt

This proves measurable evolution.

---

# 82. Headline Demonstration 2 — Extinction Event

While a Champion is active:

change a tool schema.

Population failures spike.

TUI:

```text
DRIFT DETECTED
```

Hephaestus:

1. identifies affected lineages
2. retrieves relevant prior experiences/Genes
3. spawns adaptation branches
4. evaluates descendants
5. shadows the best
6. canaries it
7. promotes it
8. retains rollback

No human edits the harness.

This proves self-healing adaptation.

---

# 83. Headline Demonstration 3 — Evolution Evolves

Early Evolver:

```text
E1

$81
43 experiments
+10pp
```

Later Evolver:

```text
E12

$29
17 experiments
+10pp
```

Headline:

> **Hephaestus got better at getting better.**

This is the recursive-improvement demonstration.

---

# 84. Optional Demonstration 4 — Gene Transfer

A security lineage discovers a strong verification Gene.

```text
SECURITY
   │
   │ evidence-first verification
   ▼
GENE BANK
   │
   ├── BACKEND +6.2pp
   ├── OPS     +8.4pp
   └── RESEARCH -1.1pp
```

Hephaestus learns:

- where the Gene transfers
- where it does not
- what the tradeoffs are

This demonstrates population-level learning.

---

# 85. How Hephaestus Connects to Existing Projects

A compelling long-term demonstration is to manage a real portfolio as the first fleet.

```text
                         HEPHAESTUS
                      evolutionary forge
                             │
        ┌────────────────────┼────────────────────┐
        │                    │                    │
        ▼                    ▼                    ▼
      IRIS                  HERA                STRAITS
 authority-heavy       memory-heavy         operations-heavy
 agent lineage         knowledge lineage     systems lineage
```

## Iris as a managed lineage

Potential Hephaestus targets:

- tool-routing reliability
- approval latency
- context quality
- coding-session success
- control-plane reliability
- fail-closed behavior

Immutable Iris safety invariants remain protected.

Hephaestus may optimize the agent harness around them.

## Hera as a managed lineage

Potential targets:

- retrieval precision
- retrieval cost
- contradiction surfacing
- citation usefulness
- memory injection quality
- pruning policy
- session distillation

Hephaestus can experimentally evolve retrieval/memory strategies while Hera-inspired provenance remains mandatory.

## Straits as an operations subject

Potential targets:

- anomaly pipeline resilience
- operational diagnostics
- incident handling
- deployment strategies
- self-healing workflows

This gives Hephaestus real systems workloads beyond synthetic benchmarks.

---

# 86. Iris Integration Strategy

Three possible levels.

## Level 1 — conceptual reuse

Hephaestus independently implements:

- authority policy
- local daemon
- kill/freeze
- fail-closed behavior

No runtime dependency.

Recommended for V0.1.

## Level 2 — shared authority library

Extract reusable capability primitives from Iris into a separate package.

Possible later.

## Level 3 — Iris as operator channel

Hephaestus could expose selected control actions through Iris:

```text
"show me Hephaestus drift"
"freeze Hephaestus"
"what changed from G41 to G42?"
"approve canary for G47"
```

Important:

Iris remains an interface/operator assistant.

Hephaestus remains the canonical control plane.

---

# 87. Hera Integration Strategy

## Level 1 — conceptual reuse

Implement Hephaestus experience storage independently but carry over:

- provenance
- contradiction handling
- hybrid retrieval ideas
- explicit evidence linking

Recommended for V0.1.

## Level 2 — Hera-backed operator research

Allow Hephaestus incident/evolution records to be exported to Hera as human-readable durable knowledge.

## Level 3 — shared knowledge substrate

Potentially reuse Hera's retrieval/indexing engine for local experience retrieval.

Only if benchmarks show it is advantageous.

### Important boundary

Hera memory and Hephaestus canonical evolutionary evidence are not the same thing.

Canonical promotion evidence must live inside Hephaestus.

---

# 88. Security Model

Threats:

- candidate attempts to inspect hidden evaluator
- prompt injection through repository/tool output
- candidate attempts to modify Laws
- candidate attempts to modify ledger
- candidate attempts to grant itself authority
- candidate attempts to leak secrets
- evaluator poisoning
- budget bypass
- sibling candidate interference
- artifact substitution
- stale capability reuse
- self-promotion
- malicious Gene transfer
- poisoned memory/experience

Required defenses:

- sandbox isolation
- capability tokens
- immutable hashes
- evaluator separation
- separate evaluator process/user where possible
- deny-by-default network
- no ambient credentials
- append-only signed/hash-linked event stream
- content-addressed artifacts
- hard runtime quotas
- operator freeze
- mutation guards

---

# 89. Failure Philosophy

Hephaestus should classify failures.

Example taxonomy:

```text
MODEL_FAILURE
TOOL_FAILURE
TOOL_SCHEMA_DRIFT
CONTEXT_OVERFLOW
CONTEXT_LOSS
MEMORY_RETRIEVAL_FAILURE
PLANNING_FAILURE
VERIFICATION_FAILURE
PREMATURE_COMPLETION
AUTHORITY_DENIAL
BUDGET_EXHAUSTION
SANDBOX_FAILURE
PROVIDER_OUTAGE
EVALUATOR_FAILURE
HEPHAESTUS_CONTROL_FAILURE
```

Recovery depends on class.

Do not blindly retry everything.

---

# 90. Statistical Philosophy

The system must treat evaluation as noisy.

Required approaches where applicable:

- paired comparisons
- bootstrap confidence intervals
- multiple seeds
- repeated trials
- severity-weighted failures
- cost-adjusted objectives
- Pareto frontier
- minimum effect size
- hidden holdout
- regression floor

A candidate may be "better but too expensive."

That should remain a tradeoff rather than being collapsed into one magical score too early.

---

# 91. Fitness Model

Initial dimensions:

```text
correctness
reliability
cost
latency
tool errors
human intervention
recovery rate
regressions
```

Avoid single-number fitness initially.

Represent candidate comparisons as Pareto dominance where possible.

Later Worlds can define explicit utility functions.

---

# 92. Gene Bank Requirements

A Gene record includes:

```text
gene_id
name
category
origin_lineage
origin_genome
origin_mutation
required_capabilities
applicable_domains
known_effects
confidence
cost_effect
latency_effect
contradictions
supersedes
retired_reason
```

Genes may be:

```text
experimental
validated
transferable
domain_specific
superseded
retired
harmful
```

---

# 93. Contradiction Handling

Directly inspired by Hera.

Example:

```text
Gene A claim:
"retrying tool X twice improves reliability"

Gene B claim:
"retrying tool X increases duplicate writes"
```

Do not silently choose whichever embedding ranks highest.

Create a contradiction record.

Resolve by:

- domain
- tool version
- World version
- authority mode
- idempotency guarantees
- new experiment

---

# 94. Self-Healing Hierarchy

Hephaestus should eventually heal at four levels.

## Level 1 — Process healing

```text
worker crashes → resume
model timeout → retry/failover
sandbox dies → restore
```

## Level 2 — Workflow healing

```text
trajectory stalls
 ↓
failure classifier
 ↓
replan / alternate tool / alternate model
```

## Level 3 — Agent healing

```text
persistent failures
 ↓
Forge adaptation branch
 ↓
Arena
 ↓
new Champion
```

## Level 4 — Hephaestus healing

```text
control-plane degradation
 ↓
independent watchdog
 ↓
state reconstruction
 ↓
verification
```

The fourth level must be extremely constrained.

---

# 95. Future Distributed Architecture

Only after local V1 proves the thesis.

Potential architecture:

```text
Hephaestus Control Plane
         │
         ├── experiment scheduler
         ├── policy
         ├── ledger
         └── artifact index
         │
   ┌─────┼─────┐
   ▼     ▼     ▼
worker worker worker
```

Possible later technologies:

- Postgres
- object storage
- NATS / durable queue
- Kubernetes
- Temporal
- remote sandboxes
- A2A

Do not introduce these prematurely.

---

# 96. GitHub Integration — Later but Important

Hephaestus should eventually support:

- repo registration
- branch/worktree creation
- PR creation
- check-run ingestion
- CI evidence ingestion
- release monitoring

But GitHub is an external execution surface.

Iris-style authority rules apply.

A candidate should not automatically merge its own PR simply because it created it.

---

# 97. Evaluation Classes

## Deterministic evaluators

- unit tests
- static analysis
- compilation
- schema validation
- contract tests

## Stochastic evaluators

- agent task success
- rubric scoring
- LLM judge
- human preference

## Adversarial evaluators

- mutation testing
- fault injection
- prompt injection
- corrupted context
- tool drift

## Live evaluators

- canary error rate
- latency
- user task completion
- intervention rate

Promotion should combine evidence classes.

---

# 98. Public Benchmark Philosophy

The Gauntlet should be:

- reproducible
- versioned as Worlds
- adversarial
- transparent where possible
- sealed where necessary
- cost-accounted
- provider-neutral
- able to distinguish one-shot agent quality from improvement ability

Core benchmark question:

> **How much better can a system make its starting agent under a fixed budget?**

That is more interesting than simply ranking static agents.

---

# 99. Suggested Initial Milestone Timeline

This is sequencing, not a promise of calendar duration.

## Phase A — Trust substrate

- L0 Constitution
- L1 CI
- L2 Event Spine
- L3 Genome
- L7 Worlds
- L9 evaluator isolation
- Iris-inspired Authority Plane

## Phase B — Execution substrate

- L4 runtime adapter
- L5 sandbox
- L6 traces
- Hera-inspired Experience Plane
- initial CLI

## Phase C — Measurement substrate

- L8 Arena
- L11 selection
- L14 evidence receipts

## Phase D — First evolution

- G0 baseline
- L10 mutation engine
- G1 descendants
- L12 lineage
- L13 TUI
- L15 autonomous generations

## Phase E — Population intelligence

- L16 Gene Bank
- L17 transfer
- L18 speciation

## Phase F — Living system

- L19 drift
- L20 adaptation
- L21 canary

## Phase G — Recursive system

- L22 Evolver evolution
- L23 web
- L24 public benchmark
- L25 hardening
- L26 dogfooding

---

# 100. Definition of Success

Hephaestus is successful if it can demonstrate all of the following:

1. start from a deliberately imperfect agent
2. run it against a World
3. identify recurrent failure modes from observable traces
4. produce explicit mutation hypotheses
5. generate immutable descendants
6. isolate descendants safely
7. evaluate them against protected tests
8. statistically distinguish better candidates
9. promote a better candidate deterministically
10. preserve complete ancestry
11. replay the promotion evidence
12. transfer a useful Gene to another lineage
13. reject a harmful transfer
14. detect environmental drift
15. adapt through a branch without immediately replacing the Champion
16. canary a new descendant
17. rollback on regression
18. survive process failure
19. respect hard budgets
20. never let an agent grant itself authority
21. never let a candidate modify The Laws
22. show all of this clearly in the TUI
23. eventually improve the improvement process itself

---

# 101. The Product Story

The simplest explanation should remain:

> **Your agents shouldn't be the same tomorrow.**

Then:

> Deploy an agent. Hephaestus watches how it performs, generates better descendants, tests them against evaluations they cannot cheat, and replaces the parent when a descendant proves it is better.

Then show:

```text
G0 ── G4 ── G12 ── G19 ── G42 ★

51%                     79%
```

Click the lineage.

Every improvement has receipts.

---

# 102. Resume / Portfolio-Level Description

> **Built Hephaestus, an evolutionary control plane for AI agents that transforms execution traces into experimentally validated harness improvements. The system versions agents as immutable Genomes, generates and evaluates descendants against protected Worlds, tracks ancestry and transferable Genes, detects drift, performs canary promotion and rollback, and exposes the entire evolutionary process through a Rust TUI and evidence ledger.**

Expanded technical version:

> **Designed a Rust-based local-first agent runtime combining content-addressed Genomes, append-only event sourcing, capability-scoped execution, sandboxed Codex/Claude adapters, hidden evaluation, statistical selection, provenance-aware experience memory, lineage tracking, cross-lineage Gene transfer, drift adaptation, and recursive harness optimization.**

---

# 103. Final Architecture Principle

Every major design decision should be tested against one question:

> **Does this make Hephaestus better at proving that an agent became better?**

If the answer is no, it is probably not V0.1 work.

The order matters:

```text
CONSTITUTION
    ↓
IDENTITY
    ↓
AUTHORITY
    ↓
EVIDENCE
    ↓
EVALUATION
    ↓
MUTATION
    ↓
SELECTION
    ↓
EVOLUTION
    ↓
SELF-HEALING
    ↓
RECURSIVE IMPROVEMENT
```

Not the other way around.

---

# 104. Final Relationship Map

```text
                              HEPHAESTUS
                       self-improving agent forge
                                  │
        ┌─────────────────────────┼─────────────────────────┐
        │                         │                         │
        ▼                         ▼                         ▼
  AUTHORITY PLANE          EXPERIENCE PLANE            ARENA
  inspired by IRIS         inspired by HERA       protected evidence
        │                         │                         │
        │                         │                         │
        └──────────────┐          │          ┌──────────────┘
                       ▼          ▼          ▼
                           THE FORGE
                               │
                               ▼
                         DESCENDANTS
                               │
                               ▼
                           LINEAGES
                               │
               ┌───────────────┼───────────────┐
               ▼               ▼               ▼
             CODING         SECURITY           OPS
                                                │
                                                ▼
                                      self-healing behavior
```

Iris answers:

> **What may the agent do?**

Hera answers:

> **What did we learn, and why do we believe it?**

Hephaestus answers:

> **Given what we learned and what we are allowed to change, can we forge a better agent and prove it?**

That is the project.

---

# Appendix A — Initial Research Themes to Track

The project should continuously monitor the following research categories:

- Agentic Harness Engineering
- self-harness optimization
- source-level self-modifying agents
- persistent agent adaptation
- online harness learning
- recursive improvement
- tool creation
- context engineering
- memory architecture
- hidden evaluation
- agent benchmark optimization
- distributed agent adaptation
- MCP
- A2A
- durable execution
- agent observability
- capability security
- sandboxed execution

Representative topics/papers discussed during planning include:

- Agentic Harness Engineering
- Meta-Harness
- Self-Harness
- HarnessFix
- MOSS
- HyperAgents
- Evo-Harness
- EvoHarness-RL
- HarnessOpt-Bench
- PAST-Bench
- Evo-Bench
- Darwin Gödel Machine-style archive/evolution ideas

These should be treated as adjacent research, not requirements to clone.

---

# Appendix B — First README Hero

```text
┌─────────────────────────────────────────────────────────┐
│                     HEPHAESTUS                          │
│          The forge for self-improving AI agents.       │
└─────────────────────────────────────────────────────────┘

Give Hephaestus an agent.

It runs it.
It finds where it fails.
It forges descendants.
It tests them against evaluations they cannot cheat.
It promotes the strongest proven version.

G0 ───── G7 ───── G19 ───── G42 ★
51%                            79%

Every mutation has evidence.
Every Champion has ancestry.
Every promotion can roll back.
```

---

# Appendix C — First Demo Acceptance Checklist

- [ ] G0 represented as immutable Genome
- [ ] World W1 exists
- [ ] hidden evaluator is isolated
- [ ] execution events are complete
- [ ] candidate sandbox is isolated
- [ ] authority capability is explicit
- [ ] failure cluster is generated
- [ ] mutation hypothesis is recorded
- [ ] G1 descendant is generated
- [ ] parent and child run paired evaluation
- [ ] evidence receipt is generated
- [ ] promotion is deterministic
- [ ] ancestry graph updates
- [ ] TUI shows Genome diff
- [ ] TUI shows evidence
- [ ] rollback works
- [ ] daemon survives kill/restart
- [ ] 3 unattended generations complete
- [ ] unseen-task performance improves
- [ ] no candidate modifies Laws
- [ ] no candidate self-promotes

---

# Appendix D — North Star

The north-star demonstration is not:

> "An LLM rewrote its own prompt."

It is:

> **Hephaestus started with an imperfect agent, observed real failures, forged multiple descendants, proved one was better under protected evaluation, promoted it, transferred one successful adaptation to another lineage, detected a later environmental change, adapted again without human harness edits, and preserved enough evidence to reproduce every decision.**

That is the standard.
