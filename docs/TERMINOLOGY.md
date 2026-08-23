# Terminology

| Term | Frozen meaning |
|---|---|
| Agent | A running instance of a Genome |
| Genome | An immutable specification of an agent intelligence configuration |
| Mutation | One hypothesized change to a Genome |
| Descendant | A Genome produced from one or more parents |
| Generation | Evolutionary depth in an ancestry graph |
| Lineage | The ancestry graph of related Genomes |
| Champion | The currently promoted Genome for one lineage and World |
| World | A versioned evaluation environment and its Laws |
| Law | A non-evolvable rule governing a World |
| Arena | The protected environment where candidates are evaluated |
| Gene | A mutation with evidence supporting reuse beyond its origin |
| Gene Bank | The registry of experimentally supported Genes |
| Drift | A material environment or workload distribution change |
| Forge | Failure analysis, mutation, and descendant generation |
| Promotion | Deterministic Champion replacement supported by evidence |
| Rollback | Reversion to a reconstructable prior Champion |
| Species | A specialist lineage justified by measured niche performance |
| Evidence receipt | Machine-readable proof for a comparison or decision |

The canonical Rust vocabulary is `EntityKind` in `crates/hephaestus-core/src/domain.rs`. New subsystems must reuse these meanings instead of inventing local synonyms.
