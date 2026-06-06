//! Shared sandbox wrap + lifecycle event emission for CLI runners.

use crate::session_update::SessionUpdate;
use houston_policy::SessionPolicy;
use houston_sandbox::{capabilities, wrap_sandbox, SandboxError};
use tokio::process::Command;
use tokio::sync::mpsc;

pub fn policy_hash(policy: &SessionPolicy) -> String {
    let json = serde_json::to_string(policy).unwrap_or_default();
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    json.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Wrap `cmd` with the active sandbox backend and emit `SandboxApplied`.
/// Returns `None` when sandbox setup fails (error already sent on `tx`).
pub fn apply_session_sandbox(
    tx: &mpsc::UnboundedSender<SessionUpdate>,
    cmd: Command,
    policy: &SessionPolicy,
) -> Option<Command> {
    let backend = capabilities().platform.to_string();
    let hash = policy_hash(policy);
    match wrap_sandbox(cmd, policy) {
        Ok(wrapped) => {
            let _ = tx.send(SessionUpdate::SandboxApplied {
                backend,
                policy_hash: hash,
            });
            Some(wrapped)
        }
        Err(SandboxError::Unsupported { platform, message }) => {
            let _ = tx.send(SessionUpdate::Status(
                crate::types::SessionStatus::Error(format!(
                    "Sandbox backend {platform} unavailable: {message}"
                )),
            ));
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn policy_hash_is_stable_for_same_policy() {
        let policy = SessionPolicy::for_working_dir(PathBuf::from("/tmp/a"), None);
        assert_eq!(policy_hash(&policy), policy_hash(&policy));
    }
}
