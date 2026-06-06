//! Workspace executive manager configuration (`executive-config.json`).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Workspace-level executive manager configuration.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
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
