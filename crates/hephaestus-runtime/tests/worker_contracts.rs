use std::time::Duration;

use hephaestus_runtime::{
    CompletionReason, IsolatedWorker, IsolationPolicy, RuntimeError, WorkerDomain, WorkerLimits,
};
use tempfile::tempdir;

#[test]
#[cfg(feature = "test-support")]
fn isolated_workers_stream_bounded_input_in_distinct_domains() {
    let root = tempdir().expect("worker root");
    let limits = WorkerLimits::new(Duration::from_secs(2), 1_000, 1_000).expect("limits");
    let candidate = IsolatedWorker::open(
        root.path(),
        IsolationPolicy::unconfined_for_testing(),
        WorkerDomain::Candidate,
        "/bin/cat",
        [],
        limits,
    )
    .expect("candidate worker");
    let evaluator = IsolatedWorker::open(
        root.path(),
        IsolationPolicy::unconfined_for_testing(),
        WorkerDomain::Evaluator,
        "/bin/cat",
        [],
        limits,
    )
    .expect("evaluator worker");

    let candidate_output = candidate.execute("paired", b"candidate request").unwrap();
    let evaluator_output = evaluator.execute("paired", b"evaluator request").unwrap();

    assert_eq!(candidate_output.domain, WorkerDomain::Candidate);
    assert_eq!(candidate_output.stdout, b"candidate request");
    assert_eq!(
        candidate_output.completion_reason,
        CompletionReason::Success
    );
    assert_eq!(evaluator_output.domain, WorkerDomain::Evaluator);
    assert_eq!(evaluator_output.stdout, b"evaluator request");
    assert_eq!(
        evaluator_output.completion_reason,
        CompletionReason::Success
    );
    assert!(root.path().read_dir().unwrap().next().is_none());
}

#[test]
#[cfg(feature = "test-support")]
fn worker_domains_receive_distinct_private_environments() {
    let root = tempdir().expect("worker root");
    let limits = WorkerLimits::new(Duration::from_secs(2), 1_000, 10_000).unwrap();
    let worker = |domain| {
        IsolatedWorker::open(
            root.path(),
            IsolationPolicy::unconfined_for_testing(),
            domain,
            "/usr/bin/env",
            [],
            limits,
        )
        .unwrap()
    };

    let candidate = worker(WorkerDomain::Candidate)
        .execute("same-id", b"")
        .unwrap();
    let evaluator = worker(WorkerDomain::Evaluator)
        .execute("same-id", b"")
        .unwrap();
    let candidate_env = String::from_utf8(candidate.stdout).unwrap();
    let evaluator_env = String::from_utf8(evaluator.stdout).unwrap();

    assert!(candidate_env.contains("candidate-same-id"));
    assert!(evaluator_env.contains("evaluator-same-id"));
    assert_ne!(candidate_env, evaluator_env);
    assert!(root.path().read_dir().unwrap().next().is_none());
}

#[test]
#[cfg(target_os = "macos")]
fn worker_policy_denies_protected_paths() {
    let root = tempdir().expect("worker root");
    let protected = tempdir().expect("protected root");
    let secret = protected.path().join("sealed-expected-output");
    std::fs::write(&secret, b"must not cross worker boundary").unwrap();
    let worker = IsolatedWorker::open(
        root.path(),
        IsolationPolicy::detect([protected.path().to_owned()]),
        WorkerDomain::Candidate,
        "/bin/cat",
        [secret.to_string_lossy().into_owned()],
        WorkerLimits::new(Duration::from_secs(2), 1_000, 1_000).unwrap(),
    )
    .unwrap();

    let output = worker.execute("leak-probe", b"").unwrap();

    assert_eq!(output.completion_reason, CompletionReason::ProviderFailure);
    assert!(!output.stdout.windows(4).any(|window| window == b"must"));
    assert!(root.path().read_dir().unwrap().next().is_none());
}

#[test]
#[cfg(feature = "test-support")]
fn timeout_output_overrun_and_crash_all_cleanup_worker_roots() {
    let root = tempdir().expect("worker root");
    let timeout = IsolatedWorker::open(
        root.path(),
        IsolationPolicy::unconfined_for_testing(),
        WorkerDomain::Evaluator,
        "/bin/sleep",
        ["2".to_owned()],
        WorkerLimits::new(Duration::from_millis(20), 1_000, 1_000).unwrap(),
    )
    .unwrap()
    .execute("timeout", b"")
    .unwrap();
    assert_eq!(
        timeout.completion_reason,
        CompletionReason::WallBudgetExceeded
    );
    assert!(root.path().read_dir().unwrap().next().is_none());

    let output = IsolatedWorker::open(
        root.path(),
        IsolationPolicy::unconfined_for_testing(),
        WorkerDomain::Evaluator,
        "/usr/bin/yes",
        [],
        WorkerLimits::new(Duration::from_secs(2), 1_000, 17).unwrap(),
    )
    .unwrap()
    .execute("output", b"")
    .unwrap();
    assert_eq!(
        output.completion_reason,
        CompletionReason::OutputBudgetExceeded
    );
    assert!(output.stdout.len() + output.stderr.len() <= 17);
    assert!(root.path().read_dir().unwrap().next().is_none());

    let crash = IsolatedWorker::open(
        root.path(),
        IsolationPolicy::unconfined_for_testing(),
        WorkerDomain::Evaluator,
        "/usr/bin/false",
        [],
        WorkerLimits::new(Duration::from_secs(2), 1_000, 1_000).unwrap(),
    )
    .unwrap()
    .execute("crash", b"")
    .unwrap();
    assert_eq!(crash.completion_reason, CompletionReason::ProviderFailure);
    assert!(root.path().read_dir().unwrap().next().is_none());
}

#[test]
fn malformed_or_oversized_requests_fail_before_worker_root_creation() {
    assert!(WorkerLimits::new(Duration::ZERO, 1, 1).is_err());
    assert!(WorkerLimits::new(Duration::from_secs(1), 0, 1).is_err());
    assert!(WorkerLimits::new(Duration::from_secs(1), 1, 0).is_err());
    let root = tempdir().expect("worker root");
    let worker = IsolatedWorker::open(
        root.path(),
        contract_isolation(),
        WorkerDomain::Candidate,
        "/bin/cat",
        [],
        WorkerLimits::new(Duration::from_secs(1), 3, 10).unwrap(),
    )
    .unwrap();

    assert!(worker.execute("../escape", b"").is_err());
    assert!(matches!(
        worker.execute("valid", b"four"),
        Err(RuntimeError::InvalidSpec(
            "worker input exceeds its byte limit"
        ))
    ));
    assert!(root.path().read_dir().unwrap().next().is_none());
}

#[test]
fn replaced_worker_root_is_rejected_before_launch() {
    let outer = tempdir().expect("outer root");
    let root = outer.path().join("workers");
    let moved = outer.path().join("original-workers");
    let worker = IsolatedWorker::open(
        &root,
        contract_isolation(),
        WorkerDomain::Evaluator,
        "/bin/cat",
        [],
        WorkerLimits::new(Duration::from_secs(1), 10, 10).unwrap(),
    )
    .unwrap();
    std::fs::rename(&root, &moved).unwrap();
    std::fs::create_dir(&root).unwrap();

    assert!(matches!(
        worker.execute("identity-probe", b""),
        Err(RuntimeError::InvalidSpec("worker root identity changed"))
    ));
    assert!(root.read_dir().unwrap().next().is_none());
    assert!(moved.read_dir().unwrap().next().is_none());
}

#[test]
#[cfg(feature = "test-support")]
fn spawn_failure_cleans_private_worker_root() {
    let root = tempdir().expect("worker root");
    let worker = IsolatedWorker::open(
        root.path(),
        IsolationPolicy::unconfined_for_testing(),
        WorkerDomain::Evaluator,
        root.path().join("missing-executable"),
        [],
        WorkerLimits::new(Duration::from_secs(1), 10, 10).unwrap(),
    )
    .unwrap();

    assert!(worker.execute("spawn-failure", b"").is_err());
    assert!(root.path().read_dir().unwrap().next().is_none());
}

#[test]
#[cfg(not(target_os = "macos"))]
fn workers_fail_closed_without_a_verified_backend_and_cleanup() {
    let root = tempdir().expect("worker root");
    let marker = root.path().join("process-started");
    let worker = IsolatedWorker::open(
        root.path(),
        IsolationPolicy::detect([]),
        WorkerDomain::Candidate,
        "/usr/bin/touch",
        [marker.to_string_lossy().into_owned()],
        WorkerLimits::new(Duration::from_secs(1), 1, 1).unwrap(),
    )
    .unwrap();

    assert!(worker.execute("unavailable", b"").is_err());
    assert!(!marker.exists());
    assert!(root.path().read_dir().unwrap().next().is_none());
}

#[cfg(feature = "test-support")]
fn contract_isolation() -> IsolationPolicy {
    IsolationPolicy::unconfined_for_testing()
}

#[cfg(not(feature = "test-support"))]
fn contract_isolation() -> IsolationPolicy {
    IsolationPolicy::detect([])
}
