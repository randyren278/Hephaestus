use std::{
    fs::{self, File},
    io::Read,
    os::unix::fs::{MetadataExt, PermissionsExt},
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
    source_revision: String,
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

    pub(crate) fn run_root(&self) -> &Path {
        &self.run_root
    }

    /// Maximum authority granted to the worker.
    #[must_use]
    pub const fn capabilities(&self) -> CapabilitySet {
        self.capabilities
    }

    /// Exact Git commit materialized in this sandbox.
    #[must_use]
    pub fn source_revision(&self) -> &str {
        &self.source_revision
    }

    pub(crate) fn authorize_spec(
        &self,
        token: &CapabilityToken,
        spec: &RunSpec,
    ) -> Result<(), RuntimeError> {
        self.authorize(token, spec.capabilities())?;
        self.validate_spec_binding(spec)
    }

    pub(crate) fn validate_spec_binding(&self, spec: &RunSpec) -> Result<(), RuntimeError> {
        if self.run_id != spec.run_id()
            || self.source_repository != spec.source_repository()
            || self.source_revision != spec.source_revision()
        {
            return Err(RuntimeError::InvalidSpec(
                "run specification does not match the materialized sandbox",
            ));
        }
        Ok(())
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
        cleanup_paths(
            Path::new("git"),
            &self.source_repository,
            &self.worktree,
            &self.run_root,
            true,
        )
        .map_err(CleanupFailures::into_runtime_error)
    }
}

/// Creates sibling-separated private Git worktrees under one sandbox root.
pub struct SandboxManager {
    root: PathBuf,
    root_device: u64,
    root_inode: u64,
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
        let canonical_root = fs::canonicalize(&root)?;
        let metadata = fs::symlink_metadata(canonical_root)?;
        Ok(Self {
            root,
            root_device: metadata.dev(),
            root_inode: metadata.ino(),
            token_ttl,
        })
    }

    /// Creates a detached worktree and opaque capability proof for one run.
    ///
    /// # Errors
    ///
    /// Fails if the run already exists, randomness is unavailable, or Git cannot
    /// materialize the requested repository revision.
    pub fn create(&self, spec: &RunSpec) -> Result<(Sandbox, CapabilityToken), RuntimeError> {
        self.create_with(spec, Path::new("git"), Path::new("git"), |secret| {
            File::open("/dev/urandom")?.read_exact(secret)
        })
    }

    fn create_with<F>(
        &self,
        spec: &RunSpec,
        git_program: &Path,
        cleanup_git_program: &Path,
        fill_entropy: F,
    ) -> Result<(Sandbox, CapabilityToken), RuntimeError>
    where
        F: FnOnce(&mut [u8; 32]) -> std::io::Result<()>,
    {
        self.validate_root()?;
        let run_root = self.root.join(spec.run_id());
        fs::create_dir(&run_root)?;
        let worktree = run_root.join("worktree");
        let mut rollback = CreationRollback::new(
            spec.source_repository(),
            &run_root,
            &worktree,
            cleanup_git_program,
        );
        let execution_dir = run_root.join("execution");
        let creation = (|| {
            fs::set_permissions(&run_root, fs::Permissions::from_mode(0o700))?;
            fs::create_dir(&execution_dir)?;
            fs::set_permissions(&execution_dir, fs::Permissions::from_mode(0o700))?;
            let output = Command::new(git_program)
                .arg("-C")
                .arg(spec.source_repository())
                .args(["worktree", "add", "--detach"])
                .arg(&worktree)
                .arg(spec.source_revision())
                .output()?;
            rollback.require_git_cleanup();
            if !output.status.success() {
                return Err(RuntimeError::Git(redacted_git_failure(&output.stderr)));
            }
            let mut secret = [0_u8; 32];
            fill_entropy(&mut secret)?;
            let expires_at = Instant::now()
                .checked_add(self.token_ttl)
                .ok_or(RuntimeError::InvalidSpec("token TTL exceeds clock range"))?;
            Ok((secret, expires_at))
        })();
        let (secret, expires_at) = match creation {
            Ok(created) => created,
            Err(operation) => return Err(rollback.finish_error(operation)),
        };
        let token = CapabilityToken {
            secret,
            run_id: spec.run_id().to_owned(),
            expires_at,
        };
        rollback.disarm();
        drop(rollback);
        let sandbox = Sandbox {
            run_id: spec.run_id().to_owned(),
            run_root,
            worktree,
            execution_dir,
            source_repository: spec.source_repository().to_owned(),
            source_revision: spec.source_revision().to_owned(),
            capabilities: spec.capabilities(),
            secret,
            expires_at,
        };
        Ok((sandbox, token))
    }

    fn validate_root(&self) -> Result<(), RuntimeError> {
        let metadata = fs::symlink_metadata(&self.root)?;
        if metadata.file_type().is_symlink()
            || !metadata.is_dir()
            || metadata.dev() != self.root_device
            || metadata.ino() != self.root_inode
        {
            return Err(RuntimeError::InvalidSpec(
                "sandbox root identity changed after opening",
            ));
        }
        Ok(())
    }
}

struct CreationRollback<'a> {
    source_repository: &'a Path,
    run_root: &'a Path,
    worktree: &'a Path,
    git_program: &'a Path,
    git_cleanup_required: bool,
    armed: bool,
}

impl<'a> CreationRollback<'a> {
    const fn new(
        source_repository: &'a Path,
        run_root: &'a Path,
        worktree: &'a Path,
        git_program: &'a Path,
    ) -> Self {
        Self {
            source_repository,
            run_root,
            worktree,
            git_program,
            git_cleanup_required: false,
            armed: true,
        }
    }

    const fn require_git_cleanup(&mut self) {
        self.git_cleanup_required = true;
    }

    fn finish_error(&mut self, operation: RuntimeError) -> RuntimeError {
        match self.rollback() {
            Ok(()) => operation,
            Err(failures) => failures.into_rollback_error(operation),
        }
    }

    fn rollback(&mut self) -> Result<(), CleanupFailures> {
        self.armed = false;
        cleanup_paths(
            self.git_program,
            self.source_repository,
            self.worktree,
            self.run_root,
            self.git_cleanup_required,
        )
    }

    const fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for CreationRollback<'_> {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let _ignored = self.rollback();
    }
}

struct CleanupFailures {
    git_failed: bool,
    filesystem_failed: bool,
}

impl CleanupFailures {
    const fn into_runtime_error(self) -> RuntimeError {
        RuntimeError::CleanupFailed {
            git_failed: self.git_failed,
            filesystem_failed: self.filesystem_failed,
        }
    }

    fn into_rollback_error(self, operation: RuntimeError) -> RuntimeError {
        RuntimeError::RollbackFailed {
            operation: Box::new(operation),
            git_failed: self.git_failed,
            filesystem_failed: self.filesystem_failed,
        }
    }
}

fn cleanup_paths(
    git_program: &Path,
    source_repository: &Path,
    worktree: &Path,
    run_root: &Path,
    git_cleanup_required: bool,
) -> Result<(), CleanupFailures> {
    let git_failed = if git_cleanup_required {
        match Command::new(git_program)
            .arg("-C")
            .arg(source_repository)
            .args(["worktree", "remove", "--force"])
            .arg(worktree)
            .output()
        {
            Ok(output) => !output.status.success(),
            Err(_) => true,
        }
    } else {
        false
    };
    let filesystem_failed = match fs::symlink_metadata(run_root) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            fs::remove_file(run_root).is_err()
        }
        Ok(_) => fs::remove_dir_all(run_root).is_err(),
        Err(error) => error.kind() != std::io::ErrorKind::NotFound,
    };
    if git_failed || filesystem_failed {
        Err(CleanupFailures {
            git_failed,
            filesystem_failed,
        })
    } else {
        Ok(())
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

#[cfg(test)]
mod tests {
    use std::{io, process::Command, time::Duration};

    use hephaestus_core::authority::CapabilitySet;
    use tempfile::tempdir;

    use crate::{Budget, RunSpec, RuntimeError};

    use super::SandboxManager;

    #[test]
    fn creation_rolls_back_filesystem_state_when_git_cannot_spawn() {
        let repository = repository_fixture();
        let root = tempdir().expect("sandbox root");
        let manager =
            SandboxManager::open(root.path(), Duration::from_secs(30)).expect("sandbox manager");
        let spec = run_spec("spawn-failure", repository.path());
        let missing_git = root.path().join("missing-git");

        let result = manager.create_with(&spec, &missing_git, &missing_git, |_| Ok(()));

        assert!(matches!(result, Err(RuntimeError::Io(_))));
        assert!(!root.path().join(spec.run_id()).exists());
        assert!(!worktree_list(repository.path()).contains("spawn-failure"));
    }

    #[test]
    fn creation_rolls_back_worktree_metadata_when_entropy_fails() {
        let repository = repository_fixture();
        let root = tempdir().expect("sandbox root");
        let manager =
            SandboxManager::open(root.path(), Duration::from_secs(30)).expect("sandbox manager");
        let spec = run_spec("entropy-failure", repository.path());
        let worktree = root.path().join(spec.run_id()).join("worktree");

        let result = manager.create_with(
            &spec,
            std::path::Path::new("git"),
            std::path::Path::new("git"),
            |_| Err(io::Error::other("injected entropy failure")),
        );

        assert!(matches!(result, Err(RuntimeError::Io(_))));
        assert!(!root.path().join(spec.run_id()).exists());
        assert!(
            !worktree_list(repository.path()).contains(&format!("worktree {}", worktree.display()))
        );
        let (sandbox, _) = manager.create(&spec).expect("run identity is reusable");
        sandbox.cleanup().expect("clean retried sandbox");
    }

    #[test]
    fn creation_reports_incomplete_compensating_cleanup() {
        let repository = repository_fixture();
        let root = tempdir().expect("sandbox root");
        let manager =
            SandboxManager::open(root.path(), Duration::from_secs(30)).expect("sandbox manager");
        let spec = run_spec("rollback-failure", repository.path());
        let missing_git = root.path().join("missing-cleanup-git");

        let result = manager.create_with(&spec, std::path::Path::new("git"), &missing_git, |_| {
            Err(io::Error::other("injected entropy failure"))
        });

        assert!(matches!(
            result,
            Err(RuntimeError::RollbackFailed {
                git_failed: true,
                filesystem_failed: false,
                ..
            })
        ));
        assert!(!root.path().join(spec.run_id()).exists());
        assert!(worktree_list(repository.path()).contains("rollback-failure"));
        run_git(repository.path(), &["worktree", "prune", "--expire", "now"]);
    }

    #[test]
    fn excessive_token_ttl_fails_without_leaving_a_worktree() {
        let repository = repository_fixture();
        let root = tempdir().expect("sandbox root");
        let manager = SandboxManager::open(root.path(), Duration::MAX).expect("sandbox manager");
        let spec = run_spec("excessive-ttl", repository.path());

        let result = manager.create(&spec);

        assert!(matches!(result, Err(RuntimeError::InvalidSpec(_))));
        assert!(!root.path().join(spec.run_id()).exists());
        assert!(!worktree_list(repository.path()).contains("excessive-ttl"));
    }

    #[test]
    fn creation_rejects_a_replaced_sandbox_root() {
        let repository = repository_fixture();
        let outer = tempdir().expect("outer directory");
        let root = outer.path().join("sandboxes");
        let alternate = outer.path().join("alternate");
        let moved = outer.path().join("original-sandboxes");
        std::fs::create_dir(&root).expect("sandbox root");
        std::fs::create_dir(&alternate).expect("alternate root");
        let manager =
            SandboxManager::open(&root, Duration::from_secs(30)).expect("sandbox manager");
        let spec = run_spec("replaced-root", repository.path());
        std::fs::rename(&root, &moved).expect("move original root");
        std::os::unix::fs::symlink(&alternate, &root).expect("replace root with symlink");

        let result = manager.create(&spec);

        assert!(matches!(result, Err(RuntimeError::InvalidSpec(_))));
        assert!(!alternate.join(spec.run_id()).exists());
        assert!(!moved.join(spec.run_id()).exists());
    }

    fn run_spec(run_id: &str, repository: &std::path::Path) -> RunSpec {
        RunSpec::new(
            run_id,
            "genome",
            "world",
            repository,
            "prompt",
            CapabilitySet::new(false, false),
            Budget::new(Duration::from_secs(5), 1_000, 0).expect("budget"),
        )
        .expect("run spec")
    }

    fn repository_fixture() -> tempfile::TempDir {
        let repository = tempdir().expect("repository");
        run_git(repository.path(), &["init", "-q"]);
        std::fs::write(repository.path().join("fixture"), b"fixture\n").expect("fixture");
        run_git(repository.path(), &["add", "fixture"]);
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

    fn worktree_list(repository: &std::path::Path) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(repository)
            .args(["worktree", "list", "--porcelain"])
            .output()
            .expect("list worktrees");
        assert!(output.status.success());
        String::from_utf8(output.stdout).expect("UTF-8 worktree list")
    }

    fn run_git(repository: &std::path::Path, arguments: &[&str]) {
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(repository)
                .args(arguments)
                .status()
                .expect("run Git")
                .success()
        );
    }
}
