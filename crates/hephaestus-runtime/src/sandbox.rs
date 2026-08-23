use std::{
    fs::{self, File},
    io::Read,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

use hephaestus_core::authority::CapabilitySet;

use crate::{RunSpec, RuntimeError};

/// Opaque, non-cloneable proof binding authority to one run and expiry.
pub struct CapabilityToken {
    secret: [u8; 32],
    run_id: String,
    expires_at: Instant,
}

/// Isolated execution layout and the maximum authority available inside it.
pub struct Sandbox {
    run_id: String,
    run_root: PathBuf,
    worktree: PathBuf,
    execution_dir: PathBuf,
    source_repository: PathBuf,
    capabilities: CapabilitySet,
    secret: [u8; 32],
    expires_at: Instant,
}

impl Sandbox {
    /// Isolated Git worktree visible to the worker.
    #[must_use]
    pub fn worktree(&self) -> &Path {
        &self.worktree
    }

    /// Private directory for provider output and checkpoints.
    #[must_use]
    pub fn execution_dir(&self) -> &Path {
        &self.execution_dir
    }

    /// Maximum authority granted to the worker.
    #[must_use]
    pub const fn capabilities(&self) -> CapabilitySet {
        self.capabilities
    }

    /// Verifies token binding, expiry, and requested authority.
    ///
    /// # Errors
    ///
    /// Fails for a different run, an expired or forged token, or capability widening.
    pub fn authorize(
        &self,
        token: &CapabilityToken,
        requested: CapabilitySet,
    ) -> Result<(), RuntimeError> {
        if token.run_id != self.run_id
            || token.expires_at != self.expires_at
            || Instant::now() >= self.expires_at
            || !constant_time_equal(&token.secret, &self.secret)
            || self.capabilities.derive_child(requested).is_err()
        {
            return Err(RuntimeError::CapabilityDenied);
        }
        Ok(())
    }

    /// Removes the Git worktree and private run directory.
    ///
    /// # Errors
    ///
    /// Returns a Git or filesystem error when cleanup cannot complete.
    pub fn cleanup(self) -> Result<(), RuntimeError> {
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.source_repository)
            .args(["worktree", "remove", "--force"])
            .arg(&self.worktree)
            .output()?;
        if !output.status.success() {
            return Err(RuntimeError::Git(redacted_git_failure(&output.stderr)));
        }
        if self.run_root.exists() {
            fs::remove_dir_all(&self.run_root)?;
        }
        Ok(())
    }
}

/// Creates sibling-separated private Git worktrees under one sandbox root.
pub struct SandboxManager {
    root: PathBuf,
    token_ttl: Duration,
}

impl SandboxManager {
    /// Opens a private sandbox root with a non-zero token lifetime.
    ///
    /// # Errors
    ///
    /// Rejects symlinks, non-directories, and zero token lifetimes.
    pub fn open(root: impl Into<PathBuf>, token_ttl: Duration) -> Result<Self, RuntimeError> {
        if token_ttl.is_zero() {
            return Err(RuntimeError::InvalidSpec("token TTL must be non-zero"));
        }
        let root = root.into();
        match fs::symlink_metadata(&root) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(RuntimeError::InvalidSpec("sandbox root is unsafe"));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir_all(&root)?;
            }
            Err(error) => return Err(error.into()),
        }
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
        Ok(Self { root, token_ttl })
    }

    /// Creates a detached worktree and opaque capability proof for one run.
    ///
    /// # Errors
    ///
    /// Fails if the run already exists, randomness is unavailable, or Git cannot
    /// materialize the requested repository revision.
    pub fn create(&self, spec: &RunSpec) -> Result<(Sandbox, CapabilityToken), RuntimeError> {
        let run_root = self.root.join(spec.run_id());
        fs::create_dir(&run_root)?;
        fs::set_permissions(&run_root, fs::Permissions::from_mode(0o700))?;
        let worktree = run_root.join("worktree");
        let execution_dir = run_root.join("execution");
        fs::create_dir(&execution_dir)?;
        fs::set_permissions(&execution_dir, fs::Permissions::from_mode(0o700))?;

        let output = Command::new("git")
            .arg("-C")
            .arg(spec.source_repository())
            .args(["worktree", "add", "--detach"])
            .arg(&worktree)
            .arg("HEAD")
            .output()?;
        if !output.status.success() {
            let _ignored = fs::remove_dir_all(&run_root);
            return Err(RuntimeError::Git(redacted_git_failure(&output.stderr)));
        }
        let mut secret = [0_u8; 32];
        File::open("/dev/urandom")?.read_exact(&mut secret)?;
        let expires_at = Instant::now() + self.token_ttl;
        let token = CapabilityToken {
            secret,
            run_id: spec.run_id().to_owned(),
            expires_at,
        };
        let sandbox = Sandbox {
            run_id: spec.run_id().to_owned(),
            run_root,
            worktree,
            execution_dir,
            source_repository: spec.source_repository().to_owned(),
            capabilities: spec.capabilities(),
            secret,
            expires_at,
        };
        Ok((sandbox, token))
    }
}

fn constant_time_equal(left: &[u8; 32], right: &[u8; 32]) -> bool {
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

fn redacted_git_failure(stderr: &[u8]) -> String {
    let line = String::from_utf8_lossy(stderr);
    line.lines()
        .next()
        .unwrap_or("git operation failed")
        .chars()
        .take(256)
        .collect()
}
