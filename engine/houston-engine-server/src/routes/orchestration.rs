//! Workspace roles and agent orchestration routes.

use crate::routes::error::ApiError;
use crate::state::ServerState;
use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use houston_agent_files::{read_workspace_roles, write_workspace_roles};
use houston_engine_core::agents_crud;
use houston_engine_core::roles::{run_orchestrated_procedure, OrchestrationParams, RoleResolver};
use houston_engine_core::workspaces;
use houston_engine_core::CoreError;
use houston_engine_protocol::WorkspaceRoles;
use houston_ui_events::HoustonEvent;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

pub fn router() -> Router<Arc<ServerState>> {
    Router::new()
        .route("/workspaces/:workspace_id/roles", get(get_roles).put(put_roles))
        .route(
            "/workspaces/:workspace_id/agents/:agent_name/orchestrate",
            post(orchestrate),
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

async fn get_roles(
    State(st): State<Arc<ServerState>>,
    Path(workspace_id): Path<String>,
) -> Result<Json<WorkspaceRoles>, ApiError> {
    let ws_dir = resolve_workspace_dir(&st, &workspace_id)?;
    let roles = read_workspace_roles(&ws_dir).map_err(map_roles_error)?;
    Ok(Json(roles))
}

async fn put_roles(
    State(st): State<Arc<ServerState>>,
    Path(workspace_id): Path<String>,
    Json(body): Json<WorkspaceRoles>,
) -> Result<Json<WorkspaceRoles>, ApiError> {
    let ws_dir = resolve_workspace_dir(&st, &workspace_id)?;
    write_workspace_roles(&ws_dir, &body).map_err(map_roles_error)?;
    Ok(Json(body))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OrchestrateRequest {
    pub procedure_id: String,
    #[serde(default)]
    pub prompt: Option<String>,
    pub session_key: String,
    #[serde(default)]
    pub busy_wait_timeout_secs: Option<u64>,
    #[serde(default)]
    pub sync_session_timeout_secs: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OrchestrateResponse {
    pub session_key: String,
}

async fn orchestrate(
    State(st): State<Arc<ServerState>>,
    Path((workspace_id, agent_name)): Path<(String, String)>,
    Json(req): Json<OrchestrateRequest>,
) -> Result<Json<OrchestrateResponse>, ApiError> {
    let ws_dir = resolve_workspace_dir(&st, &workspace_id)?;
    let agent_dir = resolve_agent_dir(st.engine.paths.docs(), &workspace_id, &agent_name)?;

    let resolver = RoleResolver::load(&ws_dir).map_err(CoreError::from)?;
    resolver
        .resolve_procedure(&agent_name, &req.procedure_id)
        .map_err(CoreError::from)?;

    let agent_path = agent_dir.to_string_lossy().to_string();
    let session_key = req.session_key.clone();
    let params = OrchestrationParams {
        workspace_path: ws_dir,
        orchestrator_agent_dir: agent_dir,
        orchestrator_agent_name: agent_name,
        procedure_id: req.procedure_id,
        session_key: req.session_key.clone(),
        user_prompt: req.prompt,
        busy_wait_timeout: req.busy_wait_timeout_secs.map(Duration::from_secs),
        sync_session_timeout: req.sync_session_timeout_secs.map(Duration::from_secs),
    };

    let engine = st.engine.clone();
    let events = st.engine.events.clone();
    tokio::spawn(async move {
        if let Err(e) = run_orchestrated_procedure(&engine, events.clone(), params).await {
            tracing::warn!("[orchestration] procedure failed: {e}");
            events.emit(HoustonEvent::SessionStatus {
                agent_path: agent_path.clone(),
                session_key: session_key.clone(),
                status: "error".into(),
                error: Some(e.to_string()),
            });
        }
    });

    Ok(Json(OrchestrateResponse {
        session_key: req.session_key,
    }))
}

fn map_roles_error(err: houston_agent_files::AgentFilesError) -> CoreError {
    CoreError::BadRequest(err.to_string())
}

