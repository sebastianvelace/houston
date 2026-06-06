//! macOS sandbox backend: closefds + Seatbelt via `sandbox-exec`.

use super::{SandboxBackend, SandboxCapabilities};
use crate::macos_exec::{build_sandbox_exec_command, sandbox_exec_available};
use crate::SandboxError;
use houston_policy::SessionPolicy;
use std::os::unix::io::RawFd;
use tokio::process::Command;

pub struct MacosBackend;

impl SandboxBackend for MacosBackend {
    fn wrap_command(&self, cmd: Command, policy: &SessionPolicy) -> Result<Command, SandboxError> {
        if crate::sandbox_strict() && !sandbox_exec_available() {
            return Err(SandboxError::Unsupported {
                platform: "macos-seatbelt",
                message: "sandbox-exec is not available. \
                          Set HOUSTON_SANDBOX=permissive to spawn without isolation."
                    .into(),
            });
        }

        let (mut wrapped, profile_path) = build_sandbox_exec_command(cmd, policy).map_err(
            |message| SandboxError::Unsupported {
                platform: "macos-seatbelt",
                message,
            },
        )?;

        unsafe {
            wrapped.pre_exec(|| {
                close_inherited_fds();
                Ok(())
            });
        }

        // Profile file must survive until `sandbox-exec` reads it at spawn.
        std::mem::forget(profile_path);
        Ok(wrapped)
    }

    fn capabilities(&self) -> SandboxCapabilities {
        SandboxCapabilities {
            filesystem_isolation: sandbox_exec_available(),
            network_isolation: false,
            fd_cleanup: true,
            credential_isolation: true,
            platform: "macos-seatbelt",
        }
    }
}

fn close_inherited_fds() {
    let entries = match std::fs::read_dir("/dev/fd") {
        Ok(e) => e,
        Err(_) => return,
    };

    let fds: Vec<RawFd> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            e.file_name()
                .to_str()
                .and_then(|s| s.parse::<RawFd>().ok())
        })
        .filter(|&fd| fd >= 3)
        .collect();

    for fd in fds {
        unsafe { libc::close(fd) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use houston_policy::SessionPolicy;
    use std::path::PathBuf;

    #[test]
    fn wrap_command_succeeds_when_sandbox_exec_present() {
        if !sandbox_exec_available() {
            return;
        }
        let cmd = Command::new("true");
        let policy = SessionPolicy::for_working_dir(PathBuf::from("/tmp"), None);
        MacosBackend.wrap_command(cmd, &policy).expect("wrap ok");
    }

    #[test]
    fn capabilities_report_seatbelt_backend() {
        let caps = MacosBackend.capabilities();
        assert!(caps.credential_isolation);
        assert_eq!(caps.platform, "macos-seatbelt");
    }
}
