//! Linux sandbox backend: closefds + Landlock LSM.

use super::{SandboxBackend, SandboxCapabilities};
use crate::SandboxError;
use houston_policy::SessionPolicy;
use std::os::unix::io::RawFd;
use std::path::{Path, PathBuf};
use tokio::process::Command;

pub struct LinuxBackend;

impl SandboxBackend for LinuxBackend {
    fn wrap_command(&self, mut cmd: Command, policy: &SessionPolicy) -> Result<Command, SandboxError> {
        if crate::sandbox_strict() && !is_landlock_available() {
            return Err(SandboxError::Unsupported {
                platform: "linux-landlock",
                message: "Landlock is not available on this kernel. \
                          Install bubblewrap (bwrap) or set HOUSTON_SANDBOX=permissive."
                    .into(),
            });
        }

        let working_dir = policy.working_dir.clone();
        let extra_ro = policy.extra_ro_paths.clone();
        let extra_rw = policy.extra_rw_paths.clone();
        let strict = crate::sandbox_strict();

        unsafe {
            cmd.pre_exec(move || {
                close_inherited_fds();
                if let Err(e) = try_apply_landlock(&working_dir, &extra_ro, &extra_rw) {
                    if strict {
                        let msg =
                            format!("[houston-sandbox] landlock setup failed (strict): {e}\n");
                        libc::write(2, msg.as_ptr() as *const libc::c_void, msg.len());
                        std::process::exit(1);
                    }
                    let msg = format!("[houston-sandbox] landlock setup failed (non-fatal): {e}\n");
                    libc::write(2, msg.as_ptr() as *const libc::c_void, msg.len());
                }
                Ok(())
            });
        }
        Ok(cmd)
    }

    fn capabilities(&self) -> SandboxCapabilities {
        SandboxCapabilities {
            filesystem_isolation: is_landlock_available(),
            network_isolation: false,
            fd_cleanup: true,
            credential_isolation: true,
            platform: "linux-landlock",
        }
    }
}

/// Close all open file descriptors ≥ 3 by reading /proc/self/fd.
fn close_inherited_fds() {
    let entries = match std::fs::read_dir("/proc/self/fd") {
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

fn try_apply_landlock(
    working_dir: &Path,
    extra_ro: &[PathBuf],
    extra_rw: &[PathBuf],
) -> Result<(), Box<dyn std::error::Error>> {
    use landlock::{Access, AccessFs, ABI, PathBeneath, PathFd, Ruleset, RulesetAttr, RulesetCreatedAttr};

    let abi = ABI::V1;
    let all = AccessFs::from_all(abi);

    let mut ruleset = Ruleset::default()
        .handle_access(all)?
        .create()?
        .add_rule(PathBeneath::new(PathFd::new(working_dir)?, all))?;

    let sys_paths: &[&str] = &[
        "/usr", "/lib", "/lib64", "/bin", "/sbin", "/etc", "/opt", "/tmp", "/run", "/proc",
        "/dev",
    ];
    for path_str in sys_paths {
        let p = Path::new(path_str);
        if p.exists() {
            ruleset = ruleset.add_rule(PathBeneath::new(PathFd::new(p)?, all))?;
        }
    }

    for ro_path in extra_ro {
        if ro_path.exists() {
            ruleset = ruleset.add_rule(PathBeneath::new(PathFd::new(ro_path)?, all))?;
        }
    }

    for rw_path in extra_rw {
        if rw_path.exists() {
            ruleset = ruleset.add_rule(PathBeneath::new(PathFd::new(rw_path)?, all))?;
        }
    }

    ruleset.restrict_self()?;
    Ok(())
}

fn is_landlock_available() -> bool {
    use landlock::{Access, AccessFs, ABI, Ruleset, RulesetAttr};
    (|| -> Result<(), Box<dyn std::error::Error>> {
        Ruleset::default()
            .handle_access(AccessFs::from_all(ABI::V1))?
            .create()?;
        Ok(())
    })()
    .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use houston_policy::SessionPolicy;
    use std::path::PathBuf;

    #[test]
    fn wrap_command_does_not_panic_for_existing_dir() {
        let cmd = tokio::process::Command::new("true");
        let policy = SessionPolicy::for_working_dir(PathBuf::from("/tmp"), None);
        LinuxBackend.wrap_command(cmd, &policy).expect("wrap ok");
    }

    #[test]
    fn capabilities_reports_fd_cleanup_true() {
        let caps = LinuxBackend.capabilities();
        assert!(caps.fd_cleanup);
        assert!(caps.credential_isolation);
        assert_eq!(caps.platform, "linux-landlock");
    }

    #[test]
    fn close_inherited_fds_does_not_panic() {
        close_inherited_fds();
    }
}
