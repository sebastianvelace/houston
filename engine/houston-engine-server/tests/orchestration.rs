//! Integration tests for workspace roles and orchestration routes.

use houston_engine_server::{build_router, ServerConfig, ServerState};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;

async fn spawn() -> (SocketAddr, String, tempfile::TempDir) {
    let token = "orch-test".to_string();
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
async fn roles_get_put_roundtrip() {
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

    let empty: serde_json::Value = c
        .get(format!("http://{addr}/v1/workspaces/{workspace_id}/roles"))
        .bearer_auth(&tok)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(empty["version"], 1);
    assert_eq!(empty["roles"].as_array().unwrap().len(), 0);

    let body = serde_json::json!({
        "version": 1,
        "roles": [{
            "id": "finance",
            "name": "Finance",
            "agents": ["Accounting"],
            "provides": [{ "id": "summary", "description": "Summary data" }],
            "procedures": []
        }]
    });
    let saved: serde_json::Value = c
        .put(format!("http://{addr}/v1/workspaces/{workspace_id}/roles"))
        .bearer_auth(&tok)
        .json(&body)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(saved["roles"][0]["id"], "finance");
}

#[tokio::test]
async fn orchestrate_rejects_unknown_procedure() {
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

    let created: serde_json::Value = c
        .post(format!("http://{addr}/v1/workspaces/{workspace_id}/agents"))
        .bearer_auth(&tok)
        .json(&serde_json::json!({
            "name": "Director",
            "configId": "blank"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(created["agent"]["name"], "Director");

    let res = c
        .post(format!(
            "http://{addr}/v1/workspaces/{workspace_id}/agents/Director/orchestrate"
        ))
        .bearer_auth(&tok)
        .json(&serde_json::json!({
            "procedureId": "monthly_report",
            "sessionKey": "orch-1"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
}
