//! Integration tests for executive manager routes.

use houston_engine_server::{build_router, ServerConfig, ServerState};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;

async fn spawn() -> (SocketAddr, String, tempfile::TempDir) {
    let token = "exec-test".to_string();
    let docs = tempfile::TempDir::new().unwrap();
    let home = tempfile::TempDir::new().unwrap();
    let cfg = ServerConfig {
        bind: "127.0.0.1:0".parse().unwrap(),
        token: token.clone(),
        home_dir: home.path().to_path_buf(),
        docs_dir: docs.path().to_path_buf(),
        app_system_prompt: String::new(),
        app_onboarding_prompt: String::new(),
        tunnel_url: "http://test.invalid".into(),
    };
    let listener = TcpListener::bind(cfg.bind).await.unwrap();
    let addr = listener.local_addr().unwrap();
    let state = Arc::new(ServerState::new_in_memory(cfg).await.unwrap());
    let app = build_router(state);
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    std::mem::forget(home);
    (addr, token, docs)
}

#[tokio::test]
async fn executive_config_get_put_roundtrip() {
    let (addr, tok, _docs) = spawn().await;
    let c = reqwest::Client::new();

    let ws: serde_json::Value = c
        .post(format!("http://{addr}/v1/workspaces"))
        .bearer_auth(&tok)
        .json(&serde_json::json!({ "name": "alpha" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let workspace_id = ws["id"].as_str().unwrap();

    let default_cfg: serde_json::Value = c
        .get(format!("http://{addr}/v1/workspaces/{workspace_id}/executive-config"))
        .bearer_auth(&tok)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(default_cfg["version"], 1);
    assert_eq!(default_cfg["executiveAgent"], "Director");
    assert_eq!(
        default_cfg["connectedAgents"].as_array().unwrap().len(),
        0
    );

    let agents: Vec<serde_json::Value> = c
        .get(format!("http://{addr}/v1/workspaces/{workspace_id}/agents"))
        .bearer_auth(&tok)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let director = agents
        .iter()
        .find(|a| a["name"] == "Director")
        .expect("GET should auto-create Director");
    assert_eq!(director["name"], "Director");

    let created: serde_json::Value = c
        .post(format!("http://{addr}/v1/workspaces/{workspace_id}/agents"))
        .bearer_auth(&tok)
        .json(&serde_json::json!({
            "name": "Marketing",
            "configId": "blank"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(created["agent"]["name"], "Marketing");

    let body = serde_json::json!({
        "version": 1,
        "executiveAgent": "Director",
        "connectedAgents": ["Marketing"]
    });
    let saved: serde_json::Value = c
        .put(format!("http://{addr}/v1/workspaces/{workspace_id}/executive-config"))
        .bearer_auth(&tok)
        .json(&body)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(saved["connectedAgents"][0], "Marketing");

    let rejected = c
        .put(format!("http://{addr}/v1/workspaces/{workspace_id}/executive-config"))
        .bearer_auth(&tok)
        .json(&serde_json::json!({
            "version": 1,
            "executiveAgent": "Director",
            "connectedAgents": ["Ghost"]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(rejected.status(), 400);
}
