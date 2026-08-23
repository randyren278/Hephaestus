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
    GenomeRecord, ResponseData,
};
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
        let mut child = ProcessCommand::new(DAEMON)
            .arg("--data-dir")
            .arg(data_dir)
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
