use super::availability::{AgentAvailability, AgentsBusyError, BusyWaitConfig};
use super::sync_session::{run_sync_session, SyncSessionError};
use crate::agents_crud::{self, CreateAgent};
use crate::sessions::{self, StartParams};
use crate::state::EngineState;
use crate::CoreError;
use houston_agent_files::{read_executive_config, write_executive_config};
use houston_engine_protocol::{ErrorCode, ExecutiveConfig};
use houston_ui_events::{DynEventSink, HoustonEvent};
use std::path::Path;
use std::time::Duration;
use thiserror::Error;

const DEFAULT_BUSY_TIMEOUT: Duration = Duration::from_secs(60);
const DEFAULT_EXECUTIVE_CLAUDE_MD: &str =
    "## Instructions\n\nYou are the executive assistant for the business owner. You aggregate updates from team agents and present a clear, actionable briefing.\n\n## Learnings\n";

#[derive(Debug, Error)]
pub enum ExecutiveError {
    #[error("executive config error: {0}")]
    Config(String),
    #[error("connected agent not found: {0}")]
    AgentNotFound(String),
    #[error("agent {agent_name} is busy")]
    AgentBusy { agent_name: String },
    #[error(transparent)]
    SyncSession(#[from] SyncSessionError),
    #[error("executive briefing session failed: {0}")]
    Session(#[from] CoreError),
}

impl From<ExecutiveError> for CoreError {
    fn from(err: ExecutiveError) -> Self {
        match err {
            ExecutiveError::AgentBusy { agent_name } => CoreError::Labeled {
                code: ErrorCode::Unavailable,
                kind: "agent_busy",
                message: format!("Agent '{agent_name}' is busy"),
            },
            ExecutiveError::AgentNotFound(name) => CoreError::NotFound(format!("agent {name}")),
            ExecutiveError::Config(msg) => CoreError::BadRequest(msg),
            ExecutiveError::SyncSession(e) => CoreError::Labeled {
                code: ErrorCode::Unavailable,
                kind: "sync_session_failed",
                message: e.to_string(),
            },
            ExecutiveError::Session(e) => e,
        }
    }
}

pub fn build_briefing_prompt(user_prompt: &str) -> String {
    format!(
        "Proporciona un resumen actualizado para el dueño del negocio sobre tu área. \
         Consulta: {user_prompt}. Responde solo con datos relevantes."
    )
}

pub fn build_executive_enriched_prompt(user_prompt: &str, responses: &[(String, String)]) -> String {
    let mut parts = Vec::new();
    for (agent, text) in responses {
        parts.push(format!("[{agent}]\n{text}"));
    }
    parts.push(format!(
        "El dueño del negocio pregunta: {user_prompt}\n\nSintetiza las respuestas anteriores en un briefing ejecutivo claro y accionable."
    ));
    parts.join("\n\n")
}

fn map_config_error(err: houston_agent_files::AgentFilesError) -> ExecutiveError {
    ExecutiveError::Config(err.to_string())
}

pub fn ensure_executive_agent(
    root: &Path,
    workspace_id: &str,
    ws_dir: &Path,
) -> Result<ExecutiveConfig, ExecutiveError> {
    let config = read_executive_config(ws_dir).map_err(map_config_error)?;
    ensure_executive_agent_named(root, workspace_id, ws_dir, &config.executive_agent)
        .map_err(ExecutiveError::Session)?;
    Ok(config)
}

pub fn ensure_executive_agents_for_all_workspaces(
    root: &Path,
    workspaces_root: &Path,
) -> Result<(), CoreError> {
    let workspaces = crate::workspaces::list(workspaces_root)?;
    for ws in workspaces {
        let ws_dir = workspaces_root.join(&ws.name);
        if let Err(e) = ensure_executive_agent(root, &ws.id, &ws_dir) {
            tracing::warn!(
                workspace = %ws.name,
                error = %e,
                "[boot] failed to ensure executive agent"
            );
        }
    }
    Ok(())
}

pub struct ExecutiveBriefingParams {
    pub executive_agent_dir: std::path::PathBuf,
    pub executive_agent_name: String,
    pub connected_agents: Vec<String>,
    pub prompt: String,
    pub session_key: String,
    pub busy_wait_timeout: Option<Duration>,
    pub sync_session_timeout: Option<Duration>,
}

pub async fn run_executive_briefing(
    engine: &EngineState,
    events: DynEventSink,
    root: &Path,
    workspace_id: &str,
    params: ExecutiveBriefingParams,
) -> Result<(), ExecutiveError> {
    let executive_path = params.executive_agent_dir.to_string_lossy().to_string();
    events.emit(HoustonEvent::OrchestrationProcedureStarted {
        agent_path: executive_path.clone(),
        procedure_id: "executive_briefing".into(),
    });

    let busy_config = BusyWaitConfig {
        timeout: params.busy_wait_timeout.unwrap_or(DEFAULT_BUSY_TIMEOUT),
        ..BusyWaitConfig::default()
    };

    let mut handles = Vec::new();
    for agent_name in &params.connected_agents {
        let engine = engine.clone();
        let events = events.clone();
        let agent_name = agent_name.clone();
        let prompt = build_briefing_prompt(&params.prompt);
        let sync_timeout = params.sync_session_timeout;
        let busy_config = busy_config.clone();
        let workspaces_root = root.to_path_buf();
        let workspace_id = workspace_id.to_string();

        handles.push(tokio::spawn(async move {
            let agents = agents_crud::list(&workspaces_root, &workspace_id)
                .map_err(ExecutiveError::Session)?;
            let agent = agents
                .into_iter()
                .find(|a| a.name == agent_name)
                .ok_or_else(|| ExecutiveError::AgentNotFound(agent_name.clone()))?;
            let agent_dir = std::path::PathBuf::from(&agent.folder_path);
            let agent_path = agent.folder_path.clone();
            let candidates = [(
                agent_name.as_str(),
                agent_dir.as_path(),
                agent_path.as_str(),
            )];
            let availability = AgentAvailability::new(&engine.sessions.pid_map);
            let (_name, dir, _path) = availability
                .pick_available(&candidates, &busy_config)
                .await
                .map_err(|AgentsBusyError { agents }| ExecutiveError::AgentBusy {
                    agent_name: agents.first().cloned().unwrap_or_else(|| agent_name.clone()),
                })?;

            let text =
                run_sync_session(&engine, events, dir, &agent_name, &prompt, sync_timeout).await?;
            Ok::<_, ExecutiveError>((agent_name, text))
        }));
    }

    let mut responses = Vec::new();
    for handle in handles {
        responses.push(
            handle
                .await
                .map_err(|e| ExecutiveError::Session(CoreError::Internal(e.to_string())))??,
        );
    }
    responses.sort_by(|a, b| a.0.cmp(&b.0));

    let enriched = build_executive_enriched_prompt(&params.prompt, &responses);
    let provider = sessions::resolve_provider(&params.executive_agent_dir);
    let start = StartParams {
        agent_dir: params.executive_agent_dir.clone(),
        working_dir: params.executive_agent_dir,
        session_key: params.session_key,
        prompt: enriched,
        system_prompt: None,
        source: Some("executive_briefing".into()),
        provider: provider.provider,
        model: provider.model,
        effort: None,
        compact: false,
    };
    sessions::start(
        &engine.sessions,
        events,
        engine.db.clone(),
        &engine.app_system_prompt,
        start,
    )
    .await
    .map(|_| ())
    .map_err(ExecutiveError::Session)
}

pub fn validate_connected_agents(
    root: &Path,
    workspace_id: &str,
    connected_agents: &[String],
) -> Result<(), CoreError> {
    let agents = agents_crud::list(root, workspace_id)?;
    let names: std::collections::HashSet<_> = agents.iter().map(|a| a.name.as_str()).collect();
    for name in connected_agents {
        if !names.contains(name.as_str()) {
            return Err(CoreError::BadRequest(format!(
                "connected agent '{name}' does not exist in workspace"
            )));
        }
    }
    Ok(())
}

pub fn ensure_executive_agent_named(
    root: &Path,
    workspace_id: &str,
    ws_dir: &Path,
    executive_agent: &str,
) -> Result<(), CoreError> {
    let agent_dir = ws_dir.join(executive_agent);
    if agent_dir.is_dir() {
        return Ok(());
    }
    agents_crud::create(
        root,
        workspace_id,
        CreateAgent {
            name: executive_agent.into(),
            config_id: "blank".into(),
            color: None,
            claude_md: Some(DEFAULT_EXECUTIVE_CLAUDE_MD.into()),
            installed_path: None,
            seeds: None,
            existing_path: None,
        },
    )?;
    Ok(())
}

pub fn write_validated_executive_config(
    root: &Path,
    workspace_id: &str,
    ws_dir: &Path,
    config: &ExecutiveConfig,
) -> Result<(), CoreError> {
    validate_connected_agents(root, workspace_id, &config.connected_agents)?;
    ensure_executive_agent_named(root, workspace_id, ws_dir, &config.executive_agent)?;
    write_executive_config(ws_dir, config).map_err(|e| CoreError::BadRequest(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn briefing_prompt_includes_user_query() {
        let prompt = build_briefing_prompt("¿Cómo van las ventas?");
        assert!(prompt.contains("¿Cómo van las ventas?"));
        assert!(prompt.contains("dueño del negocio"));
    }

    #[test]
    fn enriched_prompt_tags_agents_and_question() {
        let responses = vec![
            ("Contabilidad".into(), "Ingresos: 100k".into()),
            ("Marketing".into(), "Alcance: 50k".into()),
        ];
        let prompt = build_executive_enriched_prompt("Resumen del mes", &responses);
        assert!(prompt.contains("[Contabilidad]"));
        assert!(prompt.contains("Ingresos: 100k"));
        assert!(prompt.contains("[Marketing]"));
        assert!(prompt.contains("Resumen del mes"));
        assert!(prompt.contains("briefing ejecutivo"));
    }
}
