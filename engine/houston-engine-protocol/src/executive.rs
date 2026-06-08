use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExecutiveConfig {
    pub version: u32,
    pub executive_agent: String,
    pub connected_agents: Vec<String>,
}

impl Default for ExecutiveConfig {
    fn default() -> Self {
        Self {
            version: 1,
            executive_agent: "Director".into(),
            connected_agents: Vec::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutiveBriefingRequest {
    pub prompt: String,
    pub session_key: String,
    #[serde(default)]
    pub busy_wait_timeout_secs: Option<u64>,
    #[serde(default)]
    pub sync_session_timeout_secs: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutiveBriefingResponse {
    pub session_key: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executive_config_round_trip() {
        let cfg = ExecutiveConfig {
            version: 1,
            executive_agent: "Director".into(),
            connected_agents: vec!["Contabilidad".into(), "Marketing".into()],
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let parsed: ExecutiveConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, parsed);
    }
}
