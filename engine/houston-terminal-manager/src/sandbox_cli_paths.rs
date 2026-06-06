//! Extra sandbox allowlist paths for CLI subprocess spawns.
//!
//! Bubblewrap and Landlock jails only expose system dirs plus the agent
//! working directory. Managed CLIs (`~/.local/bin/claude`, bundled codex)
//! and Houston's per-session staged HOME live outside that tree and must
//! be explicitly allowlisted or the spawn dies with `execvp: No such file`.

use houston_policy::{houston_data_root, SessionPolicy};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Extend `policy` with read-only CLI install paths and a read-write
/// Houston runtime bind for `staged_home`.
pub fn enrich_policy_for_cli_spawn(
    policy: SessionPolicy,
    cli_path: &Path,
    staged_home: &Path,
) -> SessionPolicy {
    let mut policy = policy;
    for path in readonly_paths_for_cli(cli_path) {
        policy = policy.with_ro_path(path);
    }
    for path in rw_paths_for_staged_home(staged_home) {
        policy = policy.with_rw_path(path);
    }
    policy
}

fn readonly_paths_for_cli(cli_path: &Path) -> Vec<PathBuf> {
    let mut paths = BTreeSet::new();
    if let Some(parent) = cli_path.parent() {
        paths.insert(parent.to_path_buf());
    }
    if let Ok(meta) = fs::symlink_metadata(cli_path) {
        if meta.file_type().is_symlink() {
            if let Ok(target) = fs::read_link(cli_path) {
                let resolved = resolve_cli_target(cli_path, &target);
                if let Some(parent) = resolved.parent() {
                    paths.insert(parent.to_path_buf());
                }
                if let Some(share_root) = claude_share_root(&resolved) {
                    paths.insert(share_root);
                }
            }
        }
    }
    paths.into_iter().collect()
}

fn rw_paths_for_staged_home(staged_home: &Path) -> Vec<PathBuf> {
    let runtime_root = houston_data_root().join("runtime");
    let mut paths = BTreeSet::new();
    paths.insert(staged_home.to_path_buf());
    if runtime_root.exists() {
        paths.insert(runtime_root);
    }
    paths.into_iter().collect()
}

fn resolve_cli_target(cli_path: &Path, target: &Path) -> PathBuf {
    if target.is_absolute() {
        target.to_path_buf()
    } else {
        cli_path
            .parent()
            .map(|p| p.join(target))
            .unwrap_or_else(|| target.to_path_buf())
    }
}

/// `~/.local/share/claude` when the resolved binary lives under that tree.
fn claude_share_root(resolved: &Path) -> Option<PathBuf> {
    const MARKER: &str = "/.local/share/claude";
    let s = resolved.to_string_lossy();
    let idx = s.find(MARKER)?;
    Some(PathBuf::from(&s[..idx + MARKER.len()]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use houston_policy::SessionPolicy;
    use std::os::unix::fs::symlink;
    use tempfile::TempDir;

    #[test]
    fn enrich_adds_cli_bin_and_runtime_paths() {
        let tmp = TempDir::new().unwrap();
        std::env::set_var("HOUSTON_HOME", tmp.path());

        let share = tmp.path().join("share/claude/versions");
        fs::create_dir_all(&share).unwrap();
        let bin = tmp.path().join("bin");
        fs::create_dir_all(&bin).unwrap();
        let binary = share.join("2.1.0");
        fs::write(&binary, b"").unwrap();
        symlink(&binary, bin.join("claude")).unwrap();

        let staged = tmp.path().join("runtime/claude-home/s1");
        fs::create_dir_all(&staged).unwrap();

        let policy = enrich_policy_for_cli_spawn(
            SessionPolicy::for_working_dir(tmp.path().join("agent"), None),
            &bin.join("claude"),
            &staged,
        );
        assert!(policy.extra_ro_paths.iter().any(|p| p.ends_with("bin")));
        assert!(policy
            .extra_rw_paths
            .iter()
            .any(|p| p.ends_with("runtime")));

        std::env::remove_var("HOUSTON_HOME");
    }
}
