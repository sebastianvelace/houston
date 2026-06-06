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
///
/// bwrap-specific note: the staged HOME contains symlinks (e.g.
/// `.claude/.credentials.json` → `~/.claude/.credentials.json`). Inside the
/// bwrap mount namespace, symlink targets that live outside the mounted dirs
/// are unreachable. We walk the staged home, resolve every symlink, and add
/// each target's parent directory as an RO path so bwrap binds it into the
/// container at the same absolute path.
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
    for path in ro_symlink_target_dirs(staged_home) {
        policy = policy.with_ro_path(path);
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

/// Walk `dir` recursively; for every symlink found, resolve its target and
/// collect the target's parent directory. These dirs must be RO-bound in bwrap
/// so that symlinks inside the staged HOME (e.g. `.claude/.credentials.json`)
/// can be followed across the mount namespace boundary.
fn ro_symlink_target_dirs(dir: &Path) -> Vec<PathBuf> {
    let mut out = BTreeSet::new();
    collect_symlink_target_dirs(dir, dir, &mut out);
    out.into_iter().collect()
}

fn collect_symlink_target_dirs(root: &Path, dir: &Path, out: &mut BTreeSet<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = fs::symlink_metadata(&path) else { continue };
        if meta.file_type().is_symlink() {
            if let Ok(target) = fs::read_link(&path) {
                let resolved = if target.is_absolute() {
                    target
                } else {
                    path.parent()
                        .map(|p| p.join(&target))
                        .unwrap_or(target)
                };
                if let Some(parent) = resolved.parent() {
                    // Skip targets that live inside the staged home itself —
                    // those are already covered by the RW bind.
                    if !parent.starts_with(root) {
                        out.insert(parent.to_path_buf());
                    }
                }
            }
        } else if path.is_dir() {
            collect_symlink_target_dirs(root, &path, out);
        }
    }
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
    fn enrich_adds_symlink_target_dirs_as_ro_paths() {
        use std::os::unix::fs::symlink;
        let tmp = TempDir::new().unwrap();
        std::env::set_var("HOUSTON_HOME", tmp.path());

        let real_home = tmp.path().join("real-home");
        let real_claude = real_home.join(".claude");
        fs::create_dir_all(&real_claude).unwrap();
        fs::write(real_claude.join(".credentials.json"), b"{}").unwrap();

        let staged = tmp.path().join("runtime/claude-home/s1");
        let staged_claude = staged.join(".claude");
        fs::create_dir_all(&staged_claude).unwrap();
        symlink(
            real_claude.join(".credentials.json"),
            staged_claude.join(".credentials.json"),
        )
        .unwrap();

        let cli = tmp.path().join("bin/claude");
        fs::create_dir_all(cli.parent().unwrap()).unwrap();
        fs::write(&cli, b"").unwrap();

        let policy = enrich_policy_for_cli_spawn(
            SessionPolicy::for_working_dir(tmp.path().join("agent"), None),
            &cli,
            &staged,
        );

        assert!(
            policy.extra_ro_paths.iter().any(|p| *p == real_claude),
            "must add real .claude dir as RO so bwrap can follow credential symlink; \
             got: {:?}",
            policy.extra_ro_paths,
        );

        std::env::remove_var("HOUSTON_HOME");
    }

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
