//! Windows sandbox backend.
//!
//! Full Job Object + restricted-token isolation is tracked as follow-up
//! work. In strict mode this backend fails closed instead of silently
//! spawning an unrestricted subprocess.

use super::{SandboxBackend, SandboxCapabilities};
use crate::SandboxError;
use houston_policy::SessionPolicy;
use tokio::process::Command;

pub struct WindowsBackend;

impl SandboxBackend for WindowsBackend {
    fn wrap_command(&self, cmd: Command, _policy: &SessionPolicy) -> Result<Command, SandboxError> {
        if crate::sandbox_strict() {
            return Err(SandboxError::Unsupported {
                platform: "windows-stub",
                message: "OS-level sandbox is not implemented on Windows yet. \
                          Set HOUSTON_SANDBOX=permissive to spawn without isolation, \
                          or HOUSTON_SANDBOX=off to disable this check."
                    .into(),
            });
        }
        Ok(cmd)
    }

    fn capabilities(&self) -> SandboxCapabilities {
        SandboxCapabilities {
            filesystem_isolation: false,
            network_isolation: false,
            fd_cleanup: false,
            credential_isolation: true,
            platform: "windows-stub",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use houston_policy::SessionPolicy;
    use std::path::PathBuf;

    #[test]
    fn strict_mode_fails_closed() {
        std::env::set_var("HOUSTON_SANDBOX", "strict");
        let cmd = Command::new("cmd");
        let policy = SessionPolicy::for_working_dir(PathBuf::from("C:\\agent"), None);
        let err = WindowsBackend.wrap_command(cmd, &policy).unwrap_err();
        assert!(matches!(err, SandboxError::Unsupported { .. }));
        std::env::remove_var("HOUSTON_SANDBOX");
    }

    #[test]
    fn permissive_mode_allows_spawn() {
        std::env::set_var("HOUSTON_SANDBOX", "permissive");
        let cmd = Command::new("cmd");
        let policy = SessionPolicy::for_working_dir(PathBuf::from("C:\\agent"), None);
        WindowsBackend.wrap_command(cmd, &policy).expect("permissive ok");
        std::env::remove_var("HOUSTON_SANDBOX");
    }
}
