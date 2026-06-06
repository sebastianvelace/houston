//! `sandbox-exec` wrapper construction for the macOS Seatbelt backend.

use crate::macos_profile::{profile_inputs, render_profile};
use houston_policy::SessionPolicy;
use std::ffi::OsStr;
use std::io::Write;
use std::path::{Path, PathBuf};
use tokio::process::Command;

pub fn sandbox_exec_path() -> PathBuf {
    PathBuf::from("/usr/bin/sandbox-exec")
}

pub fn sandbox_exec_available() -> bool {
    sandbox_exec_path().is_file()
}

pub fn build_sandbox_exec_command(
    cmd: Command,
    policy: &SessionPolicy,
) -> Result<(Command, PathBuf), String> {
    let (real_home, houston_data) = profile_inputs(policy);
    let profile = render_profile(policy, &real_home, &houston_data);
    let profile_path = write_profile_file(&profile)?;

    let std_cmd = cmd.as_std();
    let program = std_cmd.get_program().to_os_string();
    let args: Vec<_> = std_cmd.get_args().map(OsStr::to_os_string).collect();
    let cwd = std_cmd.get_current_dir().map(PathBuf::from);
    let envs: Vec<(std::ffi::OsString, Option<std::ffi::OsString>)> = std_cmd
        .get_envs()
        .map(|(k, v)| (k.to_os_string(), v.map(OsStr::to_os_string)))
        .collect();

    let mut wrapped = Command::new(sandbox_exec_path());
    wrapped.arg("-f").arg(&profile_path);
    wrapped.arg("--");
    wrapped.arg(program);
    for arg in args {
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

    Ok((wrapped, profile_path))
}

fn write_profile_file(profile: &str) -> Result<PathBuf, String> {
    let mut tmp = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
    tmp.write_all(profile.as_bytes()).map_err(|e| e.to_string())?;
    let (_file, path) = tmp.keep().map_err(|e| e.to_string())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use houston_policy::SessionPolicy;
    use std::path::PathBuf;

    #[test]
    fn build_command_includes_sandbox_exec_argv() {
        if !sandbox_exec_available() {
            return;
        }
        let cmd = Command::new("true");
        let policy = SessionPolicy::for_working_dir(PathBuf::from("/tmp/agent"), None);
        let (wrapped, _profile) = build_sandbox_exec_command(cmd, &policy).expect("wrap");
        let std_cmd = wrapped.as_std();
        assert_eq!(std_cmd.get_program(), sandbox_exec_path().as_os_str());
        let argv: Vec<_> = std_cmd.get_args().collect();
        assert!(argv.iter().any(|a| a == "-f"));
        assert!(argv.iter().any(|a| a == "--"));
    }
}
