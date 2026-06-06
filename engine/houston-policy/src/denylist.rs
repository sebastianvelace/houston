//! Global and per-workspace denylist construction for [`SessionPolicy`].

use std::path::{Path, PathBuf};

/// Houston data root (`~/.houston` or `~/.dev-houston` in debug).
pub fn houston_data_root() -> PathBuf {
    if let Ok(override_path) = std::env::var("HOUSTON_HOME") {
        return PathBuf::from(override_path);
    }
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let subdir = if cfg!(debug_assertions) {
        ".dev-houston"
    } else {
        ".houston"
    };
    home.join(subdir)
}

/// Root containing all workspaces (`~/.houston/workspaces`).
pub fn workspaces_root() -> PathBuf {
    houston_data_root().join("workspaces")
}

/// Parent workspace folder for an agent under `workspaces/{Ws}/{Agent}`.
pub fn infer_workspace_root(agent_root: &Path) -> Option<PathBuf> {
    let workspaces = workspaces_root();
    let agent = canonical_or(agent_root);
    let ws = canonical_or(&workspaces);
    if agent.starts_with(&ws) {
        return agent.parent().map(|p| p.to_path_buf());
    }
    None
}

/// Build the denylist for one agent session.
pub fn build_denied_prefixes(agent_root: &Path, workspace_root: Option<&Path>) -> Vec<PathBuf> {
    let mut denied = global_denied_prefixes();
    denied.push(workspaces_root());

    let ws = workspace_root
        .map(|p| p.to_path_buf())
        .or_else(|| infer_workspace_root(agent_root));

    if let Some(ws_root) = ws {
        if let Ok(entries) = std::fs::read_dir(&ws_root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && !same_entity(&path, agent_root) {
                    denied.push(path);
                }
            }
        }
    }

    denied
}

fn global_denied_prefixes() -> Vec<PathBuf> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    let houston = houston_data_root();
    let mut out = vec![
        home.join(".claude"),
        home.join(".codex"),
        home.join(".gemini"),
        home.join(".ssh"),
        home.join(".gnupg"),
        home.join(".aws"),
        home.join(".config").join("gh"),
        houston.join("engine.json"),
    ];
    if home.join(".claude.json").exists() {
        out.push(home.join(".claude.json"));
    }
    out
}

fn canonical_or(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn same_entity(a: &Path, b: &Path) -> bool {
    let ca = canonical_or(a);
    let cb = canonical_or(b);
    ca == cb
}

/// True when `child` resolves under `root` (inclusive).
pub fn path_within_root(child: &Path, root: &Path) -> bool {
    let child = canonical_or(child);
    let root = canonical_or(root);
    child.starts_with(&root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn denylist_excludes_sibling_agent_dirs() {
        let tmp = TempDir::new().unwrap();
        let ws = tmp.path().join("workspaces").join("MiEmpresa");
        let marketing = ws.join("Marketing");
        let contabilidad = ws.join("Contabilidad");
        fs::create_dir_all(&marketing).unwrap();
        fs::create_dir_all(&contabilidad).unwrap();

        let denied = build_denied_prefixes(&marketing, Some(&ws));
        assert!(
            denied.iter().any(|p| p.ends_with("Contabilidad")),
            "Contabilidad must be denied for Marketing"
        );
    }

    #[test]
    fn allowlist_includes_own_agent_root() {
        let tmp = TempDir::new().unwrap();
        let ws = tmp.path().join("workspaces").join("MiEmpresa");
        let marketing = ws.join("Marketing");
        fs::create_dir_all(&marketing).unwrap();

        let denied = build_denied_prefixes(&marketing, Some(&ws));
        assert!(
            !denied.iter().any(|p| same_entity(p, &marketing)),
            "own agent_root must not appear in denied_prefixes"
        );
    }

    #[test]
    fn path_within_root_accepts_child() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("agent");
        let child = root.join("subdir");
        fs::create_dir_all(&child).unwrap();
        assert!(path_within_root(&child, &root));
    }

    #[test]
    fn path_within_root_rejects_sibling() {
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("Marketing");
        let b = tmp.path().join("Contabilidad");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        assert!(!path_within_root(&b, &a));
    }
}
