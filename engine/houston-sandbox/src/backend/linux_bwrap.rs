//! Linux bubblewrap backend: mount namespaces + explicit bind allowlist.

use super::linux_bwrap_args::build_bwrap_args;
use super::super::linux_seccomp;
use super::{SandboxBackend, SandboxCapabilities};
use crate::SandboxError;
use houston_policy::SessionPolicy;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::sync::OnceLock;
use tokio::process::Command;

pub struct BwrapBackend;

static BWRAP_USABLE: OnceLock<bool> = OnceLock::new();

/// `bwrap` is on PATH and a minimal namespace probe succeeds in this process.
///
/// Bubblewrap can be installed yet fail at runtime (e.g. `bwrap: Failed to make /
/// slave: Operation not permitted` when the engine lacks mount-namespace rights).
/// Auto backend selection must probe, not just check the binary path.
pub fn bwrap_available() -> bool {
    which_bwrap().is_some() && bwrap_usable()
}

/// Cached functional probe — runs once per process in the engine's own context.
pub fn bwrap_usable() -> bool {
    *BWRAP_USABLE.get_or_init(probe_bwrap_usable)
}

pub fn which_bwrap() -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).find_map(|dir| {
            let candidate = dir.join("bwrap");
            if candidate.is_file() {
                Some(candidate)
            } else {
                None
            }
        })
    })
}

fn probe_bwrap_usable() -> bool {
    let Some(bwrap) = which_bwrap() else {
        return false;
    };

    let probe_dir = std::env::temp_dir().join(format!(
        "houston-bwrap-probe-{}",
        std::process::id()
    ));
    if std::fs::create_dir_all(&probe_dir).is_err() {
        return false;
    }

    let probe = probe_dir.as_os_str();
    let mut cmd = std::process::Command::new(&bwrap);
    cmd.arg("--die-with-parent").arg("--new-session");
    for path in ["/usr", "/lib", "/lib64", "/bin", "/sbin", "/etc", "/opt"] {
        if PathBuf::from(path).exists() {
            cmd.arg("--ro-bind").arg(path).arg(path);
        }
    }
    cmd.args([
        "--dev", "/dev", "--proc", "/proc", "--tmpfs", "/tmp", "--bind",
    ]);
    cmd.arg(probe).arg(probe);
    cmd.arg("--chdir").arg(probe);
    cmd.arg("--").arg("/usr/bin/true");

    let ok = cmd
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let _ = std::fs::remove_dir(&probe_dir);

    if !ok {
        tracing::warn!(
            "bubblewrap is installed but cannot create mount namespaces in this \
             process; falling back to landlock sandbox backend"
        );
    }
    ok
}

impl SandboxBackend for BwrapBackend {
    fn wrap_command(&self, cmd: Command, policy: &SessionPolicy) -> Result<Command, SandboxError> {
        if crate::sandbox_strict() && !bwrap_available() {
            return Err(SandboxError::Unsupported {
                platform: "linux-bwrap",
                message: "bubblewrap (bwrap) is not installed. \
                          Install bubblewrap or set HOUSTON_SANDBOX=permissive."
                    .into(),
            });
        }
        let std_cmd = cmd.as_std();
        let program = std_cmd.get_program().to_os_string();
        let args: Vec<_> = std_cmd.get_args().map(OsStr::to_os_string).collect();
        let cwd = std_cmd.get_current_dir().map(PathBuf::from);
        let envs: Vec<(std::ffi::OsString, Option<std::ffi::OsString>)> = std_cmd
            .get_envs()
            .map(|(k, v)| (k.to_os_string(), v.map(OsStr::to_os_string)))
            .collect();

        let bwrap_args = build_bwrap_args(policy, &program, &args, cwd.as_deref())?;
        let mut wrapped = Command::new(bwrap_args.bwrap_bin);
        for arg in bwrap_args.args {
            wrapped.arg(arg);
        }
        if let Some(dir) = cwd {
            wrapped.current_dir(dir);
        }
        for (key, val) in envs {
            if let Some(v) = val {
                wrapped.env(key, v);
            } else {
                wrapped.env_remove(key);
            }
        }

        let strict = crate::sandbox_strict();
        unsafe {
            wrapped.pre_exec(move || {
                if let Err(e) = linux_seccomp::install_dangerous_syscall_filter() {
                    let msg = format!("[houston-sandbox] seccomp setup failed: {e}\n");
                    libc::write(2, msg.as_ptr() as *const libc::c_void, msg.len());
                    if strict {
                        std::process::exit(1);
                    }
                }
                Ok(())
            });
        }
        Ok(wrapped)
    }

    fn capabilities(&self) -> SandboxCapabilities {
        SandboxCapabilities {
            filesystem_isolation: bwrap_available(),
            network_isolation: false,
            fd_cleanup: true,
            credential_isolation: true,
            platform: "linux-bwrap",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bwrap_usable_matches_probe_when_installed() {
        if which_bwrap().is_none() {
            return;
        }
        assert_eq!(bwrap_usable(), probe_bwrap_usable());
    }

    #[test]
    fn bwrap_available_requires_functional_probe() {
        if which_bwrap().is_none() {
            assert!(!bwrap_available());
            return;
        }
        assert_eq!(bwrap_available(), bwrap_usable());
    }
}
