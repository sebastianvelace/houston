//! Linux sandbox backend: closefds + Landlock LSM.
//!
//! Applied in `pre_exec` (runs in the forked child, before exec):
//!   1. Close all inherited file descriptors ≥ 3 via /proc/self/fd.
//!   2. Apply Landlock rules: working_dir gets full access; standard
//!      system paths are also allowed (Landlock denies everything else).
//!      Silently degrades on kernels < 5.13 or when Landlock is disabled.
//!
//! Both steps are best-effort — a failure emits a stderr line and
//! continues so the agent session is not bricked by a kernel quirk.

use super::{SandboxBackend, SandboxCapabilities};
use houston_policy::SessionPolicy;
use std::os::unix::io::RawFd;
use std::path::{Path, PathBuf};
use tokio::process::Command;

pub struct LinuxBackend;

impl SandboxBackend for LinuxBackend {
    fn configure(&self, cmd: &mut Command, policy: &SessionPolicy) {
        let working_dir = policy.working_dir.clone();
        let extra_ro = policy.extra_ro_paths.clone();

        unsafe {
            cmd.pre_exec(move || {
                close_inherited_fds();
                apply_landlock_rules(&working_dir, &extra_ro);
                Ok(())
            });
        }
    }

    fn capabilities(&self) -> SandboxCapabilities {
        SandboxCapabilities {
            filesystem_isolation: is_landlock_available(),
            network_isolation: false,
            fd_cleanup: true,
            platform: "linux-landlock",
        }
    }
}

/// Close all open file descriptors ≥ 3 by reading /proc/self/fd.
/// Collect the list BEFORE closing to avoid invalidating the dir iterator.
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
        // EBADF for an already-closed or /proc-iteration fd is benign.
        unsafe { libc::close(fd) };
    }
}

/// Apply Landlock FS rules. Silently degrades on kernels < 5.13.
fn apply_landlock_rules(working_dir: &Path, extra_ro: &[PathBuf]) {
    if let Err(e) = try_apply_landlock(working_dir, extra_ro) {
        let msg = format!(
            "[houston-sandbox] landlock setup failed (non-fatal): {e}\n"
        );
        // tracing is not available in pre_exec — write directly to stderr.
        unsafe {
            libc::write(2, msg.as_ptr() as *const libc::c_void, msg.len());
        }
    }
}

fn try_apply_landlock(
    working_dir: &Path,
    extra_ro: &[PathBuf],
) -> Result<(), Box<dyn std::error::Error>> {
    use landlock::{Access, AccessFs, ABI, PathBeneath, PathFd, Ruleset, RulesetAttr};

    let abi = ABI::V1;
    let all = AccessFs::from_all(abi);

    let mut ruleset = Ruleset::default()
        .handle_access(all)?
        .create()?
        .add_rule(PathBeneath::new(PathFd::new(working_dir)?, all))?;

    // System paths the subprocess needs to function (read-only by OS perms).
    // Landlock allow-lists these; everything else is denied.
    let sys_paths: &[&str] = &[
        "/usr", "/lib", "/lib64", "/bin", "/sbin", "/etc", "/opt", "/tmp",
        // Common on modern distros
        "/run", "/proc", "/dev",
    ];
    for path_str in sys_paths {
        let p = Path::new(path_str);
        if p.exists() {
            ruleset = ruleset.add_rule(PathBeneath::new(PathFd::new(p)?, all))?;
        }
    }

    // Caller-specified extra paths (e.g. credential staging dir).
    for ro_path in extra_ro {
        if ro_path.exists() {
            ruleset = ruleset.add_rule(PathBeneath::new(PathFd::new(ro_path)?, all))?;
        }
    }

    ruleset.restrict_self()?;
    Ok(())
}

fn is_landlock_available() -> bool {
    // Probe: try creating an empty Landlock ruleset. Fails gracefully on
    // kernels without Landlock support (< 5.13 or CONFIG_SECURITY_LANDLOCK=n).
    use landlock::{Access, AccessFs, ABI, Ruleset, RulesetAttr};
    let result: Result<(), Box<dyn std::error::Error>> = (|| {
        Ruleset::default()
            .handle_access(AccessFs::from_all(ABI::V1))?
            .create()?;
        Ok(())
    })();
    result.is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use houston_policy::SessionPolicy;
    use std::path::PathBuf;

    #[test]
    fn configure_does_not_panic_for_existing_dir() {
        let mut cmd = tokio::process::Command::new("true");
        let policy = SessionPolicy::for_working_dir(PathBuf::from("/tmp"));
        LinuxBackend.configure(&mut cmd, &policy);
    }

    #[test]
    fn capabilities_reports_fd_cleanup_true() {
        let caps = LinuxBackend.capabilities();
        assert!(caps.fd_cleanup);
        assert_eq!(caps.platform, "linux-landlock");
    }

    #[test]
    fn close_inherited_fds_does_not_panic() {
        // Smoke test: should silently succeed even if /proc/self/fd is weird.
        close_inherited_fds();
    }
}
