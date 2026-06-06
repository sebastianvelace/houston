//! Agent roles resolution and multi-agent orchestration.

mod availability;
mod orchestrator;
mod resolver;
mod sync_session;

pub use availability::{AgentAvailability, BusyWaitConfig};
pub use orchestrator::{
    build_enriched_prompt, run_orchestrated_procedure, OrchestrationError, OrchestrationParams,
};
pub use resolver::{DataRequest, ResolvedProcedure, RoleResolver, RoleResolverError};
pub use sync_session::{run_sync_session, SyncSessionError};
