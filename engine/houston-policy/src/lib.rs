//! Session isolation policy.
//!
//! `SessionPolicy` describes what filesystem paths and network access
//! an agent subprocess may use. It is constructed by the engine before
//! spawning any CLI and consumed by `houston-sandbox` to enforce the
//! restrictions at the OS level.
//!
//! Kept as a pure-data crate (no OS calls, no async) so the policy
//! can be unit-tested on any platform without special setup.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// What outbound network access the subprocess may use.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EgressPolicy {
    /// No restriction — all network traffic allowed. Default in v1.
    #[default]
    Full,
    /// Block all outbound network. Useful for pure-filesystem agents.
    Deny,
    /// Allow only specific domains via a local proxy (post-hackathon).
    Allowlist(Vec<String>),
}

/// Isolation policy for one CLI subprocess session.
///
/// The sandbox backend reads this to configure OS-level restrictions
/// (Landlock on Linux, sandbox-exec on macOS, Job Object on Windows).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionPolicy {
    /// The directory the subprocess is allowed to read AND write.
    /// Typically the agent's working directory (project folder).
    pub working_dir: PathBuf,
    /// Additional paths that are read-only for the subprocess.
    /// The sandbox backend always adds standard system paths (/usr, /lib…).
    pub extra_ro_paths: Vec<PathBuf>,
    /// Network egress behaviour.
    pub egress: EgressPolicy,
}

impl SessionPolicy {
    /// Minimal policy: read/write access to `working_dir`, full network.
    pub fn for_working_dir(working_dir: PathBuf) -> Self {
        Self {
            working_dir,
            extra_ro_paths: Vec::new(),
            egress: EgressPolicy::Full,
        }
    }

    /// Add an extra read-only path (e.g. a shared credentials directory).
    pub fn with_ro_path(mut self, path: PathBuf) -> Self {
        self.extra_ro_paths.push(path);
        self
    }

    /// Override the egress policy.
    pub fn with_egress(mut self, egress: EgressPolicy) -> Self {
        self.egress = egress;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn for_working_dir_sets_correct_defaults() {
        let p = SessionPolicy::for_working_dir(PathBuf::from("/tmp/agent"));
        assert_eq!(p.working_dir, PathBuf::from("/tmp/agent"));
        assert!(p.extra_ro_paths.is_empty());
        assert!(matches!(p.egress, EgressPolicy::Full));
    }

    #[test]
    fn builder_methods_chain() {
        let p = SessionPolicy::for_working_dir(PathBuf::from("/tmp/agent"))
            .with_ro_path(PathBuf::from("/etc"))
            .with_egress(EgressPolicy::Deny);
        assert_eq!(p.extra_ro_paths.len(), 1);
        assert!(matches!(p.egress, EgressPolicy::Deny));
    }

    #[test]
    fn serializes_and_deserializes() {
        let p = SessionPolicy::for_working_dir(PathBuf::from("/tmp/agent"))
            .with_egress(EgressPolicy::Allowlist(vec!["api.anthropic.com".into()]));
        let json = serde_json::to_string(&p).unwrap();
        let p2: SessionPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(p.working_dir, p2.working_dir);
        assert!(matches!(p2.egress, EgressPolicy::Allowlist(_)));
    }
}
