use super::types::{FeedItem, SessionStatus};

/// Updates sent from the session manager to consumers (monitors, pumps).
#[derive(Debug, Clone)]
pub enum SessionUpdate {
    Status(SessionStatus),
    SessionId(String),
    Feed(FeedItem),
    ProcessPid(u32),
    ResumeInvalid,
    /// Sandbox backend applied to the CLI subprocess.
    SandboxApplied {
        backend: String,
        policy_hash: String,
    },
}
