//! Executive manager configuration and briefing routes.

use crate::routes::error::ApiError;
use crate::state::ServerState;
use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use houston_engine_core::agents_crud;
use houston_engine_core::roles::{
    ensure_executive_agent, run_executive_briefing, validate_connected_agents,
    write_validated_executive_config, ExecutiveBriefingParams,
};
use houston_engine_core::workspaces;
use houston_engine_core::CoreError;
use houston_engine_protocol::ExecutiveConfig;
use houston_ui_events::HoustonEvent;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

pub fn router() -> Router<Arc<ServerState>> {
    Router::new()
        .route(
            "/workspaces/:workspace_id/executive-config",
            get(get_executive_config).put(put_executive_config),
        )
        .route(
            "/workspaces/:workspace_id/executive/briefing",
            post(executive_briefing),
        )
}

fn resolve_workspace_dir(st: &ServerState, workspace_id: &str) -> Result<PathBuf, CoreError> {
    let root = st.engine.paths.docs();
    let all = workspaces::read_all(root)?;
    let ws = all
        .into_iter()
        .find(|w| w.id == workspace_id)
        .ok_or_else(|| CoreError::NotFound(format!("workspace {workspace_id}")))?;
    Ok(root.join(ws.name))
}

fn resolve_agent_dir(
    root: &std::path::Path,
    workspace_id: &str,
    agent_name: &str,
) -> Result<PathBuf, CoreError> {
    let agents = agents_crud::list(root, workspace_id)?;
    agents
        .into_iter()
        .find(|a| a.name == agent_name)
        .map(|a| PathBuf::from(a.folder_path))
        .ok_or_else(|| CoreError::NotFound(format!("agent {agent_name}")))
}

async fn get_executive_config(
    State(st): State<Arc<ServerState>>,
    Path(workspace_id): Path<String>,
) -> Result<Json<ExecutiveConfig>, ApiError> {
    let root = st.engine.paths.docs();
    let ws_dir = resolve_workspace_dir(&st, &workspace_id)?;
    let config =
        ensure_executive_agent(root, &workspace_id, &ws_dir).map_err(CoreError::from)?;
    Ok(Json(config))
}

async fn put_executive_config(
    State(st): State<Arc<ServerState>>,
    Path(workspace_id): Path<String>,
    Json(body): Json<ExecutiveConfig>,
) -> Result<Json<ExecutiveConfig>, ApiError> {
    let root = st.engine.paths.docs();
    let ws_dir = resolve_workspace_dir(&st, &workspace_id)?;
    write_validated_executive_config(root, &workspace_id, &ws_dir, &body)?;
    Ok(Json(body))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExecutiveBriefingRequest {
    pub prompt: String,
    pub session_key: String,
    #[serde(default)]
    pub busy_wait_timeout_secs: Option<u64>,
    #[serde(default)]
    pub sync_session_timeout_secs: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExecutiveBriefingResponse {
    pub session_key: String,
}

async fn executive_briefing(
    State(st): State<Arc<ServerState>>,
    Path(workspace_id): Path<String>,
    Json(req): Json<ExecutiveBriefingRequest>,
) -> Result<Json<ExecutiveBriefingResponse>, ApiError> {
    let root = st.engine.paths.docs().to_path_buf();
    let ws_dir = resolve_workspace_dir(&st, &workspace_id)?;
    let config = ensure_executive_agent(&root, &workspace_id, &ws_dir).map_err(CoreError::from)?;
    validate_connected_agents(&root, &workspace_id, &config.connected_agents)?;

    let executive_dir = resolve_agent_dir(&root, &workspace_id, &config.executive_agent)?;
    let executive_path = executive_dir.to_string_lossy().to_string();
    let session_key = req.session_key.clone();
    let params = ExecutiveBriefingParams {
        executive_agent_dir: executive_dir,
        executive_agent_name: config.executive_agent.clone(),
        connected_agents: config.connected_agents.clone(),
        prompt: req.prompt,
        session_key: req.session_key.clone(),
        busy_wait_timeout: req.busy_wait_timeout_secs.map(Duration::from_secs),
        sync_session_timeout: req.sync_session_timeout_secs.map(Duration::from_secs),
    };

    let engine = st.engine.clone();
    let events = st.engine.events.clone();
    let workspace_id_spawn = workspace_id.clone();
    let workspaces_root = root.clone();
    tokio::spawn(async move {
        if let Err(e) = run_executive_briefing(
            &engine,
            events.clone(),
            &workspaces_root,
            &workspace_id_spawn,
            params,
        )
        .await
        {
            tracing::warn!("[executive] briefing failed: {e}");
            events.emit(HoustonEvent::SessionStatus {
                agent_path: executive_path,
                session_key,
                status: "error".into(),
                error: Some(e.to_string()),
            });
        }
    });

    Ok(Json(ExecutiveBriefingResponse {
        session_key: req.session_key,
    }))
}
