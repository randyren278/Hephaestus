use std::{collections::BTreeMap, fs, process::Command, time::Duration};

use hephaestus_experience::{
    EvidenceRecorder, RecordedRuntime, RedactionPolicy, RetentionLimits, TraceKind, TraceReceipt,
};
use hephaestus_runtime::{
    AdapterCapabilities, Budget, CapabilityToken, DeterministicRuntime, Provider, RunHandle,
    RunSnapshot, RunSpec, RunStatus, RuntimeAdapter, RuntimeError, Sandbox, SandboxManager,
};
use tempfile::{TempDir, tempdir};

#[test]
fn offline_run_records_redacted_lifecycle_observations_checkpoints_and_cost() {
    let repository = repository_fixture();
    let sandboxes = tempdir().expect("sandbox directory");
    let manager = SandboxManager::open(sandboxes.path(), Duration::from_secs(30))
        .expect("open sandbox manager");
    let spec = spec("observed-run", repository.path(), false);
    let (sandbox, token) = manager.create(&spec).expect("create sandbox");
    let evidence = tempdir().expect("evidence directory");
    let mut runtime = RecordedRuntime::new(
        DeterministicRuntime::default(),
        recorder(&evidence, 32, 16_384),
    )
    .expect("create recorded runtime");

    runtime
        .start(&spec, &sandbox, &token)
        .expect("start observed run");
    runtime
        .record_observable(
            spec.run_id(),
            TraceKind::ToolCalled,
            BTreeMap::from([
                ("tool".to_owned(), "read_file".to_owned()),
                ("authorization".to_owned(), "known-secret".to_owned()),
            ]),
        )
        .expect("record visible tool call");
    let terminal = runtime.snapshot(spec.run_id()).expect("snapshot run");
    assert_eq!(terminal.status, RunStatus::Succeeded);
    assert_eq!(
        runtime.provider(),
        hephaestus_runtime::Provider::Deterministic
    );
    assert!(runtime.report_capabilities().snapshot);

    assert_initial_evidence(&runtime, &spec);

    runtime.snapshot(spec.run_id()).expect("repeat snapshot");
    assert_eq!(
        runtime
            .evidence()
            .replay_verified()
            .expect("replay deduplicated evidence")
            .len(),
        4
    );

    runtime
        .resume(&spec, &sandbox, &token, "checkpoint-secret")
        .expect("resume observed run");
    runtime
        .snapshot(spec.run_id())
        .expect("snapshot resumed run");
    let resumed_history = runtime
        .evidence()
        .replay_verified()
        .expect("replay resumed run");
    assert_eq!(resumed_history.len(), 7);
    let checkpoint: TraceReceipt =
        serde_json::from_slice(&resumed_history[4].payload).expect("decode checkpoint receipt");
    assert_eq!(checkpoint.kind, TraceKind::CheckpointCreated);
    let checkpoint_artifact = String::from_utf8(
        runtime
            .evidence()
            .artifact(&checkpoint.artifact_id)
            .expect("read checkpoint trace"),
    )
    .expect("UTF-8 checkpoint trace");
    assert!(!checkpoint_artifact.contains("checkpoint-secret"));
    sandbox.cleanup().expect("clean sandbox");
}

#[test]
fn rejected_run_records_capability_denial_without_becoming_observable() {
    let repository = repository_fixture();
    let sandboxes = tempdir().expect("sandbox directory");
    let manager = SandboxManager::open(sandboxes.path(), Duration::from_secs(30))
        .expect("open sandbox manager");
    let spec = spec("denied-run", repository.path(), true);
    let (sandbox, token) = manager.create(&spec).expect("create sandbox");
    let evidence = tempdir().expect("evidence directory");
    let mut runtime = RecordedRuntime::new(
        DeterministicRuntime::default(),
        recorder(&evidence, 8, 16_384),
    )
    .expect("create recorded runtime");

    assert!(matches!(
        runtime.start(&spec, &sandbox, &token),
        Err(RuntimeError::CapabilityDenied)
    ));
    assert!(matches!(
        runtime.record_observable("denied-run", TraceKind::Retry, BTreeMap::new()),
        Err(RuntimeError::InvalidSpec(_))
    ));
    let history = runtime.evidence().replay_verified().expect("replay denial");
    assert_eq!(history.len(), 1);
    let receipt: TraceReceipt =
        serde_json::from_slice(&history[0].payload).expect("decode denial receipt");
    assert_eq!(receipt.kind, TraceKind::CapabilityDenied);
    sandbox.cleanup().expect("clean sandbox");
}

#[test]
fn run_is_interrupted_if_required_start_evidence_cannot_be_persisted() {
    let repository = repository_fixture();
    let sandboxes = tempdir().expect("sandbox directory");
    let manager = SandboxManager::open(sandboxes.path(), Duration::from_secs(30))
        .expect("open sandbox manager");
    let spec = spec("fail-closed-run", repository.path(), false);
    let (sandbox, token) = manager.create(&spec).expect("create sandbox");
    let evidence = tempdir().expect("evidence directory");
    let mut runtime =
        RecordedRuntime::new(DeterministicRuntime::default(), recorder(&evidence, 8, 1))
            .expect("create recorded runtime");

    assert!(matches!(
        runtime.start(&spec, &sandbox, &token),
        Err(RuntimeError::Evidence(_))
    ));
    assert_eq!(
        runtime
            .inner_mut()
            .snapshot(spec.run_id())
            .expect("inspect interrupted inner runtime")
            .status,
        RunStatus::Interrupted
    );
    assert!(
        runtime
            .evidence()
            .replay_verified()
            .expect("replay empty evidence")
            .is_empty()
    );
    assert!(matches!(
        runtime.record_observable(spec.run_id(), TraceKind::Retry, BTreeMap::new()),
        Err(RuntimeError::InvalidSpec(_))
    ));
    sandbox.cleanup().expect("clean sandbox");
}

#[test]
fn adapter_failures_and_running_snapshots_are_canonical_events() {
    let repository = repository_fixture();
    let sandboxes = tempdir().expect("sandbox directory");
    let manager = SandboxManager::open(sandboxes.path(), Duration::from_secs(30))
        .expect("open sandbox manager");
    let spec = spec("scripted-run", repository.path(), false);
    let (sandbox, token) = manager.create(&spec).expect("create sandbox");
    let evidence = tempdir().expect("evidence directory");
    let mut runtime = RecordedRuntime::new(
        ScriptedRuntime::new(Provider::Codex),
        recorder(&evidence, 16, 16_384),
    )
    .expect("create recorded runtime");

    runtime
        .start(&spec, &sandbox, &token)
        .expect("start scripted run");
    assert_eq!(runtime.inner().provider, Provider::Codex);
    assert_eq!(
        runtime
            .snapshot(spec.run_id())
            .expect("running snapshot")
            .status,
        RunStatus::Running
    );
    runtime.inner_mut().status = RunStatus::Failed;
    runtime.inner_mut().exit_code = Some(9);
    assert_eq!(
        runtime
            .snapshot(spec.run_id())
            .expect("failed snapshot")
            .status,
        RunStatus::Failed
    );
    runtime
        .interrupt(spec.run_id())
        .expect("interrupt completed run");

    runtime.inner_mut().failure = FailurePoint::Resume;
    assert!(matches!(
        runtime.resume(&spec, &sandbox, &token, "checkpoint"),
        Err(RuntimeError::Unsupported(_))
    ));
    runtime.inner_mut().failure = FailurePoint::Snapshot;
    assert!(matches!(
        runtime.snapshot(spec.run_id()),
        Err(RuntimeError::Unsupported(_))
    ));
    runtime.inner_mut().failure = FailurePoint::Interrupt;
    assert!(matches!(
        runtime.interrupt(spec.run_id()),
        Err(RuntimeError::Unsupported(_))
    ));

    let kinds: Vec<TraceKind> = runtime
        .evidence()
        .replay_verified()
        .expect("replay scripted evidence")
        .iter()
        .map(|event| {
            serde_json::from_slice::<TraceReceipt>(&event.payload)
                .expect("decode scripted receipt")
                .kind
        })
        .collect();
    assert_eq!(
        kinds,
        [
            TraceKind::LifecycleStarted,
            TraceKind::CheckpointCreated,
            TraceKind::LifecycleCompleted,
            TraceKind::Error,
            TraceKind::Error,
            TraceKind::Error,
        ]
    );
    sandbox.cleanup().expect("clean sandbox");
}

#[test]
fn generic_start_failure_and_unrecordable_resume_fail_closed() {
    let repository = repository_fixture();
    let sandboxes = tempdir().expect("sandbox directory");
    let manager = SandboxManager::open(sandboxes.path(), Duration::from_secs(30))
        .expect("open sandbox manager");

    let failed_spec = spec("failed-start", repository.path(), false);
    let (failed_sandbox, failed_token) = manager
        .create(&failed_spec)
        .expect("create failed-start sandbox");
    let failed_evidence = tempdir().expect("failed-start evidence");
    let mut failed = RecordedRuntime::new(
        ScriptedRuntime {
            failure: FailurePoint::Start,
            ..ScriptedRuntime::new(Provider::Claude)
        },
        recorder(&failed_evidence, 8, 16_384),
    )
    .expect("create failed-start runtime");
    assert!(matches!(
        failed.start(&failed_spec, &failed_sandbox, &failed_token),
        Err(RuntimeError::Unsupported(_))
    ));
    let failure: TraceReceipt = serde_json::from_slice(
        &failed
            .evidence()
            .replay_verified()
            .expect("replay start failure")[0]
            .payload,
    )
    .expect("decode start failure");
    assert_eq!(failure.kind, TraceKind::Error);
    failed_sandbox
        .cleanup()
        .expect("clean failed-start sandbox");

    let resume_spec = spec("resume-evidence-failure", repository.path(), false);
    let (resume_sandbox, resume_token) =
        manager.create(&resume_spec).expect("create resume sandbox");
    let resume_evidence = tempdir().expect("resume evidence");
    let mut resumed = RecordedRuntime::new(
        DeterministicRuntime::default(),
        recorder(&resume_evidence, 1, 16_384),
    )
    .expect("create resume runtime");
    resumed
        .start(&resume_spec, &resume_sandbox, &resume_token)
        .expect("start resume runtime");
    assert!(matches!(
        resumed.resume(&resume_spec, &resume_sandbox, &resume_token, "checkpoint"),
        Err(RuntimeError::Evidence(_))
    ));
    assert_eq!(
        resumed
            .inner_mut()
            .snapshot(resume_spec.run_id())
            .expect("inspect interrupted resumed runtime")
            .status,
        RunStatus::Interrupted
    );
    assert!(matches!(
        resumed.record_observable(resume_spec.run_id(), TraceKind::Retry, BTreeMap::new()),
        Err(RuntimeError::InvalidSpec(_))
    ));
    resume_sandbox.cleanup().expect("clean resume sandbox");
}

#[test]
fn active_interrupt_records_one_terminal_reason() {
    let repository = repository_fixture();
    let sandboxes = tempdir().expect("sandbox directory");
    let manager = SandboxManager::open(sandboxes.path(), Duration::from_secs(30))
        .expect("open sandbox manager");
    let spec = spec("interrupted-run", repository.path(), false);
    let (sandbox, token) = manager.create(&spec).expect("create sandbox");
    let evidence = tempdir().expect("evidence directory");
    let mut runtime = RecordedRuntime::new(
        ScriptedRuntime::new(Provider::Claude),
        recorder(&evidence, 8, 16_384),
    )
    .expect("create recorded runtime");

    runtime
        .start(&spec, &sandbox, &token)
        .expect("start interruptible run");
    runtime
        .interrupt(spec.run_id())
        .expect("interrupt active run");
    runtime
        .interrupt(spec.run_id())
        .expect("repeat interrupt is evidence-idempotent");
    let history = runtime
        .evidence()
        .replay_verified()
        .expect("replay interrupted run");
    assert_eq!(history.len(), 2);
    let completion: TraceReceipt =
        serde_json::from_slice(&history[1].payload).expect("decode interrupt completion");
    assert_eq!(completion.kind, TraceKind::LifecycleCompleted);
    let artifact = String::from_utf8(
        runtime
            .evidence()
            .artifact(&completion.artifact_id)
            .expect("read interrupt completion"),
    )
    .expect("UTF-8 interrupt completion");
    assert!(artifact.contains("operator_interrupt"));
    sandbox.cleanup().expect("clean sandbox");
}

fn assert_initial_evidence(runtime: &RecordedRuntime<DeterministicRuntime>, spec: &RunSpec) {
    let history = runtime
        .evidence()
        .replay_verified()
        .expect("replay evidence");
    assert_eq!(history.len(), 4);
    let receipts: Vec<TraceReceipt> = history
        .iter()
        .map(|event| {
            assert_eq!(event.event_type, "trace.recorded");
            serde_json::from_slice(&event.payload).expect("decode trace receipt")
        })
        .collect();
    assert_eq!(receipts[0].kind, TraceKind::LifecycleStarted);
    assert_eq!(receipts[1].kind, TraceKind::ToolCalled);
    assert_eq!(receipts[2].kind, TraceKind::LifecycleCompleted);
    assert_eq!(receipts[3].kind, TraceKind::CostObserved);
    assert!(receipts.iter().all(|receipt| {
        receipt.provenance.run_id() == "observed-run"
            && receipt.provenance.genome_id() == "genome-1"
            && receipt.provenance.world_id() == "world-1"
    }));
    let artifacts: Vec<String> = receipts
        .iter()
        .map(|receipt| {
            String::from_utf8(
                runtime
                    .evidence()
                    .artifact(&receipt.artifact_id)
                    .expect("read trace artifact"),
            )
            .expect("UTF-8 trace artifact")
        })
        .collect();
    assert!(!artifacts[1].contains("known-secret"));
    assert!(artifacts[1].contains("[REDACTED]"));
    assert!(
        artifacts
            .iter()
            .all(|artifact| !artifact.contains(spec.prompt()))
    );
    assert!(artifacts[2].contains("latency_millis"));
}

fn recorder(directory: &TempDir, maximum_records: usize, maximum_bytes: usize) -> EvidenceRecorder {
    EvidenceRecorder::open(
        directory.path().join("events.sqlite3"),
        directory.path().join("artifacts"),
        RedactionPolicy::new(["known-secret".to_owned()]),
        RetentionLimits::new(maximum_records, maximum_bytes).expect("retention limits"),
    )
    .expect("open evidence recorder")
}

fn spec(run_id: &str, repository: &std::path::Path, network: bool) -> RunSpec {
    RunSpec::new(
        run_id,
        "genome-1",
        "world-1",
        repository,
        "inventory the repository without exposing this prompt",
        hephaestus_core::authority::CapabilitySet::new(true, network),
        Budget::new(Duration::from_secs(2), 1_000_000, 0).expect("budget"),
    )
    .expect("run spec")
}

fn repository_fixture() -> TempDir {
    let directory = tempdir().expect("repository directory");
    run_git(directory.path(), &["init", "--quiet"]);
    run_git(
        directory.path(),
        &["config", "user.email", "tests@hephaestus.invalid"],
    );
    run_git(
        directory.path(),
        &["config", "user.name", "Hephaestus Tests"],
    );
    fs::write(
        directory.path().join("fixture.txt"),
        b"deterministic fixture\n",
    )
    .expect("write repository fixture");
    run_git(directory.path(), &["add", "fixture.txt"]);
    run_git(directory.path(), &["commit", "--quiet", "-m", "fixture"]);
    directory
}

fn run_git(directory: &std::path::Path, arguments: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(directory)
        .args(arguments)
        .status()
        .expect("run git fixture command");
    assert!(
        status.success(),
        "git fixture command failed: {arguments:?}"
    );
}

struct ScriptedRuntime {
    provider: Provider,
    status: RunStatus,
    exit_code: Option<i32>,
    stdout_path: std::path::PathBuf,
    stderr_path: std::path::PathBuf,
    failure: FailurePoint,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum FailurePoint {
    None,
    Start,
    Resume,
    Interrupt,
    Snapshot,
}

impl ScriptedRuntime {
    fn new(provider: Provider) -> Self {
        Self {
            provider,
            status: RunStatus::Running,
            exit_code: None,
            stdout_path: std::path::PathBuf::new(),
            stderr_path: std::path::PathBuf::new(),
            failure: FailurePoint::None,
        }
    }
}

impl RuntimeAdapter for ScriptedRuntime {
    fn provider(&self) -> Provider {
        self.provider
    }

    fn report_capabilities(&self) -> AdapterCapabilities {
        AdapterCapabilities {
            resume: true,
            interrupt: true,
            snapshot: true,
            authority: hephaestus_core::authority::CapabilitySet::new(true, false),
        }
    }

    fn start(
        &mut self,
        spec: &RunSpec,
        sandbox: &Sandbox,
        _token: &CapabilityToken,
    ) -> Result<RunHandle, RuntimeError> {
        if self.failure == FailurePoint::Start {
            return Err(RuntimeError::Unsupported("injected start failure"));
        }
        self.stdout_path = sandbox.execution_dir().join("stdout.log");
        self.stderr_path = sandbox.execution_dir().join("stderr.log");
        Ok(RunHandle {
            run_id: spec.run_id().to_owned(),
            provider: self.provider,
        })
    }

    fn resume(
        &mut self,
        spec: &RunSpec,
        sandbox: &Sandbox,
        token: &CapabilityToken,
        _checkpoint: &str,
    ) -> Result<RunHandle, RuntimeError> {
        if self.failure == FailurePoint::Resume {
            return Err(RuntimeError::Unsupported("injected resume failure"));
        }
        self.start(spec, sandbox, token)
    }

    fn interrupt(&mut self, _run_id: &str) -> Result<(), RuntimeError> {
        if self.failure == FailurePoint::Interrupt {
            return Err(RuntimeError::Unsupported("injected interrupt failure"));
        }
        self.status = RunStatus::Interrupted;
        Ok(())
    }

    fn snapshot(&mut self, run_id: &str) -> Result<RunSnapshot, RuntimeError> {
        if self.failure == FailurePoint::Snapshot {
            return Err(RuntimeError::Unsupported("injected snapshot failure"));
        }
        Ok(RunSnapshot {
            run_id: run_id.to_owned(),
            status: self.status,
            exit_code: self.exit_code,
            stdout_path: self.stdout_path.clone(),
            stderr_path: self.stderr_path.clone(),
            capabilities: hephaestus_core::authority::CapabilitySet::new(true, false),
        })
    }
}
