use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read, Write},
    os::unix::process::CommandExt as _,
    path::PathBuf,
    process::{Child, Command, ExitStatus, Stdio},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use hephaestus_core::authority::CapabilitySet;

use crate::{
    AdapterCapabilities, CapabilityToken, Provider, ProviderInvocation, RunHandle, RunSnapshot,
    RunSpec, RunStatus, RuntimeAdapter, RuntimeError, Sandbox,
};

/// Provider-neutral child-process supervisor used for non-billable local helpers.
///
/// Hosted providers will use the same monitor after credential and hard-cost
/// mediation are available; this constructor deliberately grants no network.
pub struct SupervisedRuntime {
    executable: PathBuf,
    arguments: Vec<String>,
    runs: BTreeMap<String, SupervisedRun>,
}

struct SupervisedRun {
    shared: Arc<SharedRun>,
    stdout_path: PathBuf,
    stderr_path: PathBuf,
    capabilities: CapabilitySet,
}

struct SharedRun {
    observed: Mutex<ObservedRun>,
    changed: Condvar,
    interrupt: AtomicBool,
    output_exceeded: AtomicBool,
    io_failed: AtomicBool,
}

#[derive(Clone, Copy)]
struct ObservedRun {
    status: RunStatus,
    exit_code: Option<i32>,
}

#[derive(Clone, Copy)]
enum StopReason {
    Interrupted,
    TimedOut,
    OutputExceeded,
    IoFailed,
}

impl SupervisedRuntime {
    /// Creates an offline, non-shell process adapter for deterministic helpers.
    ///
    /// # Errors
    ///
    /// Rejects an empty executable path.
    pub fn deterministic(
        executable: impl Into<PathBuf>,
        arguments: impl IntoIterator<Item = String>,
    ) -> Result<Self, RuntimeError> {
        let executable = executable.into();
        ProviderInvocation::deterministic(&executable, [], [])?;
        Ok(Self {
            executable,
            arguments: arguments.into_iter().collect(),
            runs: BTreeMap::new(),
        })
    }

    fn launch(
        &mut self,
        spec: &RunSpec,
        sandbox: &Sandbox,
        token: &CapabilityToken,
    ) -> Result<RunHandle, RuntimeError> {
        sandbox.authorize(token, spec.capabilities())?;
        self.report_capabilities()
            .authority
            .derive_child(spec.capabilities())
            .map_err(|_| RuntimeError::CapabilityDenied)?;
        if self.runs.contains_key(spec.run_id()) {
            return Err(RuntimeError::InvalidSpec("run already exists"));
        }
        let deadline = Instant::now()
            .checked_add(spec.budget().wall())
            .ok_or(RuntimeError::InvalidSpec("wall budget exceeds clock range"))?;

        fs::create_dir_all(sandbox.execution_dir())?;
        let stdout_path = sandbox.execution_dir().join("stdout.log");
        let stderr_path = sandbox.execution_dir().join("stderr.log");
        let stdout_file = File::create(&stdout_path)?;
        let stderr_file = File::create(&stderr_path)?;
        let invocation = ProviderInvocation::deterministic(
            &self.executable,
            self.arguments.clone(),
            spec.prompt(),
        )?;
        let mut command = Command::new(invocation.program());
        command.args(invocation.arguments());
        command.current_dir(sandbox.worktree());
        command.env_clear();
        if let Some(path) = std::env::var_os("PATH") {
            command.env("PATH", path);
        }
        command.env("HOME", sandbox.execution_dir());
        command.env("TMPDIR", sandbox.execution_dir());
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        let mut child = command.spawn()?;
        let child_stdin = child
            .stdin
            .take()
            .ok_or(RuntimeError::InvalidSpec("child stdin was not piped"))?;
        let child_stdout = child
            .stdout
            .take()
            .ok_or(RuntimeError::InvalidSpec("child stdout was not piped"))?;
        let child_stderr = child
            .stderr
            .take()
            .ok_or(RuntimeError::InvalidSpec("child stderr was not piped"))?;

        let shared = Arc::new(SharedRun {
            observed: Mutex::new(ObservedRun {
                status: RunStatus::Running,
                exit_code: None,
            }),
            changed: Condvar::new(),
            interrupt: AtomicBool::new(false),
            output_exceeded: AtomicBool::new(false),
            io_failed: AtomicBool::new(false),
        });
        spawn_stdin_writer(child_stdin, invocation.stdin().to_vec());
        spawn_monitor(
            child,
            child_stdout,
            child_stderr,
            stdout_file,
            stderr_file,
            spec.budget().maximum_output_bytes(),
            deadline,
            Arc::clone(&shared),
        );
        self.runs.insert(
            spec.run_id().to_owned(),
            SupervisedRun {
                shared,
                stdout_path,
                stderr_path,
                capabilities: spec.capabilities(),
            },
        );
        Ok(RunHandle {
            run_id: spec.run_id().to_owned(),
            provider: Provider::Deterministic,
        })
    }
}

impl RuntimeAdapter for SupervisedRuntime {
    fn provider(&self) -> Provider {
        Provider::Deterministic
    }

    fn report_capabilities(&self) -> AdapterCapabilities {
        AdapterCapabilities {
            resume: false,
            interrupt: true,
            snapshot: true,
            authority: CapabilitySet::new(true, false),
        }
    }

    fn start(
        &mut self,
        spec: &RunSpec,
        sandbox: &Sandbox,
        token: &CapabilityToken,
    ) -> Result<RunHandle, RuntimeError> {
        self.launch(spec, sandbox, token)
    }

    fn resume(
        &mut self,
        _spec: &RunSpec,
        _sandbox: &Sandbox,
        _token: &CapabilityToken,
        _checkpoint: &str,
    ) -> Result<RunHandle, RuntimeError> {
        Err(RuntimeError::Unsupported(
            "supervised helper does not support resume",
        ))
    }

    fn interrupt(&mut self, run_id: &str) -> Result<(), RuntimeError> {
        let run = self
            .runs
            .get(run_id)
            .ok_or(RuntimeError::InvalidSpec("run does not exist"))?;
        run.shared.interrupt.store(true, Ordering::Release);
        let mut observed = run.shared.observed.lock().expect("run state lock poisoned");
        while observed.status == RunStatus::Running {
            observed = run
                .shared
                .changed
                .wait(observed)
                .expect("run state lock poisoned");
        }
        Ok(())
    }

    fn snapshot(&mut self, run_id: &str) -> Result<RunSnapshot, RuntimeError> {
        let run = self
            .runs
            .get(run_id)
            .ok_or(RuntimeError::InvalidSpec("run does not exist"))?;
        let observed = *run.shared.observed.lock().expect("run state lock poisoned");
        Ok(RunSnapshot {
            run_id: run_id.to_owned(),
            status: observed.status,
            exit_code: observed.exit_code,
            stdout_path: run.stdout_path.clone(),
            stderr_path: run.stderr_path.clone(),
            capabilities: run.capabilities,
        })
    }
}

fn spawn_stdin_writer(mut stdin: impl Write + Send + 'static, bytes: Vec<u8>) {
    thread::spawn(move || {
        let _ignored = stdin.write_all(&bytes);
    });
}

#[allow(clippy::too_many_arguments)]
fn spawn_monitor(
    mut child: Child,
    stdout: impl Read + Send + 'static,
    stderr: impl Read + Send + 'static,
    stdout_file: File,
    stderr_file: File,
    maximum_output_bytes: usize,
    deadline: Instant,
    shared: Arc<SharedRun>,
) {
    thread::spawn(move || {
        let used = Arc::new(AtomicUsize::new(0));
        let stdout_reader = spawn_output_reader(
            stdout,
            stdout_file,
            Arc::clone(&used),
            maximum_output_bytes,
            Arc::clone(&shared),
        );
        let stderr_reader = spawn_output_reader(
            stderr,
            stderr_file,
            used,
            maximum_output_bytes,
            Arc::clone(&shared),
        );

        let outcome = loop {
            if shared.interrupt.load(Ordering::Acquire) {
                break terminate(&mut child, StopReason::Interrupted);
            }
            if shared.output_exceeded.load(Ordering::Acquire) {
                break terminate(&mut child, StopReason::OutputExceeded);
            }
            if shared.io_failed.load(Ordering::Acquire) {
                break terminate(&mut child, StopReason::IoFailed);
            }
            if Instant::now() >= deadline {
                break terminate(&mut child, StopReason::TimedOut);
            }
            match child.try_wait() {
                Ok(Some(status)) => break (Some(status), None),
                Ok(None) => thread::sleep(Duration::from_millis(5)),
                Err(_) => break terminate(&mut child, StopReason::IoFailed),
            }
        };

        if stdout_reader.join().is_err() || stderr_reader.join().is_err() {
            shared.io_failed.store(true, Ordering::Release);
        }
        let reason = outcome.1.or_else(|| {
            if shared.output_exceeded.load(Ordering::Acquire) {
                Some(StopReason::OutputExceeded)
            } else if shared.io_failed.load(Ordering::Acquire) {
                Some(StopReason::IoFailed)
            } else {
                None
            }
        });
        let observed = ObservedRun {
            status: reason.map_or_else(
                || outcome.0.map_or(RunStatus::Failed, status_from_exit),
                |reason| match reason {
                    StopReason::Interrupted => RunStatus::Interrupted,
                    StopReason::TimedOut => RunStatus::TimedOut,
                    StopReason::OutputExceeded | StopReason::IoFailed => RunStatus::Failed,
                },
            ),
            exit_code: outcome.0.and_then(|status| status.code()),
        };
        *shared.observed.lock().expect("run state lock poisoned") = observed;
        shared.changed.notify_all();
    });
}

fn spawn_output_reader(
    mut source: impl Read + Send + 'static,
    mut destination: impl Write + Send + 'static,
    used: Arc<AtomicUsize>,
    maximum: usize,
    shared: Arc<SharedRun>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut buffer = [0_u8; 8 * 1024];
        loop {
            let count = match source.read(&mut buffer) {
                Ok(0) => return,
                Ok(count) => count,
                Err(_) => {
                    shared.io_failed.store(true, Ordering::Release);
                    return;
                }
            };
            let previous = used.fetch_add(count, Ordering::AcqRel);
            let allowed = maximum.saturating_sub(previous).min(count);
            if allowed != 0 && destination.write_all(&buffer[..allowed]).is_err() {
                shared.io_failed.store(true, Ordering::Release);
                return;
            }
            if allowed != count {
                shared.output_exceeded.store(true, Ordering::Release);
            }
        }
    })
}

fn terminate(child: &mut Child, reason: StopReason) -> (Option<ExitStatus>, Option<StopReason>) {
    let _ignored = process_group_kill(child.id()).status();
    let _ignored = child.kill();
    let status = child.wait().ok();
    (status, Some(reason))
}

fn process_group_kill(pid: u32) -> Command {
    let process_group = format!("-{pid}");
    let mut command = Command::new("/bin/kill");
    command.args(["-KILL", "--", &process_group]);
    command
}

fn status_from_exit(status: ExitStatus) -> RunStatus {
    if status.success() {
        RunStatus::Succeeded
    } else {
        RunStatus::Failed
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::*;

    struct FailingReader;

    impl Read for FailingReader {
        fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("injected read failure"))
        }
    }

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("injected write failure"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn output_capture_surfaces_reader_and_writer_failures() {
        let read_failure = shared_run();
        spawn_output_reader(
            FailingReader,
            Vec::new(),
            Arc::new(AtomicUsize::new(0)),
            10,
            Arc::clone(&read_failure),
        )
        .join()
        .expect("join failing reader");
        assert!(read_failure.io_failed.load(Ordering::Acquire));

        let write_failure = shared_run();
        spawn_output_reader(
            io::Cursor::new(b"captured output"),
            FailingWriter,
            Arc::new(AtomicUsize::new(0)),
            100,
            Arc::clone(&write_failure),
        )
        .join()
        .expect("join failing writer");
        assert!(write_failure.io_failed.load(Ordering::Acquire));

        let command = process_group_kill(123);
        let arguments: Vec<_> = command
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect();
        assert_eq!(arguments, ["-KILL", "--", "-123"]);
    }

    fn shared_run() -> Arc<SharedRun> {
        Arc::new(SharedRun {
            observed: Mutex::new(ObservedRun {
                status: RunStatus::Running,
                exit_code: None,
            }),
            changed: Condvar::new(),
            interrupt: AtomicBool::new(false),
            output_exceeded: AtomicBool::new(false),
            io_failed: AtomicBool::new(false),
        })
    }
}
