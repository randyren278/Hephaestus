use std::{fs, process::Command, thread, time::Duration};

use hephaestus_core::authority::CapabilitySet;
use hephaestus_runtime::{
    Budget, DeterministicRuntime, IsolationBackend, IsolationPolicy, Provider, ProviderInvocation,
    RunSpec, RunStatus, RuntimeAdapter, RuntimeError, SandboxManager,
};
use tempfile::tempdir;

#[test]
fn isolated_worktrees_bind_expiring_tokens_to_one_run() {
    let repository = repository_fixture();
    let sandboxes = tempdir().expect("sandbox directory");
    let manager = SandboxManager::open(sandboxes.path(), Duration::from_secs(30))
        .expect("open sandbox manager");
    let first_spec = spec("run-one", repository.path(), 1_000_000, false);
    let second_spec = spec("run-two", repository.path(), 1_000_000, false);
    let (first, first_token) = manager.create(&first_spec).expect("create first sandbox");
    let (second, second_token) = manager.create(&second_spec).expect("create second sandbox");

    assert_ne!(first.worktree(), second.worktree());
    assert!(
        first
            .authorize(&first_token, first_spec.capabilities())
            .is_ok()
    );
    assert!(matches!(
        first.authorize(&second_token, first_spec.capabilities()),
        Err(RuntimeError::CapabilityDenied)
    ));
    assert!(matches!(
        first.authorize(&first_token, CapabilitySet::new(true, true)),
        Err(RuntimeError::CapabilityDenied)
    ));

    let protected = tempdir().expect("protected directory");
    let protected_file = protected.path().join("canonical-secret");
    fs::write(&protected_file, b"must remain hidden").expect("write protected fixture");
    let policy = IsolationPolicy::detect(vec![protected.path().to_owned()]);
    let sibling_probe = ProviderInvocation::deterministic(
        "/bin/cat",
        [second
            .worktree()
            .join("fixture.txt")
            .to_string_lossy()
            .into_owned()],
        [],
    )
    .expect("build sibling probe");
    match policy.backend() {
        IsolationBackend::MacOsSeatbelt => {
            let own_probe = ProviderInvocation::deterministic(
                "/bin/cat",
                [first
                    .worktree()
                    .join("fixture.txt")
                    .to_string_lossy()
                    .into_owned()],
                [],
            )
            .expect("build own probe");
            assert!(
                policy
                    .command(&own_probe, &first)
                    .expect("build own sandbox command")
                    .status()
                    .expect("run own sandbox probe")
                    .success()
            );
            assert!(
                !policy
                    .command(&sibling_probe, &first)
                    .expect("build sibling sandbox command")
                    .status()
                    .expect("run sibling sandbox probe")
                    .success()
            );
            let protected_probe = ProviderInvocation::deterministic(
                "/bin/cat",
                [protected_file.to_string_lossy().into_owned()],
                [],
            )
            .expect("build protected probe");
            assert!(
                !policy
                    .command(&protected_probe, &first)
                    .expect("build protected sandbox command")
                    .status()
                    .expect("run protected sandbox probe")
                    .success()
            );
        }
        IsolationBackend::Unavailable => assert!(matches!(
            policy.command(&sibling_probe, &first),
            Err(RuntimeError::Unsupported(_))
        )),
    }

    first.cleanup().expect("clean first sandbox");
    second.cleanup().expect("clean second sandbox");
}

#[test]
fn deterministic_runtime_starts_resumes_interrupts_and_snapshots_real_worktrees() {
    let repository = repository_fixture();
    let sandboxes = tempdir().expect("sandbox directory");
    let manager = SandboxManager::open(sandboxes.path(), Duration::from_secs(30))
        .expect("open sandbox manager");
    let run_spec = spec("reference", repository.path(), 1_000_000, false);
    let (sandbox, token) = manager.create(&run_spec).expect("create sandbox");
    let mut runtime = DeterministicRuntime::default();

    let codex =
        ProviderInvocation::codex("codex", &run_spec, &sandbox).expect("build Codex invocation");
    assert_eq!(codex.provider(), Provider::Codex);
    assert!(
        codex
            .arguments()
            .iter()
            .any(|argument| argument == "--ephemeral")
    );
    assert!(
        codex
            .arguments()
            .iter()
            .any(|argument| argument == "workspace-write")
    );
    assert!(
        !codex
            .arguments()
            .iter()
            .any(|argument| argument == run_spec.prompt())
    );
    assert_eq!(codex.stdin(), run_spec.prompt().as_bytes());
    let claude =
        ProviderInvocation::claude("claude", &run_spec, &sandbox).expect("build Claude invocation");
    assert_eq!(claude.provider(), Provider::Claude);
    assert!(
        claude
            .arguments()
            .windows(2)
            .any(|pair| pair == ["--permission-mode", "dontAsk"])
    );

    let handle = runtime
        .start(&run_spec, &sandbox, &token)
        .expect("start reference runtime");
    assert_eq!(handle.run_id, "reference");
    let snapshot = runtime.snapshot("reference").expect("snapshot run");
    assert_eq!(snapshot.status, RunStatus::Succeeded);
    let output: serde_json::Value = serde_json::from_slice(
        &fs::read(&snapshot.stdout_path).expect("read deterministic output"),
    )
    .expect("decode deterministic output");
    assert_eq!(output["schema_version"], 1);
    assert_eq!(output["files"][0]["path"], "fixture.txt");

    runtime
        .resume(&run_spec, &sandbox, &token, "checkpoint-1")
        .expect("resume reference runtime");
    let resumed = runtime.snapshot("reference").expect("snapshot resumed run");
    let resumed_output: serde_json::Value =
        serde_json::from_slice(&fs::read(&resumed.stdout_path).expect("read resumed output"))
            .expect("decode resumed output");
    assert_eq!(resumed_output["checkpoint"], "checkpoint-1");

    runtime
        .resume(&run_spec, &sandbox, &token, "checkpoint-2")
        .expect("resume for interrupt");
    runtime.interrupt("reference").expect("interrupt run");
    assert_eq!(
        runtime
            .snapshot("reference")
            .expect("snapshot interrupt")
            .status,
        RunStatus::Interrupted
    );
    sandbox.cleanup().expect("clean sandbox");
}

#[test]
fn runtime_fails_closed_for_expiry_network_and_output_budget() {
    let repository = repository_fixture();
    let sandboxes = tempdir().expect("sandbox directory");
    let manager = SandboxManager::open(sandboxes.path(), Duration::from_secs(30))
        .expect("open sandbox manager");
    let network_spec = spec("network", repository.path(), 1_000_000, true);
    let (network_sandbox, network_token) = manager
        .create(&network_spec)
        .expect("create network sandbox");
    let network_invocation = ProviderInvocation::codex("codex", &network_spec, &network_sandbox)
        .expect("build network invocation");
    assert!(
        network_invocation
            .arguments()
            .iter()
            .any(|argument| argument == "sandbox_workspace_write.network_access=true")
    );
    let mut runtime = DeterministicRuntime::default();
    assert!(matches!(
        runtime.start(&network_spec, &network_sandbox, &network_token),
        Err(RuntimeError::CapabilityDenied)
    ));
    network_sandbox.cleanup().expect("clean network sandbox");

    let tiny_spec = spec("tiny", repository.path(), 1, false);
    let (tiny_sandbox, tiny_token) = manager.create(&tiny_spec).expect("create tiny sandbox");
    runtime
        .start(&tiny_spec, &tiny_sandbox, &tiny_token)
        .expect("start tiny run");
    assert_eq!(
        runtime.snapshot("tiny").expect("snapshot tiny run").status,
        RunStatus::Failed
    );
    tiny_sandbox.cleanup().expect("clean tiny sandbox");

    let expiring_root = tempdir().expect("expiring sandbox directory");
    let expiring = SandboxManager::open(expiring_root.path(), Duration::from_millis(1))
        .expect("open expiring manager");
    let expired_spec = spec("expired", repository.path(), 1_000_000, false);
    let (expired_sandbox, expired_token) = expiring
        .create(&expired_spec)
        .expect("create expiring sandbox");
    thread::sleep(Duration::from_millis(5));
    assert!(matches!(
        runtime.start(&expired_spec, &expired_sandbox, &expired_token),
        Err(RuntimeError::CapabilityDenied)
    ));
    expired_sandbox.cleanup().expect("clean expired sandbox");
}

#[test]
fn budgets_and_run_specs_reject_invalid_inputs() {
    assert!(Budget::new(Duration::ZERO, 1, 0).is_err());
    assert!(Budget::new(Duration::from_secs(1), 0, 0).is_err());
    let budget = Budget::new(Duration::from_secs(2), 10, 7).expect("valid budget");
    assert_eq!(budget.wall(), Duration::from_secs(2));
    assert_eq!(budget.maximum_cost_microusd(), 7);

    let repository = repository_fixture();
    assert!(matches!(
        RunSpec::new(
            "../escape",
            "genome",
            "world",
            repository.path(),
            "prompt",
            CapabilitySet::new(false, false),
            budget
        ),
        Err(RuntimeError::InvalidSpec(_))
    ));
    assert!(matches!(
        RunSpec::new(
            "valid",
            " ",
            "world",
            repository.path(),
            "prompt",
            CapabilitySet::new(false, false),
            budget
        ),
        Err(RuntimeError::InvalidSpec(_))
    ));
    assert!(matches!(
        RunSpec::new(
            "valid",
            "genome",
            "world",
            repository.path().join("missing"),
            "prompt",
            CapabilitySet::new(false, false),
            budget
        ),
        Err(RuntimeError::InvalidSpec(_))
    ));
}

#[test]
fn provider_and_sandbox_setup_reject_invalid_inputs() {
    let budget = Budget::new(Duration::from_secs(2), 10, 7).expect("valid budget");
    let repository = repository_fixture();
    let root = tempdir().expect("sandbox root");
    assert!(SandboxManager::open(root.path(), Duration::ZERO).is_err());
    let unsafe_root = root.path().join("file");
    fs::write(&unsafe_root, b"not a directory").expect("write unsafe root");
    assert!(SandboxManager::open(&unsafe_root, Duration::from_secs(1)).is_err());
    let manager = SandboxManager::open(root.path().join("valid"), Duration::from_secs(30))
        .expect("open valid manager");
    let read_only = RunSpec::new(
        "read-only",
        "genome",
        "world",
        repository.path(),
        "prompt",
        CapabilitySet::new(false, false),
        budget,
    )
    .expect("read-only spec");
    let (sandbox, token) = manager
        .create(&read_only)
        .expect("create read-only sandbox");
    let codex = ProviderInvocation::codex("codex", &read_only, &sandbox).expect("Codex command");
    assert_eq!(codex.program(), std::path::Path::new("codex"));
    assert!(
        codex
            .arguments()
            .iter()
            .any(|argument| argument == "read-only")
    );
    let claude =
        ProviderInvocation::claude("claude", &read_only, &sandbox).expect("Claude command");
    assert!(claude.arguments().iter().any(|argument| argument == "Read"));
    assert!(ProviderInvocation::codex("", &read_only, &sandbox).is_err());

    let mismatched = spec("mismatch", repository.path(), 1000, true);
    assert!(matches!(
        ProviderInvocation::claude("claude", &mismatched, &sandbox),
        Err(RuntimeError::CapabilityDenied)
    ));
    let mut runtime = DeterministicRuntime::default();
    assert_eq!(runtime.provider(), Provider::Deterministic);
    runtime
        .start(&read_only, &sandbox, &token)
        .expect("start read-only run");
    assert!(runtime.start(&read_only, &sandbox, &token).is_err());
    assert!(runtime.resume(&read_only, &sandbox, &token, " ").is_err());
    assert!(runtime.interrupt("missing").is_err());
    assert!(runtime.snapshot("missing").is_err());
    sandbox.cleanup().expect("clean read-only sandbox");

    let invalid_repository = tempdir().expect("invalid repository");
    let invalid_spec = RunSpec::new(
        "not-git",
        "genome",
        "world",
        invalid_repository.path(),
        "prompt",
        CapabilitySet::new(false, false),
        budget,
    )
    .expect("non-Git spec is structurally valid");
    assert!(matches!(
        manager.create(&invalid_spec),
        Err(RuntimeError::Git(_))
    ));

    let removed_repository = repository_fixture();
    let removed_spec = spec("cleanup-failure", removed_repository.path(), 1000, false);
    let (orphaned, _token) = manager
        .create(&removed_spec)
        .expect("create cleanup-failure sandbox");
    let removed_path = removed_repository.keep();
    fs::remove_dir_all(removed_path).expect("remove source repository");
    assert!(matches!(orphaned.cleanup(), Err(RuntimeError::Git(_))));
}

fn repository_fixture() -> tempfile::TempDir {
    let repository = tempdir().expect("repository directory");
    run_git(repository.path(), &["init", "-q"]);
    fs::write(
        repository.path().join("fixture.txt"),
        b"deterministic fixture\n",
    )
    .expect("write fixture");
    run_git(repository.path(), &["add", "fixture.txt"]);
    run_git(
        repository.path(),
        &[
            "-c",
            "user.name=Hephaestus Tests",
            "-c",
            "user.email=hephaestus@example.invalid",
            "commit",
            "-qm",
            "fixture",
        ],
    );
    repository
}

fn spec(
    run_id: &str,
    repository: &std::path::Path,
    maximum_output_bytes: usize,
    network: bool,
) -> RunSpec {
    RunSpec::new(
        run_id,
        "hephaestus:genome:test",
        "hephaestus:world:test",
        repository,
        "inventory the isolated worktree",
        CapabilitySet::new(true, network),
        Budget::new(Duration::from_secs(5), maximum_output_bytes, 0).expect("budget"),
    )
    .expect("RunSpec")
}

fn run_git(repository: &std::path::Path, arguments: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(arguments)
        .status()
        .expect("run git");
    assert!(status.success());
}
