//! Workspace roles and orchestration procedure types.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Workspace-level roles configuration (`roles.json`).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct WorkspaceRoles {
    pub version: u32,
    pub roles: Vec<Role>,
}

impl Default for WorkspaceRoles {
    fn default() -> Self {
        Self {
            version: 1,
            roles: Vec::new(),
        }
    }
}

/// A role grouping agents that share data provisions and procedures.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct Role {
    pub id: String,
    pub name: String,
    pub agents: Vec<String>,
    pub provides: Vec<DataProvision>,
    pub procedures: Vec<Procedure>,
}

/// Data an agent role can provide to an orchestrator.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct DataProvision {
    pub id: String,
    pub description: String,
}

/// A procedure an orchestrator agent can execute.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct Procedure {
    pub id: String,
    pub description: String,
    /// Entries are `"{role_id}.{provides_id}"`.
    pub requires: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_roles_round_trip() {
        let roles = WorkspaceRoles {
            version: 1,
            roles: vec![Role {
                id: "finance".into(),
                name: "Finanzas".into(),
                agents: vec!["Contabilidad".into()],
                provides: vec![DataProvision {
                    id: "financial_summary".into(),
                    description: "Resumen financiero".into(),
                }],
                procedures: vec![Procedure {
                    id: "reconcile".into(),
                    description: "Reconciliar cuentas".into(),
                    requires: vec![],
                }],
            }],
        };
        let json = serde_json::to_string(&roles).unwrap();
        let parsed: WorkspaceRoles = serde_json::from_str(&json).unwrap();
        assert_eq!(roles, parsed);
    }
}
