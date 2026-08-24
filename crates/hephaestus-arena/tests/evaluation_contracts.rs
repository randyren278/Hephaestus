use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::PermissionsExt as _,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

use hephaestus_arena::{
    ArenaError, EvaluationBinding, EvaluationInputs, EvaluationStores, IsolatedEvaluator,
    OperatorEvaluation, ReceiptContext, TrialPlan, TrustedManifest, TrustedTask, Visibility,
    evaluate_and_record,
};
use hephaestus_core::authority::CapabilitySet;
use hephaestus_experience::{
    RunBudgetReceipt, RunCompletionReason, RunResultReceipt, RunResultSigner,
};
use hephaestus_genome::{CompiledWorld, SourceFormat, compile_genome, compile_world};
use hephaestus_ledger::ArtifactId;
use hephaestus_runtime::{Budget, ExperimentContext, IsolationPolicy, RunSpec, WorkerLimits};
use tempfile::TempDir;

const VISIBLE_SECRET: &str = "visible-expected-never-in-candidate-input";
const SEALED_SECRET: &str = "sealed-expected-9f67c2";
const TASKS: [&str; 4] = [
    "task-sealed-a",
    "task-sealed-b",
    "task-visible-a",
    "task-visible-b",
];

struct Fixture {
    stores: EvaluationStores,
    world: CompiledWorld,
    binding: EvaluationBinding,
    visible: TrustedManifest,
    sealed: TrustedManifest,
    parent_genome_id: String,
    candidate_genome_id: String,
    parent: TrialPlan,
    candidate: TrialPlan,
    evaluator: IsolatedEvaluator,
    signer: RunResultSigner,
    repository: PathBuf,
    revision: String,
    alternate_revision: String,
}

fn manifests() -> (TrustedManifest, TrustedManifest) {
    (
        TrustedManifest::new(
            "visible-suite-v1",
            Visibility::Visible,
            vec![
                TrustedTask::new("task-visible-b", "input visible b", "B").unwrap(),
                TrustedTask::new("task-visible-a", "input visible a", VISIBLE_SECRET).unwrap(),
            ],
        )
        .unwrap(),
        TrustedManifest::new(
            "sealed-suite-v1",
            Visibility::Sealed,
            vec![
                TrustedTask::new("task-sealed-b", "sealed prompt 517", "Z").unwrap(),
                TrustedTask::new("task-sealed-a", "sealed prompt 204", SEALED_SECRET).unwrap(),
            ],
        )
        .unwrap(),
    )
}

fn context() -> ReceiptContext {
    ReceiptContext {
        event_id: "arena:evaluation:evaluation-001:recorded".to_owned(),
        evaluation_id: "evaluation-001".to_owned(),
        caller_id: "arena-test".to_owned(),
        timestamp_millis: 1_788_000_123_456,
    }
}

#[allow(clippy::too_many_arguments)]
fn append_run(
    stores: &mut EvaluationStores,
    signer: &RunResultSigner,
    repository: &Path,
    run_id: &str,
    genome_id: &str,
    world_id: &str,
    revision: &str,
    task_id: &str,
    input: &str,
    reason: RunCompletionReason,
    output: &[u8],
) -> String {
    append_run_with_context(
        stores,
        signer,
        repository,
        run_id,
        genome_id,
        world_id,
        revision,
        task_id,
        input,
        42,
        "environment-v1",
        Budget::new(Duration::from_secs(10), 1_048_576, 0).unwrap(),
        reason,
        output,
    )
}

#[allow(clippy::too_many_arguments)]
fn append_run_with_context(
    stores: &mut EvaluationStores,
    signer: &RunResultSigner,
    repository: &Path,
    run_id: &str,
    genome_id: &str,
    world_id: &str,
    revision: &str,
    task_id: &str,
    input: &str,
    seed: u64,
    environment_id: &str,
    budget: Budget,
    reason: RunCompletionReason,
    output: &[u8],
) -> String {
    let stdout = stores.artifacts.put(output).unwrap();
    let stderr = stores.artifacts.put(b"").unwrap();
    let trace = stores
        .artifacts
        .put(format!("trace:{run_id}").as_bytes())
        .unwrap();
    let experiment = ExperimentContext::new(task_id, input, seed, environment_id).unwrap();
    let spec = RunSpec::new_for_experiment_at_revision(
        run_id,
        genome_id,
        world_id,
        repository,
        revision,
        input,
        CapabilitySet::new(false, false),
        budget,
        experiment,
    )
    .unwrap();
    let receipt = RunResultReceipt::from_run_spec(
        &spec,
        reason,
        1,
        0,
        stdout.as_str(),
        stderr.as_str(),
        vec![trace.as_str().to_owned()],
    )
    .unwrap();
    let event_id = receipt.event_id();
    stores
        .events
        .append(signer.issue(receipt, 1_788_000_000_000).unwrap())
        .unwrap();
    event_id
}

fn plan(role: &str, replacement: Option<(&str, &str)>) -> TrialPlan {
    TrialPlan::new(TASKS.map(|task| {
        let event_id = replacement
            .as_ref()
            .filter(|(replaced, _)| *replaced == task)
            .map_or_else(
                || format!("result:{role}-{task}"),
                |(_, id)| (*id).to_owned(),
            );
        (task.to_owned(), event_id)
    }))
    .unwrap()
}

fn make_fixture(directory: &TempDir) -> Fixture {
    let evaluator_path = directory.path().join("hephaestus-evaluator");
    fs::copy(env!("CARGO_BIN_EXE_hephaestus-evaluator"), &evaluator_path).unwrap();
    fs::set_permissions(&evaluator_path, fs::Permissions::from_mode(0o700)).unwrap();
    make_fixture_with_evaluator(directory, evaluator_path)
}

#[allow(clippy::too_many_lines)]
fn make_fixture_with_evaluator(directory: &TempDir, evaluator_path: PathBuf) -> Fixture {
    let mut stores = EvaluationStores::open(
        directory.path().join("events.sqlite3"),
        directory.path().join("blobs"),
    )
    .unwrap();
    let (visible, sealed) = manifests();
    let visible_id = stores
        .artifacts
        .put(&serde_json::to_vec(&visible).unwrap())
        .unwrap();
    let sealed_id = stores
        .artifacts
        .put(&serde_json::to_vec(&sealed).unwrap())
        .unwrap();
    let evaluator_id = stores
        .artifacts
        .put(&fs::read(&evaluator_path).unwrap())
        .unwrap();
    let evaluator = IsolatedEvaluator::open_with_policy(
        directory.path().join("evaluator-runs"),
        evaluator_path,
        evaluator_id.as_str(),
        IsolationPolicy::unconfined_for_testing(),
        WorkerLimits::new(Duration::from_secs(5), 16 * 1024 * 1024, 64 * 1024).unwrap(),
    )
    .unwrap();
    let signer = RunResultSigner::from_seed([7; 32]);
    let verifier_id = stores
        .artifacts
        .put(&signer.verifier().public_key_bytes())
        .unwrap();
    let source = format!(
        r#"{{"schema_version":1,"name":"arena-world","laws":{{"candidate_network":false,"candidate_evaluator_access":false,"maximum_cost_microusd":0}},"authority_ceiling":{{"workspace_write":false,"network":false}},"mutation_scope":["harness"],"promotion":{{"minimum_delta_bps":1,"maximum_regressions":0,"confidence_bps":9500}},"objectives":["correctness"],"evaluator_artifacts":{{"arena.visible_manifest":"{}","arena.sealed_manifest":"{}","arena.evaluator":"{}","arena.runtime_verifier":"{}"}}}}"#,
        visible_id.as_str(),
        sealed_id.as_str(),
        evaluator_id.as_str(),
        verifier_id.as_str()
    );
    let world = compile_world(&source, SourceFormat::Json, &stores.artifacts).unwrap();
    let parent_genome = compile_genome(
        r#"{"schema_version":1,"name":"parent","parents":[],"model":{"provider":"deterministic","family":"v1"},"authority":{"workspace_write":false,"network":false},"artifacts":{}}"#,
        SourceFormat::Json,
        &world,
        &BTreeMap::new(),
        &stores.artifacts,
    )
    .unwrap();
    let parents = BTreeMap::from([(parent_genome.id().to_owned(), parent_genome.clone())]);
    let candidate_source = format!(
        r#"{{"schema_version":1,"name":"candidate","parents":["{}"],"model":{{"provider":"deterministic","family":"v2"}},"authority":{{"workspace_write":false,"network":false}},"artifacts":{{}}}}"#,
        parent_genome.id()
    );
    let candidate_genome = compile_genome(
        &candidate_source,
        SourceFormat::Json,
        &world,
        &parents,
        &stores.artifacts,
    )
    .unwrap();
    let repository = directory.path().join("source");
    let (revision, alternate_revision) = repository_fixture(&repository);
    let outputs = [
        ("task-visible-a", VISIBLE_SECRET, VISIBLE_SECRET),
        ("task-visible-b", "wrong", "B"),
        ("task-sealed-a", SEALED_SECRET, "wrong"),
        ("task-sealed-b", "Z", "Z"),
    ];
    for (task, parent_output, candidate_output) in outputs {
        append_run(
            &mut stores,
            &signer,
            &repository,
            &format!("parent-{task}"),
            parent_genome.id(),
            world.id(),
            &revision,
            task,
            task_input(task),
            RunCompletionReason::Success,
            parent_output.as_bytes(),
        );
        append_run(
            &mut stores,
            &signer,
            &repository,
            &format!("candidate-{task}"),
            candidate_genome.id(),
            world.id(),
            &revision,
            task,
            task_input(task),
            RunCompletionReason::Success,
            candidate_output.as_bytes(),
        );
    }
    let binding = EvaluationBinding::new(
        world.id(),
        42,
        "environment-v1",
        evaluator_id.as_str(),
        budget_receipt(),
    )
    .unwrap();
    Fixture {
        stores,
        world,
        binding,
        visible,
        sealed,
        parent_genome_id: parent_genome.id().to_owned(),
        candidate_genome_id: candidate_genome.id().to_owned(),
        parent: plan("parent", None),
        candidate: plan("candidate", None),
        evaluator,
        signer,
        repository,
        revision,
        alternate_revision,
    }
}

fn budget_receipt() -> RunBudgetReceipt {
    RunBudgetReceipt {
        wall_millis: 10_000,
        maximum_output_bytes: 1_048_576,
        maximum_cost_microusd: 0,
    }
}

fn task_input(task_id: &str) -> &'static str {
    match task_id {
        "task-visible-a" => "input visible a",
        "task-visible-b" => "input visible b",
        "task-sealed-a" => "sealed prompt 204",
        "task-sealed-b" => "sealed prompt 517",
        _ => panic!("unknown task fixture: {task_id}"),
    }
}

fn repository_fixture(path: &Path) -> (String, String) {
    fs::create_dir(path).unwrap();
    run_git(path, &["init", "-q"]);
    fs::write(path.join("fixture"), b"first\n").unwrap();
    run_git(path, &["add", "fixture"]);
    commit(path, "first");
    let first = git_stdout(path, &["rev-parse", "HEAD"]);
    fs::write(path.join("fixture"), b"second\n").unwrap();
    run_git(path, &["add", "fixture"]);
    commit(path, "second");
    let second = git_stdout(path, &["rev-parse", "HEAD"]);
    (first, second)
}

fn commit(path: &Path, message: &str) {
    run_git(
        path,
        &[
            "-c",
            "user.name=Hephaestus Tests",
            "-c",
            "user.email=hephaestus@example.invalid",
            "commit",
            "-qm",
            message,
        ],
    );
}

fn run_git(path: &Path, arguments: &[&str]) {
    assert!(
        Command::new("git")
            .arg("-C")
            .arg(path)
            .args(arguments)
            .status()
            .unwrap()
            .success()
    );
}

fn git_stdout(path: &Path, arguments: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(arguments)
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn artifact_file_count(directory: &TempDir) -> usize {
    fs::read_dir(directory.path().join("blobs"))
        .unwrap()
        .map(|entry| fs::read_dir(entry.unwrap().path()).unwrap().count())
        .sum()
}

fn evaluate(fixture: Fixture) -> Result<OperatorEvaluation, ArenaError> {
    evaluate_and_record(
        fixture.stores,
        context(),
        &fixture.world,
        EvaluationInputs {
            binding: &fixture.binding,
            visible: &fixture.visible,
            sealed: &fixture.sealed,
            parent: &fixture.parent,
            candidate: &fixture.candidate,
            evaluator: &fixture.evaluator,
        },
    )
}

#[test]
fn records_world_bound_runtime_derived_safe_evaluation() {
    let directory = TempDir::new().unwrap();
    let fixture = make_fixture(&directory);
    assert_eq!(fixture.binding.world_id(), fixture.world.id());
    assert_eq!(fixture.binding.seed(), 42);
    assert_eq!(fixture.binding.environment_id(), "environment-v1");
    assert_eq!(fixture.binding.budget(), budget_receipt());
    assert_eq!(
        fixture.binding.evaluator_id(),
        fixture.world.evaluator_artifact("arena.evaluator").unwrap()
    );
    assert!(matches!(
        fixture.sealed.candidate_tasks(),
        Err(ArenaError::VisibilityMismatch)
    ));
    let expected = (
        fixture.parent_genome_id.clone(),
        fixture.candidate_genome_id.clone(),
    );
    let operator = evaluate(fixture).unwrap();
    let recorded = operator.candidate_result();
    assert_eq!(recorded.summary.parent_genome_id, expected.0);
    assert_eq!(recorded.summary.candidate_genome_id, expected.1);
    assert_eq!(recorded.summary.parent_visible_correct, 1);
    assert_eq!(recorded.summary.candidate_visible_correct, 2);
    assert_eq!(recorded.summary.visible_total, 2);
    assert_eq!(recorded.event.actor, "arena-plane");
    assert_eq!(recorded.event.event_type, "evaluation.recorded");
    assert_eq!(operator.operator_scores().parent_visible_correct, 1);
    assert_eq!(operator.operator_scores().candidate_visible_correct, 2);
    assert_eq!(operator.operator_scores().parent_sealed_correct, 2);
    assert_eq!(operator.operator_scores().candidate_sealed_correct, 1);
    let public = serde_json::to_string(&recorded.summary).unwrap();
    for forbidden in ["sealed", "artifact", "source_revision", SEALED_SECRET] {
        assert!(!public.contains(forbidden));
    }
    let summary_value = serde_json::from_str::<serde_json::Value>(&public).unwrap();
    let candidate_debug = format!("{recorded:?}");
    for forbidden in [SEALED_SECRET, "parent_sealed", "candidate_sealed", "hash:"] {
        assert!(!candidate_debug.contains(forbidden));
    }
    assert!(
        directory
            .path()
            .join("evaluator-runs")
            .read_dir()
            .unwrap()
            .next()
            .is_none()
    );
    let keys = summary_value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        keys,
        std::collections::BTreeSet::from([
            "candidate_genome_id",
            "candidate_visible_correct",
            "evaluation_id",
            "parent_genome_id",
            "parent_visible_correct",
            "schema_version",
            "visible_total",
            "world_id",
        ])
    );
    assert!(
        String::from_utf8(operator.operator_parent_submission().unwrap())
            .unwrap()
            .contains("result:parent-task-sealed-a")
    );
    assert!(operator.operator_visible_inputs().unwrap().len() > 1);
    let candidate_result = operator.into_candidate_result();
    let candidate_debug = format!("{candidate_result:?}");
    for forbidden in ["OperatorReceipt", "EvaluationStores", SEALED_SECRET] {
        assert!(!candidate_debug.contains(forbidden));
    }
}

#[test]
fn constructors_reject_malformed_duplicate_empty_and_oversized_inputs() {
    assert!(
        EvaluationBinding::new("not-a-world", 1, "env", "0".repeat(64), budget_receipt(),).is_err()
    );
    assert!(
        EvaluationBinding::new(
            format!("hephaestus:world:{}", "G".repeat(64)),
            1,
            "env",
            "0".repeat(64),
            budget_receipt(),
        )
        .is_err()
    );
    assert!(TrustedTask::new("not valid", "input", "output").is_err());
    assert!(matches!(
        TrustedTask::new("task", "input", "x".repeat(65 * 1024)),
        Err(ArenaError::TextTooLarge { .. })
    ));
    assert!(matches!(
        TrustedManifest::new("empty", Visibility::Visible, vec![]),
        Err(ArenaError::EmptyManifest)
    ));
    assert!(matches!(
        TrustedManifest::new(
            "duplicates",
            Visibility::Visible,
            vec![
                TrustedTask::new("task", "a", "a").unwrap(),
                TrustedTask::new("task", "b", "b").unwrap(),
            ],
        ),
        Err(ArenaError::DuplicateTaskId(task)) if task == "task"
    ));
    assert!(matches!(
        TrialPlan::new([
            ("task".to_owned(), "result:one".to_owned()),
            ("task".to_owned(), "result:two".to_owned()),
        ]),
        Err(ArenaError::DuplicateTaskId(task)) if task == "task"
    ));
    let oversized =
        (0..=1_000).map(|index| (format!("task-{index}"), format!("result:run-{index}")));
    assert!(matches!(
        TrialPlan::new(oversized),
        Err(ArenaError::TooManyTasks)
    ));
    assert!(TrialPlan::new([("task".to_owned(), format!("result:{}", "r".repeat(128)),)]).is_ok());
    assert!(TrialPlan::new([("task".to_owned(), "not-a-result".to_owned())]).is_err());
}

#[test]
fn world_manifest_and_evaluator_semantics_fail_before_publication() {
    let directory = TempDir::new().unwrap();
    let fixture = make_fixture(&directory);
    let altered_visible = TrustedManifest::new(
        "visible-suite-v1",
        Visibility::Visible,
        vec![
            TrustedTask::new("task-visible-a", "changed input", VISIBLE_SECRET).unwrap(),
            TrustedTask::new("task-visible-b", "input visible b", "B").unwrap(),
        ],
    )
    .unwrap();
    let before = artifact_file_count(&directory);
    let result = evaluate_and_record(
        fixture.stores,
        context(),
        &fixture.world,
        EvaluationInputs {
            binding: &fixture.binding,
            visible: &altered_visible,
            sealed: &fixture.sealed,
            parent: &fixture.parent,
            candidate: &fixture.candidate,
            evaluator: &fixture.evaluator,
        },
    );
    assert!(matches!(
        result,
        Err(ArenaError::WorldArtifactMismatch("arena.visible_manifest"))
    ));
    assert_eq!(artifact_file_count(&directory), before);

    let evaluator_path = directory.path().join("wrong-id-evaluator");
    fs::copy(env!("CARGO_BIN_EXE_hephaestus-evaluator"), &evaluator_path).unwrap();
    fs::set_permissions(&evaluator_path, fs::Permissions::from_mode(0o700)).unwrap();
    let wrong_id = ArtifactId::for_bytes(b"unimplemented-evaluator-v2");
    assert!(matches!(
        IsolatedEvaluator::open_with_policy(
            directory.path().join("wrong-evaluator"),
            evaluator_path,
            wrong_id.as_str(),
            IsolationPolicy::unconfined_for_testing(),
            WorkerLimits::new(Duration::from_secs(1), 1024, 1024).unwrap(),
        ),
        Err(ArenaError::WorldArtifactMismatch("arena.evaluator"))
    ));
}

#[test]
fn isolated_evaluator_response_must_bind_the_exact_request() {
    let directory = TempDir::new().unwrap();
    let evaluator_path = directory.path().join("binding-forger");
    fs::write(
        &evaluator_path,
        concat!(
            "#!/bin/sh\n",
            "cat >/dev/null\n",
            "printf '%s' '",
            "{\"schema_version\":1,\"request_artifact_id\":\"",
            "0000000000000000000000000000000000000000000000000000000000000000",
            "\",\"scores\":{\"parent_visible_correct\":0,",
            "\"candidate_visible_correct\":0,\"parent_sealed_correct\":0,",
            "\"candidate_sealed_correct\":0,\"regressions\":0,",
            "\"improvements\":0,\"visible_total\":2,\"sealed_total\":2}}'\n"
        ),
    )
    .unwrap();
    fs::set_permissions(&evaluator_path, fs::Permissions::from_mode(0o700)).unwrap();
    let fixture = make_fixture_with_evaluator(&directory, evaluator_path);
    let before = artifact_file_count(&directory);
    let Err(error) = evaluate(fixture) else {
        panic!("a forged response must fail closed");
    };
    assert!(
        matches!(
            error,
            ArenaError::EvaluatorProtocol("response request binding mismatch")
        ),
        "unexpected evaluator error: {error:?}"
    );
    assert_eq!(artifact_file_count(&directory), before);
    assert!(
        directory
            .path()
            .join("evaluator-runs")
            .read_dir()
            .unwrap()
            .next()
            .is_none()
    );
}

#[test]
fn trial_plan_cannot_supply_a_fake_genome_identity() {
    assert!(matches!(
        TrialPlan::new([
            ("task-a".to_owned(), "result:run".to_owned()),
            ("task-b".to_owned(), "result:run".to_owned()),
        ]),
        Err(ArenaError::DuplicateRunEvent(event)) if event == "result:run"
    ));
    let directory = TempDir::new().unwrap();
    let fixture = make_fixture(&directory);
    let actual = fixture.candidate_genome_id.clone();
    let operator = evaluate(fixture).unwrap();
    let recorded = operator.candidate_result();
    assert_eq!(recorded.summary.candidate_genome_id, actual);
    assert_ne!(
        recorded.summary.candidate_genome_id,
        format!("hephaestus:genome:{}", "f".repeat(64))
    );
}

#[test]
fn unknown_and_forged_events_fail_without_writes() {
    let directory = TempDir::new().unwrap();
    let fixture = make_fixture(&directory);
    let before = artifact_file_count(&directory);
    let unknown = plan("parent", Some(("task-visible-a", "result:unknown")));
    let result = evaluate_and_record(
        fixture.stores,
        context(),
        &fixture.world,
        EvaluationInputs {
            binding: &fixture.binding,
            visible: &fixture.visible,
            sealed: &fixture.sealed,
            parent: &unknown,
            candidate: &fixture.candidate,
            evaluator: &fixture.evaluator,
        },
    );
    assert!(matches!(result, Err(ArenaError::UnknownRunEvent(_))));
    assert_eq!(artifact_file_count(&directory), before);

    let directory = TempDir::new().unwrap();
    let mut fixture = make_fixture(&directory);
    let attacker = RunResultSigner::from_seed([9; 32]);
    let forged_event_id = append_run(
        &mut fixture.stores,
        &attacker,
        &fixture.repository,
        "forged",
        &fixture.parent_genome_id,
        fixture.world.id(),
        &fixture.revision,
        "task-visible-a",
        task_input("task-visible-a"),
        RunCompletionReason::Success,
        b"forged",
    );
    let forged = plan("parent", Some(("task-visible-a", &forged_event_id)));
    let before = artifact_file_count(&directory);
    let result = evaluate_and_record(
        fixture.stores,
        context(),
        &fixture.world,
        EvaluationInputs {
            binding: &fixture.binding,
            visible: &fixture.visible,
            sealed: &fixture.sealed,
            parent: &forged,
            candidate: &fixture.candidate,
            evaluator: &fixture.evaluator,
        },
    );
    match result {
        Err(ArenaError::RunReceipt(_)) => {}
        Err(error) => panic!("unexpected error: {error:?}"),
        Ok(_) => panic!("forged runtime event was accepted"),
    }
    assert_eq!(artifact_file_count(&directory), before);
}

#[derive(Clone, Copy, Debug)]
enum ContextForgery {
    Task,
    Input,
    Seed,
    Environment,
    Budget,
}

#[test]
fn signed_results_cannot_be_relabelled_or_cross_experiment_boundaries() {
    for forgery in [
        ContextForgery::Task,
        ContextForgery::Input,
        ContextForgery::Seed,
        ContextForgery::Environment,
        ContextForgery::Budget,
    ] {
        let directory = TempDir::new().unwrap();
        let mut fixture = make_fixture(&directory);
        let genome_id = fixture.parent_genome_id.clone();
        let world_id = fixture.world.id().to_owned();
        let task_id = if matches!(forgery, ContextForgery::Task) {
            "other-task"
        } else {
            "task-visible-a"
        };
        let input = if matches!(forgery, ContextForgery::Input) {
            "altered visible input"
        } else {
            task_input("task-visible-a")
        };
        let seed = if matches!(forgery, ContextForgery::Seed) {
            43
        } else {
            42
        };
        let environment = if matches!(forgery, ContextForgery::Environment) {
            "other-environment-v1"
        } else {
            "environment-v1"
        };
        let budget = if matches!(forgery, ContextForgery::Budget) {
            Budget::new(Duration::from_secs(9), 1_048_576, 0).unwrap()
        } else {
            Budget::new(Duration::from_secs(10), 1_048_576, 0).unwrap()
        };
        let forged = append_run_with_context(
            &mut fixture.stores,
            &fixture.signer,
            &fixture.repository,
            &format!("context-forgery-{forgery:?}"),
            &genome_id,
            &world_id,
            &fixture.revision,
            task_id,
            input,
            seed,
            environment,
            budget,
            RunCompletionReason::Success,
            VISIBLE_SECRET.as_bytes(),
        );
        let plan = plan("parent", Some(("task-visible-a", &forged)));
        let before = artifact_file_count(&directory);
        let result = evaluate_and_record(
            fixture.stores,
            context(),
            &fixture.world,
            EvaluationInputs {
                binding: &fixture.binding,
                visible: &fixture.visible,
                sealed: &fixture.sealed,
                parent: &plan,
                candidate: &fixture.candidate,
                evaluator: &fixture.evaluator,
            },
        );
        assert!(
            matches!(
                result,
                Err(ArenaError::BindingMismatch("runtime experiment context"))
            ),
            "{forgery:?}"
        );
        assert_eq!(artifact_file_count(&directory), before, "{forgery:?}");
    }
}

#[test]
fn failed_run_and_cross_world_are_rejected() {
    let directory = TempDir::new().unwrap();
    let mut fixture = make_fixture(&directory);
    let world_id = fixture.world.id().to_owned();
    let genome_id = fixture.parent_genome_id.clone();
    let failed = append_run(
        &mut fixture.stores,
        &fixture.signer,
        &fixture.repository,
        "failed",
        &genome_id,
        &world_id,
        &fixture.revision,
        "task-visible-a",
        task_input("task-visible-a"),
        RunCompletionReason::OutputBudgetExceeded,
        b"partial",
    );
    let failed_plan = plan("parent", Some(("task-visible-a", &failed)));
    let result = evaluate_and_record(
        fixture.stores,
        context(),
        &fixture.world,
        EvaluationInputs {
            binding: &fixture.binding,
            visible: &fixture.visible,
            sealed: &fixture.sealed,
            parent: &failed_plan,
            candidate: &fixture.candidate,
            evaluator: &fixture.evaluator,
        },
    );
    assert!(matches!(result, Err(ArenaError::RunNotSuccessful(_))));

    let directory = TempDir::new().unwrap();
    let mut fixture = make_fixture(&directory);
    let genome_id = fixture.parent_genome_id.clone();
    let other_world = format!("hephaestus:world:{}", "f".repeat(64));
    let cross = append_run(
        &mut fixture.stores,
        &fixture.signer,
        &fixture.repository,
        "cross-world",
        &genome_id,
        &other_world,
        &fixture.revision,
        "task-visible-a",
        task_input("task-visible-a"),
        RunCompletionReason::Success,
        VISIBLE_SECRET.as_bytes(),
    );
    let cross_plan = plan("parent", Some(("task-visible-a", &cross)));
    let result = evaluate_and_record(
        fixture.stores,
        context(),
        &fixture.world,
        EvaluationInputs {
            binding: &fixture.binding,
            visible: &fixture.visible,
            sealed: &fixture.sealed,
            parent: &cross_plan,
            candidate: &fixture.candidate,
            evaluator: &fixture.evaluator,
        },
    );
    assert!(matches!(result, Err(ArenaError::RunWorldMismatch(_))));
}

#[test]
fn corrupt_or_missing_output_is_rejected_without_evaluation_writes() {
    let directory = TempDir::new().unwrap();
    let fixture = make_fixture(&directory);
    let history = fixture.stores.events.replay_verified().unwrap();
    let receipt = RunResultReceipt::parse_from_event(
        history
            .iter()
            .find(|event| event.event_id == "result:parent-task-visible-a")
            .unwrap(),
        &fixture.signer.verifier(),
    )
    .unwrap();
    let id = ArtifactId::parse(receipt.stdout_artifact_id).unwrap();
    fs::write(fixture.stores.artifacts.path_for(&id), b"tampered").unwrap();
    assert!(matches!(evaluate(fixture), Err(ArenaError::Ledger(_))));

    let directory = TempDir::new().unwrap();
    let mut fixture = make_fixture(&directory);
    let empty = fixture.stores.artifacts.put(b"").unwrap();
    let experiment = ExperimentContext::new(
        "task-visible-a",
        task_input("task-visible-a"),
        42,
        "environment-v1",
    )
    .unwrap();
    let spec = RunSpec::new_for_experiment_at_revision(
        "missing",
        &fixture.parent_genome_id,
        fixture.world.id(),
        &fixture.repository,
        &fixture.revision,
        task_input("task-visible-a"),
        CapabilitySet::new(false, false),
        Budget::new(Duration::from_secs(10), 1_048_576, 0).unwrap(),
        experiment,
    )
    .unwrap();
    let receipt = RunResultReceipt::from_run_spec(
        &spec,
        RunCompletionReason::Success,
        1,
        0,
        "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        empty.as_str(),
        vec![],
    )
    .unwrap();
    fixture
        .stores
        .events
        .append(fixture.signer.issue(receipt.clone(), 1).unwrap())
        .unwrap();
    let missing_event_id = receipt.event_id();
    let missing = plan("parent", Some(("task-visible-a", &missing_event_id)));
    let before = artifact_file_count(&directory);
    let result = evaluate_and_record(
        fixture.stores,
        context(),
        &fixture.world,
        EvaluationInputs {
            binding: &fixture.binding,
            visible: &fixture.visible,
            sealed: &fixture.sealed,
            parent: &missing,
            candidate: &fixture.candidate,
            evaluator: &fixture.evaluator,
        },
    );
    assert!(matches!(result, Err(ArenaError::Ledger(_))));
    assert_eq!(artifact_file_count(&directory), before);
}

#[test]
fn mixed_genome_revision_and_task_set_mismatches_are_rejected() {
    let directory = TempDir::new().unwrap();
    let fixture = make_fixture(&directory);
    let mixed = plan(
        "parent",
        Some(("task-visible-a", "result:candidate-task-visible-a")),
    );
    let result = evaluate_and_record(
        fixture.stores,
        context(),
        &fixture.world,
        EvaluationInputs {
            binding: &fixture.binding,
            visible: &fixture.visible,
            sealed: &fixture.sealed,
            parent: &mixed,
            candidate: &fixture.candidate,
            evaluator: &fixture.evaluator,
        },
    );
    assert!(matches!(result, Err(ArenaError::MixedSubmissionGenome)));

    let directory = TempDir::new().unwrap();
    let mut fixture = make_fixture(&directory);
    let genome_id = fixture.parent_genome_id.clone();
    let world_id = fixture.world.id().to_owned();
    let changed = append_run(
        &mut fixture.stores,
        &fixture.signer,
        &fixture.repository,
        "changed-revision",
        &genome_id,
        &world_id,
        &fixture.alternate_revision,
        "task-visible-a",
        task_input("task-visible-a"),
        RunCompletionReason::Success,
        VISIBLE_SECRET.as_bytes(),
    );
    let changed_plan = plan("parent", Some(("task-visible-a", &changed)));
    let result = evaluate_and_record(
        fixture.stores,
        context(),
        &fixture.world,
        EvaluationInputs {
            binding: &fixture.binding,
            visible: &fixture.visible,
            sealed: &fixture.sealed,
            parent: &changed_plan,
            candidate: &fixture.candidate,
            evaluator: &fixture.evaluator,
        },
    );
    assert!(
        matches!(result, Err(ArenaError::SourceRevisionMismatch(task)) if task == "task-visible-a")
    );

    let directory = TempDir::new().unwrap();
    let fixture = make_fixture(&directory);
    let incomplete = TrialPlan::new([(
        "task-visible-a".to_owned(),
        "result:parent-task-visible-a".to_owned(),
    )])
    .unwrap();
    let result = evaluate_and_record(
        fixture.stores,
        context(),
        &fixture.world,
        EvaluationInputs {
            binding: &fixture.binding,
            visible: &fixture.visible,
            sealed: &fixture.sealed,
            parent: &incomplete,
            candidate: &fixture.candidate,
            evaluator: &fixture.evaluator,
        },
    );
    assert!(matches!(result, Err(ArenaError::TaskSetMismatch { .. })));
}

#[test]
fn identical_retry_returns_existing_event_after_reopen() {
    let directory = TempDir::new().unwrap();
    let fixture = make_fixture(&directory);
    let first = evaluate_and_record(
        fixture.stores,
        context(),
        &fixture.world,
        EvaluationInputs {
            binding: &fixture.binding,
            visible: &fixture.visible,
            sealed: &fixture.sealed,
            parent: &fixture.parent,
            candidate: &fixture.candidate,
            evaluator: &fixture.evaluator,
        },
    )
    .unwrap();
    let event = first.candidate_result().event.clone();
    let reopened = EvaluationStores::open(
        directory.path().join("events.sqlite3"),
        directory.path().join("blobs"),
    )
    .unwrap();
    let second = evaluate_and_record(
        reopened,
        context(),
        &fixture.world,
        EvaluationInputs {
            binding: &fixture.binding,
            visible: &fixture.visible,
            sealed: &fixture.sealed,
            parent: &fixture.parent,
            candidate: &fixture.candidate,
            evaluator: &fixture.evaluator,
        },
    )
    .unwrap();
    assert_eq!(second.candidate_result().event, event);
    let stores = second.into_stores();
    assert_eq!(
        stores
            .events
            .replay_verified()
            .unwrap()
            .iter()
            .filter(|item| item.event_type == "evaluation.recorded")
            .count(),
        1
    );
}

#[test]
fn wrong_event_identity_and_conflicting_retry_fail_before_writes() {
    let directory = TempDir::new().unwrap();
    let fixture = make_fixture(&directory);
    let before = artifact_file_count(&directory);
    let mut invalid = context();
    invalid.event_id = "caller-event".to_owned();
    let result = evaluate_and_record(
        fixture.stores,
        invalid,
        &fixture.world,
        EvaluationInputs {
            binding: &fixture.binding,
            visible: &fixture.visible,
            sealed: &fixture.sealed,
            parent: &fixture.parent,
            candidate: &fixture.candidate,
            evaluator: &fixture.evaluator,
        },
    );
    assert!(matches!(
        result,
        Err(ArenaError::InvalidId {
            field: "event_id",
            ..
        })
    ));
    assert_eq!(artifact_file_count(&directory), before);

    let directory = TempDir::new().unwrap();
    let mut fixture = make_fixture(&directory);
    let genome_id = fixture.candidate_genome_id.clone();
    let world_id = fixture.world.id().to_owned();
    let alternate_event = append_run(
        &mut fixture.stores,
        &fixture.signer,
        &fixture.repository,
        "candidate-visible-a-alternate",
        &genome_id,
        &world_id,
        &fixture.revision,
        "task-visible-a",
        task_input("task-visible-a"),
        RunCompletionReason::Success,
        b"changed",
    );
    let alternate = plan("candidate", Some(("task-visible-a", &alternate_event)));
    let first = evaluate_and_record(
        fixture.stores,
        context(),
        &fixture.world,
        EvaluationInputs {
            binding: &fixture.binding,
            visible: &fixture.visible,
            sealed: &fixture.sealed,
            parent: &fixture.parent,
            candidate: &fixture.candidate,
            evaluator: &fixture.evaluator,
        },
    )
    .unwrap();
    let before = artifact_file_count(&directory);
    let result = evaluate_and_record(
        first.into_stores(),
        context(),
        &fixture.world,
        EvaluationInputs {
            binding: &fixture.binding,
            visible: &fixture.visible,
            sealed: &fixture.sealed,
            parent: &fixture.parent,
            candidate: &alternate,
            evaluator: &fixture.evaluator,
        },
    );
    assert!(matches!(result, Err(ArenaError::EvaluationConflict(_))));
    assert_eq!(artifact_file_count(&directory), before);
}
