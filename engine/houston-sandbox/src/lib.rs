//! OS-level process isolation for Houston agent CLI subprocesses.

mod backend;
#[cfg(target_os = "linux")]
mod linux_seccomp;
#[cfg(target_os = "macos")]
mod macos_exec;
mod macos_profile;

pub use backend::{detect_backend, SandboxBackend, SandboxCapabilities};
use houston_policy::SessionPolicy;
use std::fmt;
use tokio::process::Command;

/// Sandbox could not be applied for a user-initiated spawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxError {
    Unsupported {
        platform: &'static str,
        message: String,
    },
}

impl fmt::Display for SandboxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported { platform, message } => {
                write!(f, "sandbox backend {platform} unavailable: {message}")
            }
        }
    }
}

impl std::error::Error for SandboxError {}

fn sandbox_enabled() -> bool {
    !matches!(std::env::var("HOUSTON_SANDBOX").as_deref(), Ok("off"))
}

pub(crate) fn sandbox_strict() -> bool {
    match std::env::var("HOUSTON_SANDBOX").as_deref() {
        Ok("permissive") | Ok("0") | Ok("false") => false,
        Ok("strict") | Ok("1") | Ok("true") => true,
        // Unset or unknown: fail closed in release, permissive in debug dev loops.
        Err(_) | Ok(_) => !cfg!(debug_assertions),
    }
}

/// Apply sandbox restrictions to `cmd` based on `policy`.
pub fn wrap_sandbox(cmd: Command, policy: &SessionPolicy) -> Result<Command, SandboxError> {
    if !sandbox_enabled() {
        return Ok(cmd);
    }
    detect_backend().wrap_command(cmd, policy)
}

/// Report what isolation features are available on this platform/kernel.
pub fn capabilities() -> SandboxCapabilities {
    detect_backend().capabilities()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn wrap_sandbox_off_env_is_noop() {
        std::env::set_var("HOUSTON_SANDBOX", "off");
        let cmd = Command::new("true");
        let policy = SessionPolicy::for_working_dir(PathBuf::from("/tmp"), None);
        wrap_sandbox(cmd, &policy).expect("off must not error");
        std::env::remove_var("HOUSTON_SANDBOX");
    }

    #[test]
    fn sandbox_strict_explicit_values() {
        std::env::set_var("HOUSTON_SANDBOX", "strict");
        assert!(sandbox_strict());
        std::env::set_var("HOUSTON_SANDBOX", "permissive");
        assert!(!sandbox_strict());
        std::env::remove_var("HOUSTON_SANDBOX");
    }
}
