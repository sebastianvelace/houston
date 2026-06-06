//! Session isolation policy.
//!
//! `SessionPolicy` describes what filesystem paths and network access
//! an agent subprocess may use. It is constructed by the engine before
//! spawning any CLI and consumed by `houston-sandbox` to enforce the
//! restrictions at the OS level.

mod denylist;

use denylist::{build_denied_prefixes, infer_workspace_root, path_within_root};
pub use denylist::{houston_data_root, workspaces_root};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionPolicy {
    /// Directory the subprocess may read and write (typically `agent_root`).
    pub working_dir: PathBuf,
    /// Additional read-only paths for the subprocess.
    pub extra_ro_paths: Vec<PathBuf>,
    /// Paths explicitly denied (sibling agents, credentials, workspaces root).
    pub denied_prefixes: Vec<PathBuf>,
    /// Network egress behaviour.
    pub egress: EgressPolicy,
}

impl SessionPolicy {
    /// Policy for one agent: RW on `agent_root`, denylist for cross-agent paths.
    ///
    /// When `workspace_root` is `None`, it is inferred from `agent_root` when
    /// the path lives under `~/.houston/workspaces/{Ws}/{Agent}`.
    pub fn for_working_dir(agent_root: PathBuf, workspace_root: Option<PathBuf>) -> Self {
        let inferred = infer_workspace_root(&agent_root);
        let ws = workspace_root
            .as_deref()
            .or(inferred.as_deref());
        let denied_prefixes = build_denied_prefixes(&agent_root, ws);
        Self {
            working_dir: agent_root,
            extra_ro_paths: Vec::new(),
            denied_prefixes,
            egress: EgressPolicy::Full,
        }
    }

    /// True when `path` resolves under this policy's `working_dir`.
    pub fn allows_path(&self, path: &Path) -> bool {
        path_within_root(path, &self.working_dir)
    }

    pub fn with_ro_path(mut self, path: PathBuf) -> Self {
        self.extra_ro_paths.push(path);
        self
    }

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
        let p = SessionPolicy::for_working_dir(PathBuf::from("/tmp/agent"), None);
        assert_eq!(p.working_dir, PathBuf::from("/tmp/agent"));
        assert!(p.extra_ro_paths.is_empty());
        assert!(matches!(p.egress, EgressPolicy::Full));
    }

    #[test]
    fn builder_methods_chain() {
        let p = SessionPolicy::for_working_dir(PathBuf::from("/tmp/agent"), None)
            .with_ro_path(PathBuf::from("/etc"))
            .with_egress(EgressPolicy::Deny);
        assert_eq!(p.extra_ro_paths.len(), 1);
        assert!(matches!(p.egress, EgressPolicy::Deny));
    }

    #[test]
    fn serializes_and_deserializes() {
        let p = SessionPolicy::for_working_dir(PathBuf::from("/tmp/agent"), None)
            .with_egress(EgressPolicy::Allowlist(vec!["api.anthropic.com".into()]));
        let json = serde_json::to_string(&p).unwrap();
        let p2: SessionPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(p.working_dir, p2.working_dir);
        assert!(matches!(p2.egress, EgressPolicy::Allowlist(_)));
    }
}
