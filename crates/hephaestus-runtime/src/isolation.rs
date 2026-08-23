use std::{fmt::Write as _, path::PathBuf, process::Command};

use crate::{ProviderInvocation, RuntimeError, Sandbox};

/// External operating-system isolation backend selected without downgrade.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IsolationBackend {
    /// macOS Seatbelt through the system `sandbox-exec` launcher.
    MacOsSeatbelt,
    /// No verified external sandbox exists on this host.
    Unavailable,
}

/// Immutable external filesystem and network isolation policy.
pub struct IsolationPolicy {
    backend: IsolationBackend,
    protected_paths: Vec<PathBuf>,
}

impl IsolationPolicy {
    /// Detects a supported OS sandbox and records canonical paths workers cannot access.
    #[must_use]
    pub fn detect(protected_paths: impl IntoIterator<Item = PathBuf>) -> Self {
        let backend = if cfg!(target_os = "macos")
            && std::path::Path::new("/usr/bin/sandbox-exec").is_file()
        {
            IsolationBackend::MacOsSeatbelt
        } else {
            IsolationBackend::Unavailable
        };
        Self {
            backend,
            protected_paths: protected_paths.into_iter().collect(),
        }
    }

    /// Reports the selected backend.
    #[must_use]
    pub const fn backend(&self) -> IsolationBackend {
        self.backend
    }

    /// Builds an externally sandboxed provider process without starting it.
    ///
    /// # Errors
    ///
    /// Fails closed when no verified backend exists or a policy path is not UTF-8.
    pub fn command(
        &self,
        invocation: &ProviderInvocation,
        sandbox: &Sandbox,
    ) -> Result<Command, RuntimeError> {
        let mut command = Command::new(self.launcher()?);
        let profile = self.macos_profile(invocation, sandbox)?;
        command.args(["-p", &profile]);
        command.arg(invocation.program());
        command.args(invocation.arguments());
        command.current_dir(sandbox.worktree());
        Ok(command)
    }

    fn launcher(&self) -> Result<&'static str, RuntimeError> {
        match self.backend {
            IsolationBackend::MacOsSeatbelt => Ok("/usr/bin/sandbox-exec"),
            IsolationBackend::Unavailable => Err(RuntimeError::Unsupported(
                "no verified external sandbox backend",
            )),
        }
    }

    fn macos_profile(
        &self,
        invocation: &ProviderInvocation,
        sandbox: &Sandbox,
    ) -> Result<String, RuntimeError> {
        build_macos_profile(
            invocation.program(),
            sandbox.run_root(),
            &self.protected_paths,
            sandbox.capabilities().allows_network(),
        )
    }
}

fn build_macos_profile(
    executable: &std::path::Path,
    run_root: &std::path::Path,
    protected_paths: &[PathBuf],
    network: bool,
) -> Result<String, RuntimeError> {
    let run_root = quote_path(run_root)?;
    let executable = quote_path(executable)?;
    let mut profile = format!(
        "(version 1)\n(deny default)\n(import \"system.sb\")\n(allow process*)\n(allow sysctl-read)\n(allow mach-lookup)\n(allow file-read-metadata)\n(allow file-read* (subpath \"/System\") (subpath \"/usr\") (subpath \"/bin\") (literal {executable}) (subpath {run_root}))\n(allow file-write* (subpath {run_root}) (literal \"/dev/null\"))\n"
    );
    for path in protected_paths {
        writeln!(
            &mut profile,
            "(deny file-read* file-write* (subpath {}))",
            quote_path(path)?
        )
        .expect("writing to a String cannot fail");
    }
    if !network {
        profile.push_str("(deny network*)\n");
    }
    Ok(profile)
}

fn quote_path(path: &std::path::Path) -> Result<String, RuntimeError> {
    let canonical = path.canonicalize()?;
    let value = canonical
        .to_str()
        .ok_or(RuntimeError::InvalidSpec("sandbox path is not UTF-8"))?;
    if value.chars().any(char::is_control) {
        return Err(RuntimeError::InvalidSpec(
            "sandbox path contains control characters",
        ));
    }
    Ok(format!(
        "\"{}\"",
        value.replace('\\', "\\\\").replace('"', "\\\"")
    ))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn seatbelt_profile_is_deny_by_default_and_capability_exact() {
        let directory = tempdir().expect("policy directory");
        let run = directory.path().join("run");
        let protected = directory.path().join("canonical");
        fs::create_dir(&run).expect("create run root");
        fs::create_dir(&protected).expect("create protected root");
        let executable = std::path::Path::new("/bin/cat");
        let offline =
            build_macos_profile(executable, &run, std::slice::from_ref(&protected), false)
                .expect("build offline profile");
        assert!(offline.contains("(deny default)"));
        assert!(offline.contains("(deny network*)"));
        assert!(offline.contains(&quote_path(&protected).expect("quote protected path")));
        let online =
            build_macos_profile(executable, &run, &[], true).expect("build online profile");
        assert!(!online.contains("(deny network*)"));
    }

    #[test]
    fn seatbelt_profile_rejects_control_characters_in_paths() {
        let directory = tempdir().expect("policy directory");
        let executable = std::path::Path::new("/bin/cat");
        let invalid = directory.path().join("line\nbreak");
        fs::create_dir(&invalid).expect("create invalid path fixture");
        assert!(matches!(
            build_macos_profile(executable, &invalid, &[], false),
            Err(RuntimeError::InvalidSpec(_))
        ));
    }

    #[test]
    fn unavailable_backend_fails_closed_before_process_construction() {
        let policy = IsolationPolicy {
            backend: IsolationBackend::Unavailable,
            protected_paths: Vec::new(),
        };
        assert!(matches!(
            policy.launcher(),
            Err(RuntimeError::Unsupported(_))
        ));
    }
}
