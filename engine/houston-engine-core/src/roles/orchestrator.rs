//! Orchestration coordinator: gather context, then run the main procedure session.

use super::availability::{AgentAvailability, AgentsBusyError, BusyWaitConfig};
use super::resolver::{RoleResolver, RoleResolverError};
use super::sync_session::{build_provision_prompt, run_sync_session, SyncSessionError};
use crate::sessions::{self, StartParams};
use crate::state::EngineState;
use crate::CoreError;
use houston_engine_protocol::{ErrorCode, Procedure};
use houston_ui_events::{DynEventSink, HoustonEvent};
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;
use thiserror::Error;

const DEFAULT_BUSY_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Error)]
pub enum OrchestrationError {
    #[error(transparent)]
    Resolver(#[from] RoleResolverError),
    #[error(transparent)]
    SyncSession(#[from] SyncSessionError),
    #[error("all agents busy for role {role_id}: {agents:?}")]
    AgentsBusy {
        role_id: String,
        agents: Vec<String>,
    },
    #[error("orchestration session failed: {0}")]
    Session(#[from] CoreError),
}

impl From<OrchestrationError> for CoreError {
    fn from(err: OrchestrationError) -> Self {
        match err {
            OrchestrationError::AgentsBusy { role_id, agents } => CoreError::Labeled {
                code: ErrorCode::Unavailable,
                kind: "agents_busy",
                message: format!(
                    "All agents for role '{role_id}' are busy after waiting: {}",
                    agents.join(", ")
                ),
            },
            OrchestrationError::Resolver(RoleResolverError::AgentHasNoRole { agent_name }) => {
                CoreError::BadRequest(format!("agent {agent_name} has no assigned role"))
            }
            OrchestrationError::Resolver(RoleResolverError::ProcedureNotFound {
                agent_name,
                procedure_id,
            }) => CoreError::NotFound(format!(
                "procedure {procedure_id} not found for agent {agent_name}"
            )),
            OrchestrationError::Resolver(e) => CoreError::BadRequest(e.to_string()),
            OrchestrationError::SyncSession(e) => CoreError::Labeled {
                code: ErrorCode::Unavailable,
                kind: "sync_session_failed",
                message: e.to_string(),
            },
            OrchestrationError::Session(e) => e,
        }
    }
}

impl From<RoleResolverError> for CoreError {
    fn from(err: RoleResolverError) -> Self {
        OrchestrationError::Resolver(err).into()
    }
}

#[derive(Debug, Clone)]
pub struct OrchestrationParams {
    pub workspace_path: std::path::PathBuf,
    pub orchestrator_agent_dir: std::path::PathBuf,
    pub orchestrator_agent_name: String,
    pub procedure_id: String,
    pub session_key: String,
    pub user_prompt: Option<String>,
    pub busy_wait_timeout: Option<Duration>,
    pub sync_session_timeout: Option<Duration>,
}

/// Build the enriched main-session prompt from gathered sub-session context.
pub fn build_enriched_prompt(
    procedure: &Procedure,
    contexts: &[(String, String)],
    user_prompt: Option<&str>,
) -> String {
    let mut parts = Vec::new();
    for (key, value) in contexts {
        parts.push(format!("[Contexto {key}]\n{value}"));
    }
    parts.push(format!(
        "Procedimiento: {} — {}.\nEjecuta el procedimiento con el contexto anterior.",
        procedure.id, procedure.description
    ));
    if let Some(extra) = user_prompt.filter(|s| !s.trim().is_empty()) {
        parts.push(format!("Instrucciones adicionales del usuario:\n{extra}"));
    }
    parts.join("\n\n")
}

pub async fn run_orchestrated_procedure(
    engine: &EngineState,
    events: DynEventSink,
    params: OrchestrationParams,
) -> Result<(), OrchestrationError> {
    let resolver = RoleResolver::load(&params.workspace_path)?;
    let resolved = resolver.resolve_procedure(
        &params.orchestrator_agent_name,
        &params.procedure_id,
    )?;

    let orchestrator_path = params.orchestrator_agent_dir.to_string_lossy().to_string();
    events.emit(HoustonEvent::OrchestrationProcedureStarted {
        agent_path: orchestrator_path.clone(),
        procedure_id: params.procedure_id.clone(),
    });

    let busy_config = BusyWaitConfig {
        timeout: params
            .busy_wait_timeout
            .unwrap_or(DEFAULT_BUSY_TIMEOUT),
        ..BusyWaitConfig::default()
    };
    let mut grouped: HashMap<String, Vec<super::resolver::DataRequest>> = HashMap::new();
    for request in resolved.data_requests {
        grouped
            .entry(request.role_id.clone())
            .or_default()
            .push(request);
    }

    let mut handles = Vec::new();
    for (role_id, requests) in grouped {
        let engine = engine.clone();
        let events = events.clone();
        let resolver_workspace = params.workspace_path.clone();
        let busy_config = busy_config.clone();
        let sync_timeout = params.sync_session_timeout;
        let availability_rt = engine.sessions.clone();

        handles.push(tokio::spawn(async move {
            let resolver = RoleResolver::load(&resolver_workspace)?;
            let candidates = resolver.candidates_for_role(&role_id)?;
            let agent_paths: Vec<String> = candidates
                .iter()
                .map(|(_, dir)| dir.to_string_lossy().to_string())
                .collect();
            let candidate_refs: Vec<(&str, &Path, &str)> = candidates
                .iter()
                .zip(agent_paths.iter())
                .map(|((name, dir), path)| (name.as_str(), dir.as_path(), path.as_str()))
                .collect();
            let availability = AgentAvailability::new(&availability_rt);
            let (agent_name, agent_dir, _) = availability
                .pick_available(&candidate_refs, &busy_config)
                .await
                .map_err(|AgentsBusyError { agents }| OrchestrationError::AgentsBusy {
                    role_id: role_id.clone(),
                    agents,
                })?;

            let mut outputs = Vec::new();
            for request in requests {
                let prompt = build_provision_prompt(&request.provides.id, &request.provides.description);
                let text = run_sync_session(
                    &engine,
                    events.clone(),
                    agent_dir,
                    &request.provides.id,
                    &prompt,
                    sync_timeout,
                )
                .await?;
                outputs.push((request.require_key, text));
            }
            Ok::<_, OrchestrationError>((agent_name.to_string(), outputs))
        }));
    }

    let mut contexts = Vec::new();
    for handle in handles {
        let (_agent, mut partial) = handle
            .await
            .map_err(|e| OrchestrationError::Session(CoreError::Internal(e.to_string())))??;
        contexts.append(&mut partial);
    }
    contexts.sort_by(|a, b| a.0.cmp(&b.0));

    let prompt = build_enriched_prompt(
        &resolved.procedure,
        &contexts,
        params.user_prompt.as_deref(),
    );
    let provider = sessions::resolve_provider(&params.orchestrator_agent_dir);
    let start = StartParams {
        agent_dir: params.orchestrator_agent_dir.clone(),
        working_dir: params.orchestrator_agent_dir,
        session_key: params.session_key,
        prompt,
        system_prompt: None,
        source: Some("orchestration".into()),
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
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use houston_engine_protocol::Procedure;

    #[test]
    fn enriched_prompt_tags_context_and_procedure() {
        let procedure = Procedure {
            id: "monthly_executive_report".into(),
            description: "Monthly executive report".into(),
            requires: vec![],
        };
        let contexts = vec![
            (
                "finance.financial_summary".into(),
                "Revenue: 100k".into(),
            ),
            (
                "marketing.campaign_performance".into(),
                "Reach: 50k".into(),
            ),
        ];
        let prompt = build_enriched_prompt(&procedure, &contexts, Some("Focus on Q1"));
        assert!(prompt.contains("[Contexto finance.financial_summary]"));
        assert!(prompt.contains("Revenue: 100k"));
        assert!(prompt.contains("[Contexto marketing.campaign_performance]"));
        assert!(prompt.contains("monthly_executive_report"));
        assert!(prompt.contains("Focus on Q1"));
    }
}
