use crate::state::EngineState;
use houston_agents_conversations::session_runner;
use houston_ui_events::{DynEventSink, HoustonEvent, NoopEventSink};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use uuid::Uuid;

const DEFAULT_SYNC_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SyncSessionError {
    #[error("sync session timed out after {secs}s")]
    Timeout { secs: u64 },
    #[error("sync session failed: {message}")]
    Failed { message: String },
    #[error("sync session returned no assistant text")]
    EmptyResponse,
}

pub async fn run_sync_session(
    engine: &EngineState,
    events: DynEventSink,
    agent_dir: &Path,
    provides_id: &str,
    prompt: &str,
    timeout: Option<Duration>,
) -> Result<String, SyncSessionError> {
    let timeout = timeout.unwrap_or(DEFAULT_SYNC_TIMEOUT);
    let agent_path = agent_dir.to_string_lossy().to_string();
    let session_key = format!("orch-sync-{}", Uuid::new_v4());

    events.emit(HoustonEvent::OrchestrationSubSessionStarted {
        agent_path: agent_path.clone(),
        provides_id: provides_id.to_string(),
    });

    let resolved = crate::sessions::resolve_provider(agent_dir);
    let noop: DynEventSink = Arc::new(NoopEventSink);
    let handle = session_runner::spawn_and_monitor(
        noop,
        agent_path.clone(),
        session_key,
        prompt.to_string(),
        None,  // resume_id
        None,  // resume_fallback_prompt
        agent_dir.to_path_buf(),
        None,  // system_prompt
        None,  // session_id_handle
        None,  // persist
        Some(engine.sessions.pid_map.clone()),
        resolved.provider,
        resolved.model,
        None,  // effort
    );

    let result = match tokio::time::timeout(timeout, handle).await {
        Ok(Ok(result)) => result,
        Ok(Err(join_err)) => {
            events.emit(HoustonEvent::OrchestrationSubSessionCompleted {
                agent_path: agent_path.clone(),
                provides_id: provides_id.to_string(),
                success: false,
                error: Some(join_err.to_string()),
            });
            return Err(SyncSessionError::Failed {
                message: join_err.to_string(),
            });
        }
        Err(_) => {
            events.emit(HoustonEvent::OrchestrationSubSessionCompleted {
                agent_path: agent_path.clone(),
                provides_id: provides_id.to_string(),
                success: false,
                error: Some(format!("timed out after {}s", timeout.as_secs())),
            });
            return Err(SyncSessionError::Timeout {
                secs: timeout.as_secs(),
            });
        }
    };

    if let Some(err) = result.error {
        events.emit(HoustonEvent::OrchestrationSubSessionCompleted {
            agent_path: agent_path.clone(),
            provides_id: provides_id.to_string(),
            success: false,
            error: Some(err.clone()),
        });
        return Err(SyncSessionError::Failed { message: err });
    }

    let text = result
        .response_text
        .filter(|t| !t.trim().is_empty())
        .ok_or(SyncSessionError::EmptyResponse)?;

    events.emit(HoustonEvent::OrchestrationSubSessionCompleted {
        agent_path,
        provides_id: provides_id.to_string(),
        success: true,
        error: None,
    });
    Ok(text)
}
