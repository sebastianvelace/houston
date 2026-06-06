//! Platform backend selection and the [`SandboxBackend`] trait.

use houston_policy::SessionPolicy;
use tokio::process::Command;

#[cfg(target_os = "linux")]
mod linux;
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
    /// Human-readable backend identifier.
    pub platform: &'static str,
}

pub trait SandboxBackend: Send + Sync {
    /// Apply sandbox restrictions to `cmd`. Called once per session spawn.
    fn configure(&self, cmd: &mut Command, policy: &SessionPolicy);
    /// Report what this backend actually enforces.
    fn capabilities(&self) -> SandboxCapabilities;
}

/// Select the right backend for the current platform at runtime.
pub fn detect_backend() -> Box<dyn SandboxBackend> {
    detect_backend_impl()
}

#[cfg(target_os = "linux")]
fn detect_backend_impl() -> Box<dyn SandboxBackend> {
    Box::new(linux::LinuxBackend)
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
    fn configure(&self, _cmd: &mut Command, _policy: &SessionPolicy) {}
    fn capabilities(&self) -> SandboxCapabilities {
        SandboxCapabilities {
            filesystem_isolation: false,
            network_isolation: false,
            fd_cleanup: false,
            platform: "unknown-passthrough",
        }
    }
}
