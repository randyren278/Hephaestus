use std::{collections::BTreeMap, fs};

use hephaestus_core::authority::CapabilitySet;
use hephaestus_genome::{
    CompileError, CompiledGenome, CompiledWorld, SourceFormat, compile_genome, compile_world,
    ensure_comparable,
};
use hephaestus_ledger::{ArtifactId, ArtifactStore};
use tempfile::tempdir;

fn hash(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

fn compile_test_world(candidate_network: bool, store: &ArtifactStore) -> CompiledWorld {
    let evaluator = store
        .put(b"sealed evaluator")
        .expect("store sealed evaluator")
        .as_str()
        .to_owned();
    let source = format!(
        r#"{{
            "schema_version": 1,
            "name": "code-v1",
            "laws": {{
                "candidate_network": {candidate_network},
                "candidate_evaluator_access": false,
                "maximum_cost_microusd": 5000000
            }},
            "authority_ceiling": {{"workspace_write": true, "network": false}},
            "mutation_scope": ["harness"],
            "promotion": {{"minimum_delta_bps": 300, "maximum_regressions": 0, "confidence_bps": 9500}},
            "objectives": ["correctness", "cost", "latency", "reliability"],
            "evaluator_artifacts": {{"sealed": "{evaluator}"}}
        }}"#
    );
    compile_world(&source, SourceFormat::Json, store).expect("compile test World")
}

#[test]
fn equivalent_json_and_yaml_genomes_have_one_canonical_identity() {
    let directory = tempdir().expect("temporary directory");
    let store = ArtifactStore::open(directory.path()).expect("open artifact store");
    let prompt = store
        .put(b"system prompt")
        .expect("store prompt")
        .as_str()
        .to_owned();
    let world = compile_test_world(false, &store);
    let parents = BTreeMap::new();
    let json = format!(
        r#"{{
            "schema_version": 1,
            "name": "coding-g0",
            "parents": [],
            "model": {{"provider": "openai", "family": "codex"}},
            "authority": {{"workspace_write": true, "network": false}},
            "artifacts": {{"system_prompt": "{prompt}"}}
        }}"#
    );
    let yaml = format!(
        "schema_version: 1\nname: coding-g0\nparents: []\nmodel:\n  provider: openai\n  family: codex\nauthority:\n  workspace_write: true\n  network: false\nartifacts:\n  system_prompt: {prompt}\n"
    );

    let from_json = compile_genome(&json, SourceFormat::Json, &world, &parents, &store)
        .expect("compile JSON Genome");
    let from_yaml = compile_genome(&yaml, SourceFormat::Yaml, &world, &parents, &store)
        .expect("compile YAML Genome");

    assert_eq!(from_json.id(), from_yaml.id());
    assert_eq!(from_json.canonical_json(), from_yaml.canonical_json());
    assert_eq!(from_json.name(), "coding-g0");
}

#[test]
fn genome_compilation_resolves_ancestry_artifacts_and_authority() {
    let directory = tempdir().expect("temporary directory");
    let store = ArtifactStore::open(directory.path()).expect("open artifact store");
    let prompt = store
        .put(b"system prompt")
        .expect("store prompt")
        .as_str()
        .to_owned();
    let world = compile_test_world(false, &store);
    let seed = compile_genome(
        &format!(
            r#"{{"schema_version":1,"name":"g0","parents":[],"model":{{"provider":"openai","family":"codex"}},"authority":{{"workspace_write":true,"network":false}},"artifacts":{{"prompt":"{prompt}"}}}}"#
        ),
        SourceFormat::Json,
        &world,
        &BTreeMap::new(),
        &store,
    )
    .expect("compile seed");
    let parents = BTreeMap::from([(seed.id().to_owned(), seed.clone())]);

    let child_source = format!(
        r#"{{"schema_version":1,"name":"g1","parents":["{}"],"model":{{"provider":"openai","family":"codex"}},"authority":{{"workspace_write":false,"network":false}},"artifacts":{{"prompt":"{prompt}"}}}}"#,
        seed.id()
    );
    let child = compile_genome(&child_source, SourceFormat::Json, &world, &parents, &store)
        .expect("compile child");
    assert_eq!(child.parents(), &[seed.id().to_owned()]);
    assert_eq!(child.authority(), CapabilitySet::new(false, false));

    let missing_parent = child_source.replace(seed.id(), "hephaestus:genome:missing");
    assert!(matches!(
        compile_genome(
            &missing_parent,
            SourceFormat::Json,
            &world,
            &parents,
            &store
        ),
        Err(CompileError::UnresolvedParent(_))
    ));

    let spoofed_id = format!("hephaestus:genome:{}", "0".repeat(64));
    let spoofed_source = child_source.replace(seed.id(), &spoofed_id);
    let spoofed_parents = BTreeMap::from([(spoofed_id.clone(), seed.clone())]);
    assert!(matches!(
        compile_genome(
            &spoofed_source,
            SourceFormat::Json,
            &world,
            &spoofed_parents,
            &store
        ),
        Err(CompileError::ParentIdentityMismatch { .. })
    ));

    let missing_artifact = child_source.replace(&prompt, &hash(b"missing"));
    assert!(matches!(
        compile_genome(
            &missing_artifact,
            SourceFormat::Json,
            &world,
            &parents,
            &store
        ),
        Err(CompileError::UnresolvedArtifact(_))
    ));

    let widened = child_source.replace(
        r#""workspace_write":false,"network":false"#,
        r#""workspace_write":true,"network":true"#,
    );
    assert!(matches!(
        compile_genome(&widened, SourceFormat::Json, &world, &parents, &store),
        Err(CompileError::AuthorityEscalation)
    ));

    let narrow_parent = compile_genome(
        &format!(
            r#"{{"schema_version":1,"name":"narrow","parents":[],"model":{{"provider":"openai","family":"codex"}},"authority":{{"workspace_write":false,"network":false}},"artifacts":{{"prompt":"{prompt}"}}}}"#
        ),
        SourceFormat::Json,
        &world,
        &BTreeMap::new(),
        &store,
    )
    .expect("compile narrow parent");
    let escalating_child = format!(
        r#"{{"schema_version":1,"name":"escalating","parents":["{}"],"model":{{"provider":"openai","family":"codex"}},"authority":{{"workspace_write":true,"network":false}},"artifacts":{{"prompt":"{prompt}"}}}}"#,
        narrow_parent.id()
    );
    let narrow_parents = BTreeMap::from([(narrow_parent.id().to_owned(), narrow_parent.clone())]);
    assert!(matches!(
        compile_genome(
            &escalating_child,
            SourceFormat::Json,
            &world,
            &narrow_parents,
            &store
        ),
        Err(CompileError::AuthorityEscalation)
    ));
}

#[test]
fn world_authority_ceiling_is_independently_enforced() {
    let directory = tempdir().expect("temporary directory");
    let store = ArtifactStore::open(directory.path()).expect("open artifact store");
    let prompt = store
        .put(b"system prompt")
        .expect("store prompt")
        .as_str()
        .to_owned();
    let world = compile_test_world(false, &store);
    let source = format!(
        r#"{{"schema_version":1,"name":"world-escalation","parents":[],"model":{{"provider":"openai","family":"codex"}},"authority":{{"workspace_write":false,"network":true}},"artifacts":{{"prompt":"{prompt}"}}}}"#
    );

    assert!(matches!(
        compile_genome(
            &source,
            SourceFormat::Json,
            &world,
            &BTreeMap::new(),
            &store
        ),
        Err(CompileError::AuthorityEscalation)
    ));
}

#[test]
fn world_compilation_protects_laws_evaluators_and_comparability() {
    let directory = tempdir().expect("temporary directory");
    let store = ArtifactStore::open(directory.path()).expect("open artifact store");
    let evaluator = store
        .put(b"sealed evaluator")
        .expect("store evaluator")
        .as_str()
        .to_owned();
    let baseline = compile_test_world(false, &store);
    let same = compile_test_world(false, &store);
    let changed_law = compile_test_world(true, &store);

    ensure_comparable(&baseline, &same).expect("same World is comparable");
    assert_eq!(baseline.name(), "code-v1");
    assert!(!baseline.canonical_json().is_empty());
    assert_eq!(
        baseline.authority_ceiling(),
        CapabilitySet::new(true, false)
    );
    assert_ne!(baseline.id(), changed_law.id());
    assert!(matches!(
        ensure_comparable(&baseline, &changed_law),
        Err(CompileError::IncompatibleWorlds { .. })
    ));

    let protected_scope = format!(
        r#"{{"schema_version":1,"name":"bad","laws":{{"candidate_network":false,"candidate_evaluator_access":false,"maximum_cost_microusd":1}},"authority_ceiling":{{"workspace_write":false,"network":false}},"mutation_scope":["law"],"promotion":{{"minimum_delta_bps":1,"maximum_regressions":0,"confidence_bps":9500}},"objectives":["correctness"],"evaluator_artifacts":{{"sealed":"{evaluator}"}}}}"#
    );
    assert!(matches!(
        compile_world(&protected_scope, SourceFormat::Json, &store),
        Err(CompileError::ProtectedMutationTarget(_))
    ));

    let evaluator_access = protected_scope
        .replace(
            r#""candidate_evaluator_access":false"#,
            r#""candidate_evaluator_access":true"#,
        )
        .replace(
            r#""mutation_scope":["law"]"#,
            r#""mutation_scope":["harness"]"#,
        );
    assert!(matches!(
        compile_world(&evaluator_access, SourceFormat::Json, &store),
        Err(CompileError::CandidateEvaluatorAccess)
    ));

    let unresolved = protected_scope
        .replace(
            r#""mutation_scope":["law"]"#,
            r#""mutation_scope":["harness"]"#,
        )
        .replace(&evaluator, &hash(b"missing"));
    assert!(matches!(
        compile_world(&unresolved, SourceFormat::Json, &store),
        Err(CompileError::UnresolvedArtifact(_))
    ));
}

#[test]
fn compilers_reject_unknown_versions_fields_and_oversized_input() {
    let directory = tempdir().expect("temporary directory");
    let store = ArtifactStore::open(directory.path()).expect("open artifact store");
    let unknown_version = r#"{"schema_version":2,"name":"bad"}"#;
    assert!(matches!(
        compile_world(unknown_version, SourceFormat::Json, &store),
        Err(CompileError::UnsupportedSchemaVersion(2))
    ));

    let unknown_field = r#"{"schema_version":1,"name":"bad","extra":true}"#;
    assert!(matches!(
        compile_world(unknown_field, SourceFormat::Json, &store),
        Err(CompileError::Parse(_))
    ));

    assert!(matches!(
        compile_world(":", SourceFormat::Yaml, &store),
        Err(CompileError::Parse(_))
    ));

    let oversized = "x".repeat(1_048_577);
    assert!(matches!(
        compile_world(&oversized, SourceFormat::Yaml, &store),
        Err(CompileError::InputTooLarge { .. })
    ));
}

#[test]
fn compilers_reject_invalid_policy_and_corrupt_artifacts() {
    let directory = tempdir().expect("temporary directory");
    let store = ArtifactStore::open(directory.path()).expect("open artifact store");
    let evaluator = store
        .put(b"sealed evaluator")
        .expect("store evaluator")
        .as_str()
        .to_owned();
    let base = format!(
        r#"{{"schema_version":1,"name":"code-v1","laws":{{"candidate_network":false,"candidate_evaluator_access":false,"maximum_cost_microusd":1}},"authority_ceiling":{{"workspace_write":false,"network":false}},"mutation_scope":["harness"],"promotion":{{"minimum_delta_bps":1,"maximum_regressions":0,"confidence_bps":9500}},"objectives":["correctness"],"evaluator_artifacts":{{"sealed":"{evaluator}"}}}}"#
    );

    assert!(matches!(
        compile_world(
            &base.replace(r#""confidence_bps":9500"#, r#""confidence_bps":0"#),
            SourceFormat::Json,
            &store
        ),
        Err(CompileError::InvalidConfidence(0))
    ));
    assert!(matches!(
        compile_world(
            &base.replace(r#""objectives":["correctness"]"#, r#""objectives":[]"#),
            SourceFormat::Json,
            &store
        ),
        Err(CompileError::EmptyObjectives)
    ));
    assert!(matches!(
        compile_world(
            &base.replace(r#""objectives":["correctness"]"#, r#""objectives":[" "]"#),
            SourceFormat::Json,
            &store
        ),
        Err(CompileError::EmptyField("objective"))
    ));
    assert!(matches!(
        compile_world(
            &base.replace(r#""name":"code-v1""#, r#""name":"""#),
            SourceFormat::Json,
            &store
        ),
        Err(CompileError::EmptyField("name"))
    ));

    let world = compile_world(&base, SourceFormat::Json, &store).expect("compile World");
    let invalid_artifact = r#"{"schema_version":1,"name":"g0","parents":[],"model":{"provider":"openai","family":"codex"},"authority":{"workspace_write":false,"network":false},"artifacts":{"prompt":"invalid"}}"#;
    assert!(matches!(
        compile_genome(
            invalid_artifact,
            SourceFormat::Json,
            &world,
            &BTreeMap::new(),
            &store
        ),
        Err(CompileError::InvalidArtifactId(_))
    ));

    let prompt = store
        .put(b"system prompt")
        .expect("store prompt")
        .as_str()
        .to_owned();
    let valid_genome = format!(
        r#"{{"schema_version":1,"name":"g0","parents":[],"model":{{"provider":"openai","family":"codex"}},"authority":{{"workspace_write":false,"network":false}},"artifacts":{{"prompt":"{prompt}"}}}}"#
    );
    let empty_provider = valid_genome.replace(r#""provider":"openai""#, r#""provider":"""#);
    assert!(matches!(
        compile_genome(
            &empty_provider,
            SourceFormat::Json,
            &world,
            &BTreeMap::new(),
            &store
        ),
        Err(CompileError::EmptyField("model.provider"))
    ));

    let prompt_id = ArtifactId::parse(prompt).expect("canonical prompt id");
    fs::write(store.path_for(&prompt_id), b"substituted").expect("corrupt prompt artifact");
    assert!(matches!(
        compile_genome(
            &valid_genome,
            SourceFormat::Json,
            &world,
            &BTreeMap::new(),
            &store
        ),
        Err(CompileError::ArtifactIntegrity(_))
    ));
}

fn _assert_compiled_genome_is_cloneable(value: &CompiledGenome) -> CompiledGenome {
    value.clone()
}
