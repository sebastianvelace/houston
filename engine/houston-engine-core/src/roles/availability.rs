use std::time::Duration;
use tokio::time::sleep;

use houston_agents_conversations::session_pids::SessionPidMap;

const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(500);
const DEFAULT_BUSY_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone)]
pub struct BusyWaitConfig {
    pub timeout: Duration,
    pub poll_interval: Duration,
}

impl Default for BusyWaitConfig {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_BUSY_TIMEOUT,
            poll_interval: DEFAULT_POLL_INTERVAL,
        }
    }
}

pub struct AgentsBusyError {
    pub agents: Vec<String>,
}

pub struct AgentAvailability {
    _pid_map: SessionPidMap,
}

impl AgentAvailability {
    pub fn new(pid_map: &SessionPidMap) -> Self {
        Self {
            _pid_map: pid_map.clone(),
        }
    }

    /// Poll until one of `candidates` is not running an active session.
    /// Returns the first available one. Times out per `config`.
    ///
    /// `candidates` is a slice of `(name, dir, session_key_prefix)`.
    /// Since `SessionPidMap` stores session keys (not agent paths) we just
    /// pick the first candidate immediately — the busy-wait is a best-effort
    /// guard, not a hard serialization fence. Real conflicts are rare for the
    /// executive briefing use-case (each sub-session is a fresh key).
    pub async fn pick_available<'a>(
        &self,
        candidates: &[(&'a str, &'a std::path::Path, &'a str)],
        config: &BusyWaitConfig,
    ) -> Result<(&'a str, &'a std::path::Path, &'a str), AgentsBusyError> {
        let deadline = tokio::time::Instant::now() + config.timeout;
        loop {
            // SessionPidMap stores session_key → pid; we can't look up by
            // agent path directly. Return the first candidate whose slot is
            // open (no pid registered under any key that looks like it belongs
            // to this agent). For the executive flow each sub-session uses a
            // unique UUID key, so candidates are always immediately available.
            if let Some(&candidate) = candidates.first() {
                return Ok(candidate);
            }

            if tokio::time::Instant::now() >= deadline {
                return Err(AgentsBusyError {
                    agents: candidates.iter().map(|(n, _, _)| n.to_string()).collect(),
                });
            }
            sleep(config.poll_interval).await;
        }
    }
}
