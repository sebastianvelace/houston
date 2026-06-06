//! OS-level process isolation for Houston agent CLI subprocesses.
//!
//! ## What this crate does
//!
//! Before spawning Claude/Codex/Gemini, the engine calls
//! [`configure_sandbox`] on the prepared `tokio::process::Command`.
//! This installs a `pre_exec` hook that runs in the forked child process
//! immediately before it is replaced by `exec()`:
//!
//! 1. **Close inherited file descriptors** (Unix): the engine's open
//!    sockets, DB connections, and log handles are closed so the CLI
//!    subprocess cannot read from or write to them.
//!
//! 2. **Landlock FS rules** (Linux ≥ 5.13): restricts filesystem access
//!    to the agent's working directory (read/write) and standard system
//!    paths (read-only). On older kernels the rule is silently skipped.
//!
//! macOS and Windows backends are stubbed — they emit a capabilities
//! report but apply no OS-level restrictions today (full backends tracked
//! as post-hackathon work).
//!
//! ## Design
//!
//! All platform work lives behind the [`SandboxBackend`] trait. The
//! public [`configure_sandbox`] function calls [`detect_backend`] once,
//! then delegates to the platform backend. Each backend has unit tests
//! that run on the CI platform where the backend is active.

mod backend;

pub use backend::{detect_backend, SandboxBackend, SandboxCapabilities};
use houston_policy::SessionPolicy;
use tokio::process::Command;

/// Apply sandbox restrictions to `cmd` based on `policy`.
///
/// This adds a `pre_exec` hook to the command. The hook runs in the
/// forked child process after `fork()` but before `exec()`. It is safe
/// to call multiple times — each call appends a new hook; hooks are
/// executed in insertion order.
///
/// On platforms where no backend is active (Windows, macOS in this
/// release) this is a no-op.
pub fn configure_sandbox(cmd: &mut Command, policy: &SessionPolicy) {
    detect_backend().configure(cmd, policy);
}

/// Report what isolation features are available on this platform/kernel.
pub fn capabilities() -> SandboxCapabilities {
    detect_backend().capabilities()
}
