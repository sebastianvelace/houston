//! Workspace-level `roles.json` read/write and migration.

use houston_engine_protocol::WorkspaceRoles;
use std::fs;
use std::io::Write as _;
use std::path::Path;

use crate::{AgentFilesError, Result};

const ROLES_FILE: &str = "roles.json";
const SUPPORTED_VERSION: u32 = 1;

/// Embedded JSON Schema for `roles.json`.
pub const ROLES_SCHEMA: &str =
    include_str!("../../../ui/agent-schemas/src/roles.schema.json");

fn roles_path(workspace_path: &Path) -> std::path::PathBuf {
    workspace_path.join(ROLES_FILE)
}

fn unique_tmp_path(path: &Path) -> std::path::PathBuf {
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("roles.json");
    path.with_file_name(format!(".{file_name}.{}.tmp", uuid::Uuid::new_v4()))
}

/// Validate a parsed `WorkspaceRoles` document against supported version and shape.
pub fn validate_workspace_roles(roles: &WorkspaceRoles) -> Result<()> {
    if roles.version != SUPPORTED_VERSION {
        return Err(AgentFilesError::InvalidPath(format!(
            "unsupported roles.json version {} (expected {SUPPORTED_VERSION})",
            roles.version
        )));
    }
    let mut seen_role_ids = std::collections::HashSet::new();
    for role in &roles.roles {
        if role.id.trim().is_empty() {
            return Err(AgentFilesError::InvalidPath(
                "role id must not be empty".into(),
            ));
        }
        if !seen_role_ids.insert(role.id.clone()) {
            return Err(AgentFilesError::InvalidPath(format!(
                "duplicate role id {:?}",
                role.id
            )));
        }
        for provides in &role.provides {
            if provides.id.trim().is_empty() || provides.description.trim().is_empty() {
                return Err(AgentFilesError::InvalidPath(
                    "provides entries require non-empty id and description".into(),
                ));
            }
        }
        for procedure in &role.procedures {
            if procedure.id.trim().is_empty() || procedure.description.trim().is_empty() {
                return Err(AgentFilesError::InvalidPath(
                    "procedure entries require non-empty id and description".into(),
                ));
            }
        }
    }
    Ok(())
}

/// Read `roles.json` from a workspace directory. Missing file returns the default empty document.
pub fn read_workspace_roles(workspace_path: &Path) -> Result<WorkspaceRoles> {
    let path = roles_path(workspace_path);
    if !path.exists() {
        return Ok(WorkspaceRoles::default());
    }
    let content = fs::read_to_string(&path)?;
    let roles: WorkspaceRoles = serde_json::from_str(&content)?;
    validate_workspace_roles(&roles)?;
    Ok(roles)
}

/// Atomically write `roles.json` after validation.
pub fn write_workspace_roles(workspace_path: &Path, roles: &WorkspaceRoles) -> Result<()> {
    validate_workspace_roles(roles)?;
    fs::create_dir_all(workspace_path)?;
    let path = roles_path(workspace_path);
    let tmp = unique_tmp_path(&path);
    let json = serde_json::to_string_pretty(roles)?;
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(json.as_bytes())?;
        f.sync_all()?;
    }
    fs::rename(&tmp, &path)?;
    Ok(())
}

/// Create an empty `roles.json` in `workspace_path` when missing. Idempotent.
pub fn ensure_roles_file(workspace_path: &Path) -> Result<()> {
    let path = roles_path(workspace_path);
    if path.exists() {
        return Ok(());
    }
    write_workspace_roles(workspace_path, &WorkspaceRoles::default())
}

/// Walk every workspace under `workspaces_root` and ensure `roles.json` exists.
pub fn migrate_workspace_data(workspaces_root: &Path) -> Result<()> {
    if !workspaces_root.exists() {
        return Ok(());
    }
    let workspaces = match fs::read_dir(workspaces_root) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    for entry in workspaces.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        ensure_roles_file(&path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use houston_engine_protocol::{DataProvision, Procedure, Role};
    use tempfile::TempDir;

    #[test]
    fn missing_file_returns_default() {
        let dir = TempDir::new().unwrap();
        let roles = read_workspace_roles(dir.path()).unwrap();
        assert_eq!(roles, WorkspaceRoles::default());
    }

    #[test]
    fn round_trip_write_read() {
        let dir = TempDir::new().unwrap();
        let roles = WorkspaceRoles {
            version: 1,
            roles: vec![Role {
                id: "finance".into(),
                name: "Finance".into(),
                agents: vec!["Accounting".into()],
                provides: vec![DataProvision {
                    id: "summary".into(),
                    description: "Financial summary".into(),
                }],
                procedures: vec![Procedure {
                    id: "report".into(),
                    description: "Generate report".into(),
                    requires: vec!["marketing.campaigns".into()],
                }],
            }],
        };
        write_workspace_roles(dir.path(), &roles).unwrap();
        let got = read_workspace_roles(dir.path()).unwrap();
        assert_eq!(got, roles);
    }

    #[test]
    fn unknown_version_errors() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join(ROLES_FILE),
            r#"{"version":99,"roles":[]}"#,
        )
        .unwrap();
        let err = read_workspace_roles(dir.path()).unwrap_err();
        assert!(err.to_string().contains("unsupported roles.json version"));
    }

    #[test]
    fn migrate_creates_empty_roles_json() {
        let root = TempDir::new().unwrap();
        let ws = root.path().join("Acme");
        fs::create_dir_all(&ws).unwrap();
        migrate_workspace_data(root.path()).unwrap();
        let got = read_workspace_roles(&ws).unwrap();
        assert_eq!(got, WorkspaceRoles::default());
        assert!(ws.join(ROLES_FILE).exists());
    }

    #[test]
    fn migrate_is_idempotent() {
        let root = TempDir::new().unwrap();
        let ws = root.path().join("Acme");
        fs::create_dir_all(&ws).unwrap();
        migrate_workspace_data(root.path()).unwrap();
        write_workspace_roles(
            &ws,
            &WorkspaceRoles {
                version: 1,
                roles: vec![Role {
                    id: "ops".into(),
                    name: "Ops".into(),
                    agents: vec![],
                    provides: vec![],
                    procedures: vec![],
                }],
            },
        )
        .unwrap();
        migrate_workspace_data(root.path()).unwrap();
        let got = read_workspace_roles(&ws).unwrap();
        assert_eq!(got.roles.len(), 1);
        assert_eq!(got.roles[0].id, "ops");
    }
}
