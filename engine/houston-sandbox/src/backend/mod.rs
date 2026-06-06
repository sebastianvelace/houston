//! Platform backend selection and the [`SandboxBackend`] trait.

use crate::SandboxError;
use houston_policy::SessionPolicy;
use tokio::process::Command;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
mod linux_bwrap;
#[cfg(target_os = "linux")]
mod linux_bwrap_args;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

/// What isolation features the active backend provides.
#[derive(Debug, Clone)]
pub struct SandboxCapabilities {
    /// File system access restricted to the declared policy paths.
    pub filesystem_isolation: bool,
    /// Outbound network blocked or proxied.
    pub network_isolation: bool,
    /// Inherited file descriptors closed before exec.
    pub fd_cleanup: bool,
    /// CLI credential dirs staged outside the subprocess HOME.
    pub credential_isolation: bool,
    /// Human-readable backend identifier.
    pub platform: &'static str,
}

pub trait SandboxBackend: Send + Sync {
    /// Apply sandbox restrictions, returning a command ready to spawn.
    /// Backends that wrap the executable (bwrap, sandbox-exec) return a
    /// new `Command`; hook-only backends mutate via `pre_exec` and return
    /// the original handle.
    fn wrap_command(
        &self,
        cmd: Command,
        policy: &SessionPolicy,
    ) -> Result<Command, SandboxError>;

    /// Report what this backend actually enforces.
    fn capabilities(&self) -> SandboxCapabilities;
}

/// Select the right backend for the current platform at runtime.
pub fn detect_backend() -> Box<dyn SandboxBackend> {
    detect_backend_impl()
}

#[cfg(target_os = "linux")]
fn detect_backend_impl() -> Box<dyn SandboxBackend> {
    match std::env::var("HOUSTON_SANDBOX_BACKEND").as_deref() {
        Ok("bwrap") => Box::new(linux_bwrap::BwrapBackend),
        Ok("landlock") => Box::new(linux::LinuxBackend),
        Ok("auto") | Err(_) => linux_auto_backend(),
        Ok(other) => {
            tracing::warn!(
                backend = other,
                "unknown HOUSTON_SANDBOX_BACKEND, using auto"
            );
            linux_auto_backend()
        }
    }
}

#[cfg(target_os = "linux")]
fn linux_auto_backend() -> Box<dyn SandboxBackend> {
    if linux_bwrap::bwrap_available() {
        Box::new(linux_bwrap::BwrapBackend)
    } else {
        if linux_bwrap::which_bwrap().is_some() {
            tracing::info!(
                "HOUSTON_SANDBOX auto: using landlock backend (bwrap unusable here)"
            );
        }
        Box::new(linux::LinuxBackend)
    }
}

#[cfg(target_os = "macos")]
fn detect_backend_impl() -> Box<dyn SandboxBackend> {
    Box::new(macos::MacosBackend)
}

#[cfg(target_os = "windows")]
fn detect_backend_impl() -> Box<dyn SandboxBackend> {
    Box::new(windows::WindowsBackend)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn detect_backend_impl() -> Box<dyn SandboxBackend> {
    Box::new(PassthroughBackend)
}

/// No-op fallback for exotic targets.
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
struct PassthroughBackend;

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
impl SandboxBackend for PassthroughBackend {
    fn wrap_command(&self, cmd: Command, _policy: &SessionPolicy) -> Result<Command, SandboxError> {
        Ok(cmd)
    }

    fn capabilities(&self) -> SandboxCapabilities {
        SandboxCapabilities {
            filesystem_isolation: false,
            network_isolation: false,
            fd_cleanup: false,
            credential_isolation: true,
            platform: "unknown-passthrough",
        }
    }
}
