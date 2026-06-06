//! macOS sandbox backend: closefds + Seatbelt via `sandbox_init`.

use super::super::macos_profile::{profile_inputs, render_profile};
use super::{SandboxBackend, SandboxCapabilities};
use crate::SandboxError;
use houston_policy::SessionPolicy;
use std::ffi::CString;
use std::os::unix::io::RawFd;
use tokio::process::Command;

pub struct MacosBackend;

impl SandboxBackend for MacosBackend {
    fn wrap_command(&self, mut cmd: Command, policy: &SessionPolicy) -> Result<Command, SandboxError> {
        let (real_home, houston_data) = profile_inputs(policy);
        let profile = render_profile(policy, &real_home, &houston_data);

        unsafe {
            cmd.pre_exec(move || {
                close_inherited_fds();
                if let Err(e) = apply_seatbelt(&profile) {
                    let msg = format!("[houston-sandbox] seatbelt init failed: {e}\n");
                    libc::write(2, msg.as_ptr() as *const libc::c_void, msg.len());
                }
                Ok(())
            });
        }
        Ok(cmd)
    }

    fn capabilities(&self) -> SandboxCapabilities {
        SandboxCapabilities {
            filesystem_isolation: true,
            network_isolation: false,
            fd_cleanup: true,
            credential_isolation: true,
            platform: "macos-seatbelt",
        }
    }
}

fn apply_seatbelt(profile: &str) -> Result<(), String> {
    let c_profile = CString::new(profile).map_err(|e| e.to_string())?;
    let mut error_buf: *mut libc::c_char = std::ptr::null_mut();
    let rc = unsafe { sandbox_init(c_profile.as_ptr(), SANDBOX_NAMED, &mut error_buf) };
    if rc == 0 {
        return Ok(());
    }
    let detail = if error_buf.is_null() {
        format!("sandbox_init returned {rc}")
    } else {
        unsafe {
            let msg = std::ffi::CStr::from_ptr(error_buf).to_string_lossy().into_owned();
            libc::free(error_buf as *mut libc::c_void);
            msg
        }
    };
    Err(detail)
}

const SANDBOX_NAMED: u64 = 1;

extern "C" {
    fn sandbox_init(
        profile: *const libc::c_char,
        flags: u64,
        errorbuf: *mut *mut libc::c_char,
    ) -> i32;
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
    fn wrap_command_installs_pre_exec_hook() {
        let cmd = Command::new("true");
        let policy = SessionPolicy::for_working_dir(PathBuf::from("/tmp"), None);
        MacosBackend.wrap_command(cmd, &policy).expect("wrap ok");
    }

    #[test]
    fn capabilities_report_seatbelt_backend() {
        let caps = MacosBackend.capabilities();
        assert!(caps.filesystem_isolation);
        assert_eq!(caps.platform, "macos-seatbelt");
    }
}
