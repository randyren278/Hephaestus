use std::{
    fs,
    io::{Read, Write},
    net::Shutdown,
    os::unix::{fs::PermissionsExt, net::UnixStream},
    path::{Path, PathBuf},
    process::{Child, Command as ProcessCommand, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

use hephaestus_control::{
    API_VERSION, ApiErrorCode, ApiRequest, ApiResponse, Command, ControlError, ControlPlane,
    GenomeRecord, ResponseData, RunCompletionReason, WorldRecord,
};
use hephaestus_genome::{SourceFormat, compile_genome, compile_world};
use hephaestus_ledger::{ArtifactStore, EventInput, EventStore};
use tempfile::tempdir;

const DAEMON: &str = env!("CARGO_BIN_EXE_hephaestusd");
const CLI: &str = env!("CARGO_BIN_EXE_hephaestus");

struct Daemon {
    child: Child,
    data_dir: PathBuf,
}

impl Daemon {
    fn start(data_dir: &Path) -> Self {
        Self::start_with_repository(data_dir, Path::new(env!("CARGO_MANIFEST_DIR")))
    }

    fn start_with_repository(data_dir: &Path, source_repository: &Path) -> Self {
        let mut child = ProcessCommand::new(DAEMON)
            .arg("--data-dir")
            .arg(data_dir)
            .arg("--source-repository")
            .arg(source_repository)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("start daemon");
        let deadline = Instant::now() + Duration::from_secs(5);
        let socket = data_dir.join("control.sock");
        while UnixStream::connect(&socket).is_err() {
            if let Some(status) = child.try_wait().expect("inspect daemon") {
                panic!("daemon exited before serving: {status}");
            }
            assert!(Instant::now() < deadline, "daemon socket was not created");
            thread::sleep(Duration::from_millis(20));
        }
        Self {
            child,
            data_dir: data_dir.to_owned(),
        }
    }

    fn stop(mut self) {
        let output = cli(&self.data_dir, &["daemon", "stop"]);
        assert!(
            output.status.success(),
            "graceful daemon stop failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        self.child.wait().expect("wait for daemon");
    }

    fn crash(mut self) {
        self.child.kill().expect("stop daemon");
        self.child.wait().expect("wait for daemon");
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn reference_runtime_runs_through_real_daemon_and_replays_terminal_evidence() {
    let directory = tempdir().expect("temporary directory");
    let data_dir = directory.path().join("data");
    let repository = directory.path().join("source");
    fs::create_dir_all(&repository).expect("create source repository");
    git(&repository, &["init"]);
    git(&repository, &["config", "user.name", "Hephaestus Test"]);
    git(
        &repository,
        &["config", "user.email", "hephaestus@example.invalid"],
    );
    fs::write(repository.join("README.md"), b"reference inventory\n").expect("write fixture");
    fs::create_dir(repository.join("src")).expect("create source directory");
    fs::write(
        repository.join("src/lib.rs"),
        b"pub fn answer() -> u8 { 42 }\n",
    )
    .expect("write source fixture");
    git(&repository, &["add", "."]);
    git(&repository, &["commit", "-m", "fixture"]);
    let (world, genome) = seed_compiled_genome(&data_dir);

    let daemon = Daemon::start_with_repository(&data_dir, &repository);
    let frozen = cli(&data_dir, &["run", &genome.genome_id]);
    assert!(!frozen.status.success());
    assert_eq!(
        serde_json::from_slice::<ApiResponse>(&frozen.stdout)
            .expect("decode frozen response")
            .error
            .expect("frozen error")
            .code,
        ApiErrorCode::InvalidRequest
    );
    assert!(cli(&data_dir, &["unfreeze"]).status.success());
    let run = response(&cli(&data_dir, &["run", &genome.genome_id]));
    let (run_id, stdout_artifact_id, stderr_artifact_id, trace_artifact_ids, latency_millis) =
        match run.data.expect("run response") {
            ResponseData::Run {
                run_id,
                genome_id,
                world_id,
                completion_reason: RunCompletionReason::Success,
                latency_millis,
                actual_cost_microusd: 0,
                stdout_artifact_id,
                stderr_artifact_id,
                trace_artifact_ids,
            } => {
                assert_eq!(genome_id, genome.genome_id);
                assert_eq!(world_id, world.world_id);
                (
                    run_id,
                    stdout_artifact_id,
                    stderr_artifact_id,
                    trace_artifact_ids,
                    latency_millis,
                )
            }
            other => panic!("unexpected run response: {other:?}"),
        };
    assert!(run_id.starts_with("reference-"));
    assert!(latency_millis <= 10_000);
    assert_eq!(trace_artifact_ids.len(), 6);

    let artifacts = ArtifactStore::open(data_dir.join("blobs")).expect("open artifacts");
    let stdout = artifacts
        .get(&hephaestus_ledger::ArtifactId::parse(stdout_artifact_id.clone()).expect("stdout ID"))
        .expect("read stdout artifact");
    let inventory: serde_json::Value = serde_json::from_slice(&stdout).expect("decode inventory");
    assert_eq!(inventory["schema_version"], 1);
    assert_eq!(inventory["genome_id"], genome.genome_id);
    assert_eq!(inventory["world_id"], world.world_id);
    assert!(inventory["checkpoint"].is_null());
    let paths: Vec<_> = inventory["files"]
        .as_array()
        .expect("inventory files")
        .iter()
        .map(|file| file["path"].as_str().expect("file path"))
        .collect();
    assert_eq!(paths, ["README.md", "src/lib.rs"]);
    assert_eq!(
        artifacts
            .get(
                &hephaestus_ledger::ArtifactId::parse(stderr_artifact_id.clone())
                    .expect("stderr ID")
            )
            .expect("read stderr artifact"),
        b""
    );
    let operator_token = fs::read_to_string(data_dir.join("operator.token")).expect("read token");
    for id in &trace_artifact_ids {
        let bytes = artifacts
            .get(&hephaestus_ledger::ArtifactId::parse(id.clone()).expect("trace ID"))
            .expect("read trace artifact");
        assert!(!String::from_utf8_lossy(&bytes).contains(&operator_token));
    }
    let started_bytes = artifacts
        .get(
            &hephaestus_ledger::ArtifactId::parse(trace_artifact_ids[0].clone())
                .expect("started trace ID"),
        )
        .expect("read started trace");
    let started: serde_json::Value =
        serde_json::from_slice(&started_bytes).expect("decode started trace");
    assert_eq!(started["kind"], "lifecycle_started");
    assert_eq!(started["fields"]["workspace_write"], "false");
    assert_eq!(started["fields"]["network"], "false");
    assert!(matches!(
        response(&cli(&data_dir, &["status"])).data,
        Some(ResponseData::Status { active_runs: 0, .. })
    ));
    assert!(matches!(
        response(&cli(&data_dir, &["replay"])).data,
        Some(ResponseData::Replay { active_runs: 0, .. })
    ));
    assert_eq!(
        fs::read_dir(data_dir.join("sandboxes"))
            .expect("read sandbox root")
            .count(),
        0
    );
    daemon.stop();

    let ledger = EventStore::open(data_dir.join("events.sqlite3")).expect("reopen ledger");
    let history = ledger.replay_verified().expect("verify history");
    let run_events: Vec<_> = history
        .iter()
        .filter(|event| event.aggregate_id == format!("run:{run_id}"))
        .collect();
    assert_eq!(run_events.len(), 7);
    assert_eq!(
        run_events.first().expect("first trace").event_type,
        "trace.recorded"
    );
    assert_eq!(
        run_events.last().expect("result receipt").event_type,
        "run.result_recorded"
    );
    let terminal_artifact = artifacts
        .get(
            &hephaestus_ledger::ArtifactId::parse(
                trace_artifact_ids.last().expect("terminal trace").clone(),
            )
            .expect("terminal artifact ID"),
        )
        .expect("terminal artifact");
    let terminal: serde_json::Value =
        serde_json::from_slice(&terminal_artifact).expect("decode terminal artifact");
    assert_eq!(terminal["kind"], "lifecycle_completed");
    assert_eq!(terminal["fields"]["completion_reason"], "success");
    assert_eq!(terminal["fields"]["actual_cost_microusd"], "0");
    assert_eq!(
        terminal["fields"]["latency_millis"],
        latency_millis.to_string()
    );

    let restarted = Daemon::start_with_repository(&data_dir, &repository);
    assert!(matches!(
        response(&cli(&data_dir, &["status"])).data,
        Some(ResponseData::Status { active_runs: 0, .. })
    ));
    assert!(matches!(
        response(&cli(&data_dir, &["replay"])).data,
        Some(ResponseData::Replay { active_runs: 0, .. })
    ));
    restarted.stop();
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ignored = self.child.kill();
        let _ignored = self.child.wait();
    }
}

#[test]
fn operator_cli_controls_and_replays_real_daemon_state_across_restarts() {
    let directory = tempdir().expect("temporary directory");
    let data_dir = directory.path();
    let genome = seed_canonical_state(data_dir);
    let daemon = Daemon::start(data_dir);

    let initial = cli(data_dir, &["status"]);
    assert!(initial.status.success());
    assert!(matches!(
        response(&initial).data,
        Some(ResponseData::Status {
            frozen: true,
            active_runs: 1,
            genome_count: 1,
            ..
        })
    ));
    let unfreeze = cli(data_dir, &["unfreeze"]);
    assert!(matches!(
        response(&unfreeze).data,
        Some(ResponseData::Acknowledged { frozen: false, .. })
    ));

    let duplicate = ProcessCommand::new(DAEMON)
        .arg("--data-dir")
        .arg(data_dir)
        .output()
        .expect("run duplicate daemon");
    assert!(!duplicate.status.success());
    daemon.crash();

    let daemon = Daemon::start(data_dir);
    let restarted = cli(data_dir, &["status"]);
    assert!(matches!(
        response(&restarted).data,
        Some(ResponseData::Status {
            frozen: false,
            active_runs: 1,
            ..
        })
    ));

    let freeze = cli(data_dir, &["freeze"]);
    assert!(matches!(
        response(&freeze).data,
        Some(ResponseData::Acknowledged { frozen: true, .. })
    ));
    let shown = cli(data_dir, &["genome", "show", &genome.genome_id]);
    assert_eq!(
        response(&shown).data,
        Some(ResponseData::Genome {
            genome: genome.clone()
        })
    );
    let replayed = cli(data_dir, &["replay"]);
    assert!(matches!(
        response(&replayed).data,
        Some(ResponseData::Replay {
            frozen: true,
            active_runs: 1,
            ref projection_hash,
            ..
        }) if projection_hash.len() == 64
    ));
    let killed = cli(data_dir, &["kill", "--all"]);
    assert!(matches!(
        response(&killed).data,
        Some(ResponseData::Acknowledged {
            frozen: true,
            killed_runs: 1
        })
    ));
    daemon.stop();

    let final_daemon = Daemon::start(data_dir);
    let final_status = cli(data_dir, &["status"]);
    assert!(matches!(
        response(&final_status).data,
        Some(ResponseData::Status {
            frozen: true,
            active_runs: 0,
            ..
        })
    ));
    assert_private(data_dir, "operator.token", 0o600);
    assert_private(data_dir, "control.sock", 0o600);
    assert_private(data_dir, "events.sqlite3", 0o600);
    assert_private(data_dir, "daemon.lock", 0o600);
    assert_eq!(
        fs::metadata(data_dir)
            .expect("data directory metadata")
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(data_dir.join("blobs"))
            .expect("artifact directory metadata")
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    final_daemon.stop();
}

#[test]
fn local_api_fails_closed_for_bad_auth_versions_and_requests() {
    let directory = tempdir().expect("temporary directory");
    let daemon = Daemon::start(directory.path());
    let socket = directory.path().join("control.sock");

    let unauthorized = raw_request(
        &socket,
        &serde_json::to_vec(&ApiRequest {
            version: API_VERSION,
            request_id: "unauthorized".to_owned(),
            token: "0".repeat(64),
            command: Command::Unfreeze,
        })
        .expect("encode request"),
    );
    assert_eq!(
        unauthorized.error.expect("unauthorized error").code,
        ApiErrorCode::Unauthorized
    );

    let token = fs::read_to_string(directory.path().join("operator.token")).expect("read token");
    let unsupported = raw_request(
        &socket,
        &serde_json::to_vec(&ApiRequest {
            version: 2,
            request_id: "unsupported".to_owned(),
            token,
            command: Command::Status,
        })
        .expect("encode request"),
    );
    assert_eq!(
        unsupported.error.expect("version error").code,
        ApiErrorCode::UnsupportedVersion
    );

    let malformed = raw_request(&socket, br#"{"version":1,"unknown":true}"#);
    assert_eq!(
        malformed.error.expect("schema error").code,
        ApiErrorCode::InvalidRequest
    );
    let oversized = raw_request(&socket, &vec![b'x'; 65_537]);
    let oversized_error = oversized.error.expect("size error");
    assert_eq!(oversized_error.code, ApiErrorCode::InvalidRequest);
    assert_eq!(oversized_error.message, "request exceeds limit");

    let token = fs::read_to_string(directory.path().join("operator.token")).expect("read token");
    let invalid_identifier = raw_request(
        &socket,
        &serde_json::to_vec(&ApiRequest {
            version: API_VERSION,
            request_id: "empty-genome".to_owned(),
            token,
            command: Command::GenomeShow {
                genome_id: " ".to_owned(),
            },
        })
        .expect("encode request"),
    );
    assert_eq!(
        invalid_identifier.error.expect("identifier error").code,
        ApiErrorCode::InvalidRequest
    );

    let status = cli(directory.path(), &["status"]);
    assert!(matches!(
        response(&status).data,
        Some(ResponseData::Status { frozen: true, .. })
    ));
    daemon.stop();

    let ledger = EventStore::open(directory.path().join("events.sqlite3")).expect("reopen ledger");
    let history = ledger.replay_verified().expect("verify audited history");
    assert!(
        history
            .iter()
            .any(|event| event.event_type == "control.genome_show")
    );
}

#[test]
fn daemon_rejects_forged_operator_history_and_unverifiable_genomes() {
    let forged_directory = tempdir().expect("temporary directory");
    let mut forged = EventStore::open(forged_directory.path().join("events.sqlite3"))
        .expect("open forged ledger");
    forged
        .append(EventInput::new(
            "forged-unfreeze",
            "hephaestus-control",
            "control.unfreeze",
            "candidate",
            1,
            br#"{"request_id":"forged","command":{"command":"unfreeze"}}"#,
        ))
        .expect("append forged event");
    drop(forged);
    assert!(matches!(
        ControlPlane::open(forged_directory.path()),
        Err(ControlError::Projection(_))
    ));

    let missing_directory = tempdir().expect("temporary directory");
    let missing_hash = "2".repeat(64);
    let missing = GenomeRecord {
        genome_id: format!("hephaestus:genome:{missing_hash}"),
        name: "missing".to_owned(),
        world_id: format!("hephaestus:world:{}", "3".repeat(64)),
        artifact_id: missing_hash,
        parent_ids: Vec::new(),
    };
    let mut ledger = EventStore::open(missing_directory.path().join("events.sqlite3"))
        .expect("open missing-artifact ledger");
    ledger
        .append(EventInput::new(
            "missing-genome",
            &missing.genome_id,
            "genome.registered",
            "test-fixture",
            1,
            serde_json::to_vec(&missing).expect("encode missing Genome"),
        ))
        .expect("append missing Genome");
    drop(ledger);
    assert!(matches!(
        ControlPlane::open(missing_directory.path()),
        Err(ControlError::Ledger(_))
    ));

    let missing_world_directory = tempdir().expect("temporary directory");
    let world_hash = "4".repeat(64);
    let missing_world = WorldRecord {
        world_id: format!("hephaestus:world:{world_hash}"),
        name: "missing-world".to_owned(),
        artifact_id: world_hash,
    };
    let mut world_ledger = EventStore::open(missing_world_directory.path().join("events.sqlite3"))
        .expect("open missing World ledger");
    world_ledger
        .append(EventInput::new(
            "missing-world",
            &missing_world.world_id,
            "world.registered",
            "test-fixture",
            1,
            serde_json::to_vec(&missing_world).expect("encode missing World"),
        ))
        .expect("append missing World");
    drop(world_ledger);
    assert!(matches!(
        ControlPlane::open(missing_world_directory.path()),
        Err(ControlError::Ledger(_))
    ));

    let forged_world_directory = tempdir().expect("temporary directory");
    let forged_artifacts = ArtifactStore::open(forged_world_directory.path().join("blobs"))
        .expect("open forged World artifacts");
    let forged_artifact = forged_artifacts
        .put(br#"{"name":"not-a-World"}"#)
        .expect("store forged World");
    let forged_world = WorldRecord {
        world_id: format!("hephaestus:world:{}", forged_artifact.as_str()),
        name: "not-a-World".to_owned(),
        artifact_id: forged_artifact.as_str().to_owned(),
    };
    let mut forged_world_ledger =
        EventStore::open(forged_world_directory.path().join("events.sqlite3"))
            .expect("open forged World ledger");
    forged_world_ledger
        .append(EventInput::new(
            "forged-world",
            &forged_world.world_id,
            "world.registered",
            "test-fixture",
            1,
            serde_json::to_vec(&forged_world).expect("encode forged World"),
        ))
        .expect("append forged World");
    drop(forged_world_ledger);
    assert!(matches!(
        ControlPlane::open(forged_world_directory.path()),
        Err(ControlError::Projection(_))
    ));
}

#[test]
fn reference_run_rejects_a_genome_without_a_registered_world() {
    let directory = tempdir().expect("temporary directory");
    let genome = seed_canonical_state(directory.path());
    let daemon = Daemon::start(directory.path());
    assert!(cli(directory.path(), &["unfreeze"]).status.success());
    let output = cli(directory.path(), &["run", &genome.genome_id]);
    assert!(!output.status.success());
    let error = serde_json::from_slice::<ApiResponse>(&output.stdout)
        .expect("decode response")
        .error
        .expect("missing World error");
    assert_eq!(error.code, ApiErrorCode::InvalidRequest);
    assert!(matches!(
        response(&cli(directory.path(), &["status"])).data,
        Some(ResponseData::Status { active_runs: 1, .. })
    ));
    daemon.stop();
}

fn seed_canonical_state(data_dir: &Path) -> GenomeRecord {
    let artifact_store = ArtifactStore::open(data_dir.join("blobs")).expect("open artifact store");
    let canonical = br#"{"name":"seed"}"#;
    let artifact = artifact_store
        .put(canonical)
        .expect("store canonical Genome");
    let genome = GenomeRecord {
        genome_id: format!("hephaestus:genome:{}", artifact.as_str()),
        name: "seed".to_owned(),
        world_id: format!("hephaestus:world:{}", "1".repeat(64)),
        artifact_id: artifact.as_str().to_owned(),
        parent_ids: Vec::new(),
    };
    let mut ledger = EventStore::open(data_dir.join("events.sqlite3")).expect("open event ledger");
    ledger
        .append(EventInput::new(
            "seed-genome",
            &genome.genome_id,
            "genome.registered",
            "test-fixture",
            1,
            serde_json::to_vec(&genome).expect("encode Genome"),
        ))
        .expect("append Genome");
    ledger
        .append(EventInput::new(
            "seed-run",
            "run-1",
            "run.started",
            "test-fixture",
            2,
            br#"{"run_id":"run-1"}"#,
        ))
        .expect("append active run");
    genome
}

fn seed_compiled_genome(data_dir: &Path) -> (WorldRecord, GenomeRecord) {
    fs::create_dir_all(data_dir).expect("create data directory");
    let artifacts = ArtifactStore::open(data_dir.join("blobs")).expect("open artifact store");
    let world_source = r#"{
        "schema_version": 1,
        "name": "reference-world",
        "laws": {
            "candidate_network": false,
            "candidate_evaluator_access": false,
            "maximum_cost_microusd": 0
        },
        "authority_ceiling": { "workspace_write": false, "network": false },
        "mutation_scope": [],
        "promotion": {
            "minimum_delta_bps": 0,
            "maximum_regressions": 0,
            "confidence_bps": 9500
        },
        "objectives": ["inventory"],
        "evaluator_artifacts": {}
    }"#;
    let compiled_world =
        compile_world(world_source, SourceFormat::Json, &artifacts).expect("compile World");
    let world_artifact = artifacts
        .put(compiled_world.canonical_json())
        .expect("store World");
    let world = WorldRecord {
        world_id: compiled_world.id().to_owned(),
        name: compiled_world.name().to_owned(),
        artifact_id: world_artifact.as_str().to_owned(),
    };
    let genome_source = r#"{
        "schema_version": 1,
        "name": "reference-genome",
        "parents": [],
        "model": { "provider": "deterministic", "family": "reference" },
        "authority": { "workspace_write": false, "network": false },
        "artifacts": {}
    }"#;
    let compiled_genome = compile_genome(
        genome_source,
        SourceFormat::Json,
        &compiled_world,
        &std::collections::BTreeMap::new(),
        &artifacts,
    )
    .expect("compile Genome");
    let genome_artifact = artifacts
        .put(compiled_genome.canonical_json())
        .expect("store Genome");
    let genome = GenomeRecord {
        genome_id: compiled_genome.id().to_owned(),
        name: compiled_genome.name().to_owned(),
        world_id: world.world_id.clone(),
        artifact_id: genome_artifact.as_str().to_owned(),
        parent_ids: compiled_genome.parents().to_vec(),
    };
    let mut ledger = EventStore::open(data_dir.join("events.sqlite3")).expect("open event ledger");
    ledger
        .append(EventInput::new(
            "reference-world",
            &world.world_id,
            "world.registered",
            "test-fixture",
            1,
            serde_json::to_vec(&world).expect("encode World"),
        ))
        .expect("register World");
    ledger
        .append(EventInput::new(
            "reference-genome",
            &genome.genome_id,
            "genome.registered",
            "test-fixture",
            2,
            serde_json::to_vec(&genome).expect("encode Genome"),
        ))
        .expect("register Genome");
    (world, genome)
}

fn git(repository: &Path, arguments: &[&str]) {
    let output = ProcessCommand::new("git")
        .arg("-C")
        .arg(repository)
        .args(arguments)
        .output()
        .expect("run git fixture command");
    assert!(
        output.status.success(),
        "git fixture failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn cli(data_dir: &Path, arguments: &[&str]) -> Output {
    let mut command = ProcessCommand::new(CLI);
    command
        .arg("--data-dir")
        .arg(data_dir)
        .arg("--json")
        .args(arguments)
        .output()
        .expect("run CLI")
}

fn response(output: &Output) -> ApiResponse {
    assert!(
        output.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("decode CLI response")
}

fn raw_request(socket: &Path, bytes: &[u8]) -> ApiResponse {
    let mut stream = UnixStream::connect(socket).expect("connect control socket");
    stream.write_all(bytes).expect("write request");
    stream.shutdown(Shutdown::Write).expect("finish request");
    let mut response = Vec::new();
    stream.read_to_end(&mut response).expect("read response");
    serde_json::from_slice(&response).expect("decode response")
}

fn assert_private(data_dir: &Path, name: &str, expected: u32) {
    let path = PathBuf::from(data_dir).join(name);
    assert_eq!(
        fs::metadata(path)
            .expect("private path metadata")
            .permissions()
            .mode()
            & 0o777,
        expected
    );
}
