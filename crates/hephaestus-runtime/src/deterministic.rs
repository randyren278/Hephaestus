use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use hephaestus_core::authority::CapabilitySet;
use serde::Serialize;

use crate::{
    AdapterCapabilities, CapabilityToken, Provider, RunHandle, RunSnapshot, RunSpec, RunStatus,
    RuntimeAdapter, RuntimeError, Sandbox,
};

/// Offline reference runtime that inventories the isolated worktree deterministically.
#[derive(Default)]
pub struct DeterministicRuntime {
    runs: BTreeMap<String, ReferenceRun>,
}

struct ReferenceRun {
    worktree: PathBuf,
    stdout_path: PathBuf,
    stderr_path: PathBuf,
    capabilities: CapabilitySet,
    maximum_output_bytes: usize,
    status: RunStatus,
    checkpoint: Option<String>,
    genome_id: String,
    world_id: String,
    prompt_hash: String,
}

impl RuntimeAdapter for DeterministicRuntime {
    fn provider(&self) -> Provider {
        Provider::Deterministic
    }

    fn report_capabilities(&self) -> AdapterCapabilities {
        AdapterCapabilities {
            resume: true,
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
        sandbox.authorize(token, spec.capabilities())?;
        self.report_capabilities()
            .authority
            .derive_child(spec.capabilities())
            .map_err(|_| RuntimeError::CapabilityDenied)?;
        if self.runs.contains_key(spec.run_id()) {
            return Err(RuntimeError::InvalidSpec("run already exists"));
        }
        self.runs.insert(
            spec.run_id().to_owned(),
            ReferenceRun {
                worktree: sandbox.worktree().to_owned(),
                stdout_path: sandbox.execution_dir().join("stdout.json"),
                stderr_path: sandbox.execution_dir().join("stderr.txt"),
                capabilities: spec.capabilities(),
                maximum_output_bytes: spec.budget().maximum_output_bytes(),
                status: RunStatus::Running,
                checkpoint: None,
                genome_id: spec.genome_id().to_owned(),
                world_id: spec.world_id().to_owned(),
                prompt_hash: blake3::hash(spec.prompt().as_bytes()).to_hex().to_string(),
            },
        );
        Ok(RunHandle {
            run_id: spec.run_id().to_owned(),
            provider: Provider::Deterministic,
        })
    }

    fn resume(
        &mut self,
        spec: &RunSpec,
        sandbox: &Sandbox,
        token: &CapabilityToken,
        checkpoint: &str,
    ) -> Result<RunHandle, RuntimeError> {
        if checkpoint.trim().is_empty() {
            return Err(RuntimeError::InvalidSpec("checkpoint is required"));
        }
        self.runs.remove(spec.run_id());
        let handle = self.start(spec, sandbox, token)?;
        self.runs
            .get_mut(spec.run_id())
            .expect("run was inserted by start")
            .checkpoint = Some(checkpoint.to_owned());
        Ok(handle)
    }

    fn interrupt(&mut self, run_id: &str) -> Result<(), RuntimeError> {
        let run = self
            .runs
            .get_mut(run_id)
            .ok_or(RuntimeError::InvalidSpec("run does not exist"))?;
        run.status = RunStatus::Interrupted;
        fs::write(&run.stderr_path, b"interrupted by supervisor\n")?;
        Ok(())
    }

    fn snapshot(&mut self, run_id: &str) -> Result<RunSnapshot, RuntimeError> {
        let run = self
            .runs
            .get_mut(run_id)
            .ok_or(RuntimeError::InvalidSpec("run does not exist"))?;
        if run.status == RunStatus::Running {
            let output = inventory(run)?;
            if output.len() > run.maximum_output_bytes {
                run.status = RunStatus::Failed;
                fs::write(&run.stderr_path, b"output budget exceeded\n")?;
            } else {
                fs::write(&run.stdout_path, output)?;
                fs::write(&run.stderr_path, [])?;
                run.status = RunStatus::Succeeded;
            }
        }
        Ok(RunSnapshot {
            run_id: run_id.to_owned(),
            status: run.status,
            exit_code: match run.status {
                RunStatus::Succeeded => Some(0),
                RunStatus::Failed => Some(1),
                RunStatus::Running | RunStatus::Interrupted | RunStatus::TimedOut => None,
            },
            stdout_path: run.stdout_path.clone(),
            stderr_path: run.stderr_path.clone(),
            capabilities: run.capabilities,
        })
    }
}

#[derive(Serialize)]
struct Inventory<'a> {
    schema_version: u16,
    genome_id: &'a str,
    world_id: &'a str,
    prompt_hash: &'a str,
    checkpoint: Option<&'a str>,
    files: Vec<FileRecord>,
}

#[derive(Serialize)]
struct FileRecord {
    path: String,
    bytes: u64,
    blake3: String,
}

fn inventory(run: &ReferenceRun) -> Result<Vec<u8>, RuntimeError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(&run.worktree)
        .args(["ls-files", "-z"])
        .output()?;
    if !output.status.success() {
        return Err(RuntimeError::Git(
            "tracked-file inventory failed".to_owned(),
        ));
    }
    let mut files = Vec::new();
    for raw in output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|raw| !raw.is_empty())
    {
        let relative = std::str::from_utf8(raw)
            .map_err(|_| RuntimeError::InvalidSpec("repository path is not UTF-8"))?;
        let path = safe_tracked_path(&run.worktree, relative)?;
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            continue;
        }
        let bytes = fs::read(path)?;
        files.push(FileRecord {
            path: relative.to_owned(),
            bytes: metadata.len(),
            blake3: blake3::hash(&bytes).to_hex().to_string(),
        });
    }
    serde_json::to_vec(&Inventory {
        schema_version: 1,
        genome_id: &run.genome_id,
        world_id: &run.world_id,
        prompt_hash: &run.prompt_hash,
        checkpoint: run.checkpoint.as_deref(),
        files,
    })
    .map_err(|_| RuntimeError::InvalidSpec("inventory serialization failed"))
}

fn safe_tracked_path(root: &Path, relative: &str) -> Result<PathBuf, RuntimeError> {
    let relative = Path::new(relative);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(RuntimeError::InvalidSpec("tracked path escapes worktree"));
    }
    Ok(root.join(relative))
}
