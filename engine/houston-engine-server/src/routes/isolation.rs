//! `GET /v1/isolation/capabilities` — report OS sandbox features on this host.

use axum::Json;
use houston_engine_protocol::IsolationCapabilities;
use houston_sandbox::capabilities;

pub async fn capabilities_route() -> Json<IsolationCapabilities> {
    let caps = capabilities();
    Json(IsolationCapabilities {
        backend: caps.platform.to_string(),
        filesystem_isolation: caps.filesystem_isolation,
        network_isolation: caps.network_isolation,
        fd_cleanup: caps.fd_cleanup,
        credential_isolation: caps.credential_isolation,
        platform: std::env::consts::OS.to_string(),
    })
}
