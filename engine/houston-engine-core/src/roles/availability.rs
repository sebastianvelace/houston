//! Agent busy detection and wait-for-available logic.

use crate::agents::activity;
use crate::sessions::SessionRuntime;
use crate::CoreResult;
use std::path::Path;
use std::time::Duration;
use thiserror::Error;
use tokio::time::sleep;

/// Configuration for waiting on busy provider agents.
#[derive(Debug, Clone)]
pub struct BusyWaitConfig {
    pub timeout: Duration,
    pub poll_interval: Duration,
}

impl Default for BusyWaitConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(60),
            poll_interval: Duration::from_millis(500),
        }
    }
}

/// Snapshot of whether an agent can accept a new sync session.
pub struct AgentAvailability<'a> {
    rt: &'a SessionRuntime,
}

impl<'a> AgentAvailability<'a> {
    pub fn new(rt: &'a SessionRuntime) -> Self {
        Self { rt }
    }

    pub async fn is_busy(&self, agent_dir: &Path, agent_path: &str) -> CoreResult<bool> {
        if self.rt.workdir_activity().is_active(agent_dir).await {
            return Ok(true);
        }
        if self.rt.session_control().is_agent_busy(agent_path).await {
            return Ok(true);
        }
        let activities = activity::list(agent_dir)?;
        Ok(activities.iter().any(|row| row.status == "running"))
    }

    /// Pick the first available agent from `candidates`. When all are busy,
    /// poll until one frees up or `config.timeout` elapses.
    pub async fn pick_available<'b>(
        &self,
        candidates: &'b [(&'b str, &'b Path, &'b str)],
        config: &BusyWaitConfig,
    ) -> Result<(&'b str, &'b Path, &'b str), AgentsBusyError> {
        let deadline = tokio::time::Instant::now() + config.timeout;
        loop {
            for (name, dir, path) in candidates {
                match self.is_busy(dir, path).await {
                    Ok(false) => return Ok((*name, *dir, *path)),
                    Ok(true) => continue,
                    Err(e) => {
                        tracing::warn!(
                            "[orchestration] failed to read activity for {name}: {e}"
                        );
                    }
                }
            }
            if tokio::time::Instant::now() >= deadline {
                let names: Vec<String> = candidates.iter().map(|(n, _, _)| (*n).to_string()).collect();
                return Err(AgentsBusyError { agents: names });
            }
            sleep(config.poll_interval).await;
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("all candidate agents are busy: {agents:?}")]
pub struct AgentsBusyError {
    pub agents: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::types::{ActivityUpdate, NewActivity};
    use crate::sessions::SessionIdentity;
    use tempfile::TempDir;

    #[tokio::test]
    async fn running_activity_marks_agent_busy() {
        let rt = SessionRuntime::default();
        let dir = TempDir::new().unwrap();
        let agent_path = dir.path().to_string_lossy().to_string();
        let item = activity::create(
            dir.path(),
            NewActivity {
                title: "work".into(),
                description: String::new(),
                agent: None,
                worktree_path: None,
                provider: None,
                model: None,
            },
        )
        .unwrap();
        assert_eq!(item.status, "running");

        let availability = AgentAvailability::new(&rt);
        assert!(availability.is_busy(dir.path(), &agent_path).await.unwrap());

        activity::update(
            dir.path(),
            &item.id,
            ActivityUpdate {
                status: Some("needs_you".into()),
                ..ActivityUpdate::default()
            },
        )
        .unwrap();
        assert!(!availability.is_busy(dir.path(), &agent_path).await.unwrap());
    }

    #[tokio::test]
    async fn session_control_marks_agent_busy() {
        let rt = SessionRuntime::default();
        let dir = TempDir::new().unwrap();
        let agent_path = dir.path().to_string_lossy().to_string();
        let identity = SessionIdentity::new(agent_path.clone(), "chat-1".into());
        rt.session_control().register(&identity).await;

        let availability = AgentAvailability::new(&rt);
        assert!(availability.is_busy(dir.path(), &agent_path).await.unwrap());
    }
}
