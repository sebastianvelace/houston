//! Windows sandbox backend stub.
//!
//! Full backend (Job Object + restricted token) is tracked as
//! post-hackathon work. This stub is a no-op — no pre_exec hook
//! is registered because Windows process creation does not use
//! fork+exec semantics.

use super::{SandboxBackend, SandboxCapabilities};
use houston_policy::SessionPolicy;
use tokio::process::Command;

pub struct WindowsBackend;

impl SandboxBackend for WindowsBackend {
    fn configure(&self, _cmd: &mut Command, _policy: &SessionPolicy) {
        // No-op: Windows Job Object backend is not yet implemented.
        // The engine still benefits from the structural fixes
        // (workspace_context write-authorization removal, learnings filter,
        // git hooksPath env vars) on this platform.
    }

    fn capabilities(&self) -> SandboxCapabilities {
        SandboxCapabilities {
            filesystem_isolation: false,
            network_isolation: false,
            fd_cleanup: false,
            platform: "windows-stub",
        }
    }
}
