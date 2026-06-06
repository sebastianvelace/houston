//! Workspace-level `executive-config.json` read/write and migration.

use houston_engine_protocol::ExecutiveConfig;
use std::fs;
use std::io::Write as _;
use std::path::Path;

use crate::{AgentFilesError, Result};

const EXECUTIVE_CONFIG_FILE: &str = "executive-config.json";
const SUPPORTED_VERSION: u32 = 1;

fn config_path(workspace_path: &Path) -> std::path::PathBuf {
    workspace_path.join(EXECUTIVE_CONFIG_FILE)
}

fn unique_tmp_path(path: &Path) -> std::path::PathBuf {
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(EXECUTIVE_CONFIG_FILE);
    path.with_file_name(format!(".{file_name}.{}.tmp", uuid::Uuid::new_v4()))
}

/// Validate a parsed `ExecutiveConfig` document.
pub fn validate_executive_config(config: &ExecutiveConfig) -> Result<()> {
    if config.version != SUPPORTED_VERSION {
        return Err(AgentFilesError::InvalidPath(format!(
            "unsupported executive-config.json version {} (expected {SUPPORTED_VERSION})",
            config.version
        )));
    }
    if config.executive_agent.trim().is_empty() {
        return Err(AgentFilesError::InvalidPath(
            "executiveAgent must not be empty".into(),
        ));
    }
    let mut seen = std::collections::HashSet::new();
    for name in &config.connected_agents {
        if name.trim().is_empty() {
            return Err(AgentFilesError::InvalidPath(
                "connectedAgents entries must not be empty".into(),
            ));
        }
        if !seen.insert(name.clone()) {
            return Err(AgentFilesError::InvalidPath(format!(
                "duplicate connected agent {:?}",
                name
            )));
        }
    }
    Ok(())
}

/// Read `executive-config.json`. Missing file returns the default document.
pub fn read_executive_config(workspace_path: &Path) -> Result<ExecutiveConfig> {
    let path = config_path(workspace_path);
    if !path.exists() {
        return Ok(ExecutiveConfig::default());
    }
    let content = fs::read_to_string(&path)?;
    let config: ExecutiveConfig = serde_json::from_str(&content)?;
    validate_executive_config(&config)?;
    Ok(config)
}

/// Atomically write `executive-config.json` after validation.
pub fn write_executive_config(workspace_path: &Path, config: &ExecutiveConfig) -> Result<()> {
    validate_executive_config(config)?;
    fs::create_dir_all(workspace_path)?;
    let path = config_path(workspace_path);
    let tmp = unique_tmp_path(&path);
    let json = serde_json::to_string_pretty(config)?;
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(json.as_bytes())?;
        f.sync_all()?;
    }
    fs::rename(&tmp, &path)?;
    Ok(())
}

/// Create a default `executive-config.json` when missing. Idempotent.
pub fn ensure_executive_config_file(workspace_path: &Path) -> Result<()> {
    let path = config_path(workspace_path);
    if path.exists() {
        return Ok(());
    }
    write_executive_config(workspace_path, &ExecutiveConfig::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn missing_file_returns_default() {
        let dir = TempDir::new().unwrap();
        let cfg = read_executive_config(dir.path()).unwrap();
        assert_eq!(cfg, ExecutiveConfig::default());
    }

    #[test]
    fn round_trip_write_read() {
        let dir = TempDir::new().unwrap();
        let cfg = ExecutiveConfig {
            version: 1,
            executive_agent: "Director".into(),
            connected_agents: vec!["Contabilidad".into(), "Marketing".into()],
        };
        write_executive_config(dir.path(), &cfg).unwrap();
        let got = read_executive_config(dir.path()).unwrap();
        assert_eq!(got, cfg);
    }

    #[test]
    fn ensure_creates_default_file() {
        let dir = TempDir::new().unwrap();
        ensure_executive_config_file(dir.path()).unwrap();
        assert!(dir.path().join(EXECUTIVE_CONFIG_FILE).exists());
        let got = read_executive_config(dir.path()).unwrap();
        assert_eq!(got, ExecutiveConfig::default());
    }

    #[test]
    fn ensure_is_idempotent() {
        let dir = TempDir::new().unwrap();
        let custom = ExecutiveConfig {
            version: 1,
            executive_agent: "CEO".into(),
            connected_agents: vec!["Sales".into()],
        };
        write_executive_config(dir.path(), &custom).unwrap();
        ensure_executive_config_file(dir.path()).unwrap();
        let got = read_executive_config(dir.path()).unwrap();
        assert_eq!(got.executive_agent, "CEO");
    }
}
