//! Bubblewrap argv construction for the Linux sandbox backend.

use super::linux_bwrap::which_bwrap;
use crate::SandboxError;
use houston_policy::SessionPolicy;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

pub(crate) struct BwrapArgs {
    pub bwrap_bin: PathBuf,
    pub args: Vec<String>,
}

pub(crate) fn build_bwrap_args(
    policy: &SessionPolicy,
    program: &OsStr,
    args: &[std::ffi::OsString],
    cwd: Option<&Path>,
) -> Result<BwrapArgs, SandboxError> {
    let bwrap_bin = which_bwrap().ok_or_else(|| SandboxError::Unsupported {
        platform: "linux-bwrap",
        message: "bwrap not found on PATH".into(),
    })?;

    let agent_root = canonical_existing(&policy.working_dir)?;
    let mut out = vec!["--die-with-parent".into(), "--new-session".into()];

    for (host, target) in system_ro_binds() {
        if host.exists() {
            out.push("--ro-bind".into());
            out.push(host.display().to_string());
            out.push(target.display().to_string());
        }
    }

    out.extend(
        [
            "--dev", "/dev", "--proc", "/proc", "--tmpfs", "/tmp", "--bind",
        ]
        .iter()
        .map(|s| (*s).to_string()),
    );
    out.push(agent_root.display().to_string());
    out.push(agent_root.display().to_string());

    if let Some(ws_md) = workspace_markdown(&agent_root) {
        if ws_md.exists() {
            out.push("--ro-bind".into());
            out.push(ws_md.display().to_string());
            out.push(ws_md.display().to_string());
        }
    }

    for ro in &policy.extra_ro_paths {
        if ro.exists() {
            let c = canonical_existing(ro)?;
            out.push("--ro-bind".into());
            out.push(c.display().to_string());
            out.push(c.display().to_string());
        }
    }

    for rw in &policy.extra_rw_paths {
        if rw.exists() {
            let c = canonical_existing(rw)?;
            out.push("--bind".into());
            out.push(c.display().to_string());
            out.push(c.display().to_string());
        }
    }

    let chdir = cwd.unwrap_or(&agent_root);
    out.push("--chdir".into());
    out.push(chdir.display().to_string());
    out.push("--".into());
    out.push(program.to_string_lossy().into_owned());
    for arg in args {
        out.push(arg.to_string_lossy().into_owned());
    }

    Ok(BwrapArgs { bwrap_bin, args: out })
}

fn system_ro_binds() -> Vec<(PathBuf, PathBuf)> {
    let mut binds: Vec<(PathBuf, PathBuf)> = ["/usr", "/lib", "/lib64", "/bin", "/sbin", "/etc", "/opt"]
        .iter()
        .map(|p| (PathBuf::from(p), PathBuf::from(p)))
        .collect();
    // /etc/resolv.conf on systemd-resolved systems is a symlink into /run.
    // Bind the resolve dir so DNS works inside the container.
    let resolve_dir = PathBuf::from("/run/systemd/resolve");
    if resolve_dir.exists() {
        binds.push((resolve_dir.clone(), resolve_dir));
    }
    binds
}

fn workspace_markdown(agent_root: &Path) -> Option<PathBuf> {
    agent_root.parent().map(|ws| ws.join("WORKSPACE.md"))
}

fn canonical_existing(path: &Path) -> Result<PathBuf, SandboxError> {
    std::fs::canonicalize(path).map_err(|e| SandboxError::Unsupported {
        platform: "linux-bwrap",
        message: format!("sandbox path {} unavailable: {e}", path.display()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::linux_bwrap::bwrap_available;
    use houston_policy::SessionPolicy;

    #[test]
    fn bwrap_wraps_command_with_correct_args() {
        if !bwrap_available() {
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let policy = SessionPolicy::for_working_dir(tmp.path().to_path_buf(), None);
        let args = build_bwrap_args(
            &policy,
            OsStr::new("sh"),
            &[std::ffi::OsString::from("-c"), std::ffi::OsString::from("echo")],
            Some(tmp.path()),
        )
        .expect("args");
        assert!(args.args.iter().any(|a| a == "--ro-bind"));
        assert!(args.args.iter().any(|a| a == "--new-session"));
        assert!(args.args.iter().any(|a| a == "--"));
    }

    #[test]
    fn bwrap_excludes_credential_paths() {
        if !bwrap_available() {
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let policy = SessionPolicy::for_working_dir(tmp.path().to_path_buf(), None);
        let args = build_bwrap_args(&policy, OsStr::new("true"), &[], Some(tmp.path()))
            .expect("args");
        let joined = args.args.join(" ");
        assert!(!joined.contains(".claude"));
        assert!(!joined.contains(".codex"));
        assert!(!joined.contains(".gemini"));
    }

    #[test]
    fn bwrap_includes_agent_root() {
        if !bwrap_available() {
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_string_lossy().to_string();
        let policy = SessionPolicy::for_working_dir(tmp.path().to_path_buf(), None);
        let args = build_bwrap_args(&policy, OsStr::new("true"), &[], Some(tmp.path()))
            .expect("args");
        assert!(args.args.windows(2).any(|w| w[0] == "--bind" && w[1] == root));
    }

    #[test]
    fn bwrap_binds_extra_rw_paths() {
        if !bwrap_available() {
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let rw = tmp.path().join("runtime");
        std::fs::create_dir_all(&rw).unwrap();
        let policy = SessionPolicy::for_working_dir(tmp.path().join("agent"), None)
            .with_rw_path(rw.clone());
        std::fs::create_dir_all(policy.working_dir.clone()).unwrap();
        let args = build_bwrap_args(&policy, OsStr::new("true"), &[], Some(&policy.working_dir))
            .expect("args");
        let rw_str = rw.display().to_string();
        assert!(
            args.args
                .windows(3)
                .any(|w| w[0] == "--bind" && w[1] == rw_str && w[2] == rw_str),
            "expected rw bind for runtime path: {:?}",
            args.args
        );
    }

    #[test]
    fn denied_prefixes_not_bound_by_bwrap() {
        if !bwrap_available() {
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path().join("MiEmpresa");
        let marketing = ws.join("Marketing");
        let contabilidad = ws.join("Contabilidad");
        std::fs::create_dir_all(&marketing).unwrap();
        std::fs::create_dir_all(&contabilidad).unwrap();

        let policy = SessionPolicy::for_working_dir(marketing.clone(), Some(ws));
        let args = build_bwrap_args(&policy, OsStr::new("true"), &[], Some(&marketing))
            .expect("args");
        let joined = args.args.join(" ");
        assert!(
            policy
                .denied_prefixes
                .iter()
                .any(|p| p.ends_with("Contabilidad")),
            "policy must deny sibling agent"
        );
        assert!(
            !joined.contains("Contabilidad"),
            "bwrap must not bind denied sibling paths"
        );
        assert!(!joined.contains(".claude"));
        assert!(!joined.contains(".codex"));
    }
}
