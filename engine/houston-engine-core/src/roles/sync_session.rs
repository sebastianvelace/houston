use crate::state::EngineState;
use houston_agents_conversations::session_runner;
use houston_terminal_manager::{FeedItem, ProviderError};
use houston_ui_events::{DynEventSink, EventSink, HoustonEvent};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use thiserror::Error;
use uuid::Uuid;

const DEFAULT_SYNC_TIMEOUT: Duration = Duration::from_secs(120);
// Fallback retry delay when the API doesn't provide a reset hint.
const RATE_LIMIT_RETRY_DELAY_SECS: u32 = 30;

/// Captures `RateLimited` feed events so the caller can retry after the
/// indicated delay instead of surfacing a hard failure to the user.
struct RateLimitCapture {
    retry_after_secs: Mutex<Option<u32>>,
}

impl RateLimitCapture {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            retry_after_secs: Mutex::new(None),
        })
    }

    fn rate_limit_delay(&self) -> Option<u32> {
        *self.retry_after_secs.lock().unwrap()
    }
}

impl EventSink for RateLimitCapture {
    fn emit(&self, event: HoustonEvent) {
        if let HoustonEvent::FeedItem {
            item: FeedItem::ProviderError(ProviderError::RateLimited { retry_after_seconds, .. }),
            ..
        } = event
        {
            let delay = retry_after_seconds.unwrap_or(RATE_LIMIT_RETRY_DELAY_SECS);
            *self.retry_after_secs.lock().unwrap() = Some(delay);
        }
    }
}

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

    events.emit(HoustonEvent::OrchestrationSubSessionStarted {
        agent_path: agent_dir.to_string_lossy().to_string(),
        provides_id: provides_id.to_string(),
    });

    match run_sync_session_attempt(engine, agent_dir, provides_id, prompt, timeout).await {
        Ok(text) => {
            events.emit(HoustonEvent::OrchestrationSubSessionCompleted {
                agent_path: agent_dir.to_string_lossy().to_string(),
                provides_id: provides_id.to_string(),
                success: true,
                error: None,
            });
            Ok(text)
        }
        Err(RetryableError::RateLimited { delay_secs, original }) => {
            tracing::warn!(
                provides_id,
                delay_secs,
                "[sync_session] rate-limited — retrying after {}s",
                delay_secs
            );
            tokio::time::sleep(Duration::from_secs(delay_secs as u64)).await;
            match run_sync_session_attempt(engine, agent_dir, provides_id, prompt, timeout).await {
                Ok(text) => {
                    events.emit(HoustonEvent::OrchestrationSubSessionCompleted {
                        agent_path: agent_dir.to_string_lossy().to_string(),
                        provides_id: provides_id.to_string(),
                        success: true,
                        error: None,
                    });
                    Ok(text)
                }
                Err(e) => {
                    let err = e.into_sync_error().unwrap_or(original);
                    events.emit(HoustonEvent::OrchestrationSubSessionCompleted {
                        agent_path: agent_dir.to_string_lossy().to_string(),
                        provides_id: provides_id.to_string(),
                        success: false,
                        error: Some(err.to_string()),
                    });
                    Err(err)
                }
            }
        }
        Err(e) => {
            let err = e.into_sync_error().unwrap_or(SyncSessionError::Failed {
                message: "unknown error".into(),
            });
            events.emit(HoustonEvent::OrchestrationSubSessionCompleted {
                agent_path: agent_dir.to_string_lossy().to_string(),
                provides_id: provides_id.to_string(),
                success: false,
                error: Some(err.to_string()),
            });
            Err(err)
        }
    }
}

enum RetryableError {
    RateLimited {
        delay_secs: u32,
        original: SyncSessionError,
    },
    Terminal(SyncSessionError),
}

impl RetryableError {
    fn into_sync_error(self) -> Option<SyncSessionError> {
        match self {
            Self::Terminal(e) => Some(e),
            Self::RateLimited { original, .. } => Some(original),
        }
    }
}

async fn run_sync_session_attempt(
    engine: &EngineState,
    agent_dir: &Path,
    _provides_id: &str,
    prompt: &str,
    timeout: Duration,
) -> Result<String, RetryableError> {
    let agent_path = agent_dir.to_string_lossy().to_string();
    let session_key = format!("orch-sync-{}", Uuid::new_v4());
    let resolved = crate::sessions::resolve_provider(agent_dir);

    let capture = RateLimitCapture::new();
    let handle = session_runner::spawn_and_monitor(
        capture.clone() as DynEventSink,
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
            return Err(RetryableError::Terminal(SyncSessionError::Failed {
                message: join_err.to_string(),
            }))
        }
        Err(_) => {
            return Err(RetryableError::Terminal(SyncSessionError::Timeout {
                secs: timeout.as_secs(),
            }))
        }
    };

    if let Some(err) = result.error {
        let sync_err = SyncSessionError::Failed {
            message: err.clone(),
        };
        if let Some(delay_secs) = capture.rate_limit_delay() {
            return Err(RetryableError::RateLimited {
                delay_secs,
                original: sync_err,
            });
        }
        return Err(RetryableError::Terminal(sync_err));
    }

    result
        .response_text
        .filter(|t| !t.trim().is_empty())
        .ok_or(RetryableError::Terminal(SyncSessionError::EmptyResponse))
}
