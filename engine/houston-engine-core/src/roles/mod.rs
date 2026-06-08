mod availability;
mod executive;
mod sync_session;

pub use availability::{AgentAvailability, BusyWaitConfig};
pub use executive::{
    build_briefing_prompt, build_executive_enriched_prompt, ensure_executive_agent,
    ensure_executive_agent_named, ensure_executive_agents_for_all_workspaces,
    run_executive_briefing, validate_connected_agents, write_validated_executive_config,
    ExecutiveBriefingParams, ExecutiveError,
};
pub use sync_session::{run_sync_session, SyncSessionError};
