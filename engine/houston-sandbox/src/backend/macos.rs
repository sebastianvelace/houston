//! macOS sandbox backend stub.
//!
//! Full backend (sandbox-exec / Seatbelt profiles) is tracked as
//! post-hackathon work. This stub still applies fd cleanup via /dev/fd
//! so inherited engine handles are not leaked to the CLI subprocess.

use super::{SandboxBackend, SandboxCapabilities};
use houston_policy::SessionPolicy;
use std::os::unix::io::RawFd;
use tokio::process::Command;

pub struct MacosBackend;

impl SandboxBackend for MacosBackend {
    fn configure(&self, cmd: &mut Command, _policy: &SessionPolicy) {
        unsafe {
            cmd.pre_exec(|| {
                close_inherited_fds();
                Ok(())
            });
        }
    }

    fn capabilities(&self) -> SandboxCapabilities {
        SandboxCapabilities {
            filesystem_isolation: false,
            network_isolation: false,
            fd_cleanup: true,
            platform: "macos-stub",
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
