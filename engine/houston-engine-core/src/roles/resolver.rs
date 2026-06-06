//! Resolve workspace roles into concrete data requests and procedures.

use houston_agent_files::{read_workspace_roles, AgentFilesError};
use houston_engine_protocol::{DataProvision, Procedure, Role, WorkspaceRoles};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RoleResolverError {
    #[error("agent {agent_name} has no assigned role")]
    AgentHasNoRole { agent_name: String },
    #[error("procedure {procedure_id} not found for agent {agent_name}")]
    ProcedureNotFound {
        agent_name: String,
        procedure_id: String,
    },
    #[error("malformed require entry {require} (expected role_id.provides_id)")]
    MalformedRequire { require: String },
    #[error("role {role_id} not found")]
    RoleNotFound { role_id: String },
    #[error("provision {role_id}.{provides_id} not found")]
    ProvisionNotFound {
        role_id: String,
        provides_id: String,
    },
    #[error("no agent available for role {role_id}")]
    NoAgentForRole { role_id: String },
    #[error("agent {agent_name} is not installed in workspace")]
    AgentNotInWorkspace { agent_name: String },
    #[error("failed to read roles.json: {0}")]
    Io(String),
}

pub struct RoleResolver {
    workspace_path: PathBuf,
    roles: WorkspaceRoles,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProcedure {
    pub procedure: Procedure,
    pub data_requests: Vec<DataRequest>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataRequest {
    pub role_id: String,
    pub provides: DataProvision,
    pub require_key: String,
}

impl RoleResolver {
    pub fn load(workspace_path: &Path) -> Result<Self, RoleResolverError> {
        let roles = read_workspace_roles(workspace_path).map_err(|e: AgentFilesError| {
            RoleResolverError::Io(e.to_string())
        })?;
        Ok(Self {
            workspace_path: workspace_path.to_path_buf(),
            roles,
        })
    }

    pub fn roles(&self) -> &WorkspaceRoles {
        &self.roles
    }

    pub fn role_for_agent(&self, agent_name: &str) -> Option<&Role> {
        self.roles.roles.iter().find(|role| role.agents.iter().any(|a| a == agent_name))
    }

    pub fn agents_with_role(&self, role_id: &str) -> Vec<&str> {
        self.roles
            .roles
            .iter()
            .find(|role| role.id == role_id)
            .map(|role| role.agents.iter().map(String::as_str).collect())
            .unwrap_or_default()
    }

    pub fn resolve_procedure(
        &self,
        agent_name: &str,
        procedure_id: &str,
    ) -> Result<ResolvedProcedure, RoleResolverError> {
        let role = self
            .role_for_agent(agent_name)
            .ok_or_else(|| RoleResolverError::AgentHasNoRole {
                agent_name: agent_name.to_string(),
            })?;
        let procedure = role
            .procedures
            .iter()
            .find(|p| p.id == procedure_id)
            .cloned()
            .ok_or_else(|| RoleResolverError::ProcedureNotFound {
                agent_name: agent_name.to_string(),
                procedure_id: procedure_id.to_string(),
            })?;

        let mut seen_requires = HashSet::new();
        let mut data_requests = Vec::new();
        for require in &procedure.requires {
            if !seen_requires.insert(require.clone()) {
                continue;
            }
            data_requests.push(self.resolve_require(require)?);
        }

        Ok(ResolvedProcedure {
            procedure,
            data_requests,
        })
    }

    fn resolve_require(&self, require: &str) -> Result<DataRequest, RoleResolverError> {
        let parts: Vec<&str> = require.split('.').collect();
        if parts.len() != 2 {
            return Err(RoleResolverError::MalformedRequire {
                require: require.to_string(),
            });
        }
        let role_id = parts[0].trim();
        let provides_id = parts[1].trim();
        if role_id.is_empty() || provides_id.is_empty() {
            return Err(RoleResolverError::MalformedRequire {
                require: require.to_string(),
            });
        }

        let role = self
            .roles
            .roles
            .iter()
            .find(|r| r.id == role_id)
            .ok_or_else(|| RoleResolverError::RoleNotFound {
                role_id: role_id.to_string(),
            })?;
        let provides = role
            .provides
            .iter()
            .find(|p| p.id == provides_id)
            .cloned()
            .ok_or_else(|| RoleResolverError::ProvisionNotFound {
                role_id: role_id.to_string(),
                provides_id: provides_id.to_string(),
            })?;

        if role.agents.is_empty() {
            return Err(RoleResolverError::NoAgentForRole {
                role_id: role_id.to_string(),
            });
        }

        Ok(DataRequest {
            role_id: role_id.to_string(),
            provides,
            require_key: require.to_string(),
        })
    }

    pub fn resolve_agent_dir(&self, agent_name: &str) -> Result<PathBuf, RoleResolverError> {
        let folder = self.workspace_path.join(agent_name);
        if folder.is_dir() && folder.join(".houston/agent.json").exists() {
            return Ok(folder);
        }
        for role in &self.roles.roles {
            if role.agents.iter().any(|a| a == agent_name) {
                let candidate = self.workspace_path.join(agent_name);
                if candidate.is_dir() {
                    return Ok(candidate);
                }
            }
        }
        Err(RoleResolverError::AgentNotInWorkspace {
            agent_name: agent_name.to_string(),
        })
    }

    pub fn candidates_for_role(&self, role_id: &str) -> Result<Vec<(String, PathBuf)>, RoleResolverError> {
        let role = self
            .roles
            .roles
            .iter()
            .find(|r| r.id == role_id)
            .ok_or_else(|| RoleResolverError::RoleNotFound {
                role_id: role_id.to_string(),
            })?;
        if role.agents.is_empty() {
            return Err(RoleResolverError::NoAgentForRole {
                role_id: role_id.to_string(),
            });
        }
        role.agents
            .iter()
            .map(|name| {
                let dir = self.resolve_agent_dir(name)?;
                Ok((name.clone(), dir))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use houston_agent_files::write_workspace_roles;
    use houston_engine_protocol::{DataProvision, Procedure, Role, WorkspaceRoles};
    use std::fs;
    use tempfile::TempDir;

    fn sample_roles() -> WorkspaceRoles {
        WorkspaceRoles {
            version: 1,
            roles: vec![
                Role {
                    id: "finance".into(),
                    name: "Finance".into(),
                    agents: vec!["Accounting".into()],
                    provides: vec![DataProvision {
                        id: "financial_summary".into(),
                        description: "Revenue and expenses".into(),
                    }],
                    procedures: vec![],
                },
                Role {
                    id: "marketing".into(),
                    name: "Marketing".into(),
                    agents: vec!["Marketing".into()],
                    provides: vec![DataProvision {
                        id: "campaign_performance".into(),
                        description: "Campaign metrics".into(),
                    }],
                    procedures: vec![],
                },
                Role {
                    id: "orchestrator".into(),
                    name: "Director".into(),
                    agents: vec!["Director".into()],
                    provides: vec![],
                    procedures: vec![Procedure {
                        id: "monthly_executive_report".into(),
                        description: "Monthly executive report".into(),
                        requires: vec![
                            "finance.financial_summary".into(),
                            "marketing.campaign_performance".into(),
                        ],
                    }],
                },
            ],
        }
    }

    fn seed_workspace(dir: &Path, roles: &WorkspaceRoles) {
        fs::create_dir_all(dir.join("Accounting/.houston")).unwrap();
        fs::write(dir.join("Accounting/.houston/agent.json"), r#"{"id":"a1","configId":"blank","createdAt":"t"}"#).unwrap();
        fs::create_dir_all(dir.join("Marketing/.houston")).unwrap();
        fs::write(dir.join("Marketing/.houston/agent.json"), r#"{"id":"a2","configId":"blank","createdAt":"t"}"#).unwrap();
        fs::create_dir_all(dir.join("Director/.houston")).unwrap();
        fs::write(dir.join("Director/.houston/agent.json"), r#"{"id":"a3","configId":"blank","createdAt":"t"}"#).unwrap();
        write_workspace_roles(dir, roles).unwrap();
    }

    #[test]
    fn resolve_procedure_expands_requires() {
        let dir = TempDir::new().unwrap();
        seed_workspace(dir.path(), &sample_roles());
        let resolver = RoleResolver::load(dir.path()).unwrap();
        let resolved = resolver
            .resolve_procedure("Director", "monthly_executive_report")
            .unwrap();
        assert_eq!(resolved.data_requests.len(), 2);
        assert_eq!(resolved.data_requests[0].require_key, "finance.financial_summary");
        assert_eq!(resolved.data_requests[0].role_id, "finance");
        assert_eq!(resolved.data_requests[1].require_key, "marketing.campaign_performance");
    }

    #[test]
    fn malformed_require_errors() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("Director/.houston")).unwrap();
        fs::write(
            dir.path().join("Director/.houston/agent.json"),
            r#"{"id":"a3","configId":"blank","createdAt":"t"}"#,
        )
        .unwrap();
        fs::write(
            dir.path().join("roles.json"),
            r#"{
                "version": 1,
                "roles": [{
                    "id": "orchestrator",
                    "name": "Director",
                    "agents": ["Director"],
                    "provides": [],
                    "procedures": [{
                        "id": "bad",
                        "description": "Bad",
                        "requires": ["finance"]
                    }]
                }]
            }"#,
        )
        .unwrap();
        let resolver = RoleResolver::load(dir.path()).unwrap();
        let err = resolver.resolve_procedure("Director", "bad").unwrap_err();
        assert!(matches!(err, RoleResolverError::MalformedRequire { .. }));
    }

    #[test]
    fn missing_role_errors() {
        let dir = TempDir::new().unwrap();
        seed_workspace(dir.path(), &sample_roles());
        let resolver = RoleResolver::load(dir.path()).unwrap();
        let err = resolver.resolve_procedure("Unknown", "monthly_executive_report").unwrap_err();
        assert!(matches!(err, RoleResolverError::AgentHasNoRole { .. }));
    }

    #[test]
    fn missing_procedure_errors() {
        let dir = TempDir::new().unwrap();
        seed_workspace(dir.path(), &sample_roles());
        let resolver = RoleResolver::load(dir.path()).unwrap();
        let err = resolver.resolve_procedure("Director", "nope").unwrap_err();
        assert!(matches!(err, RoleResolverError::ProcedureNotFound { .. }));
    }
}
