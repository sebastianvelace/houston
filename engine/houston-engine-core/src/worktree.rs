//! Git worktree management + generic shell execution — relocated from
//! `app/src-tauri/src/commands/worktree.rs`.
//!
//! OS-native helpers (`pick_directory`, `open_terminal`) stay in the Tauri
//! shell. Only engine-eligible git/shell operations live here.

use crate::error::{CoreError, CoreResult};
use houston_policy::SessionPolicy;
use houston_sandbox::wrap_sandbox;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::process::Command;

// ── DTOs ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeInfo {
    pub path: String,
    pub branch: String,
    pub is_main: bool,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorktreeRequest {
    pub repo_path: String,
    pub name: String,
    pub branch: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RemoveWorktreeRequest {
    pub repo_path: String,
    pub worktree_path: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ListWorktreesRequest {
    pub repo_path: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RunShellRequest {
    /// Agent root used to build the sandbox policy (e.g. `~/.houston/workspaces/Ws/Agent`).
    pub agent_path: String,
    /// Working directory for the shell command. Must resolve under `agent_path`.
    pub path: String,
    pub command: String,
}

// ── Helpers ────────────────────────────────────────────────────────

fn expand_tilde(path: &Path) -> PathBuf {
    if path.starts_with("~") {
        if let Some(home) = dirs::home_dir() {
            return home.join(path.strip_prefix("~").unwrap_or(path));
        }
    }
    path.to_path_buf()
}

/// Derive the worktrees directory adjacent to the repo.
/// `/Users/me/my-app` → `/Users/me/my-app-worktrees`.
fn worktrees_dir(repo_path: &Path) -> PathBuf {
    let name = repo_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "project".to_string());
    repo_path.with_file_name(format!("{name}-worktrees"))
}

// ── Git worktree ──────────────────────────────────────────────────

pub async fn create_worktree(req: CreateWorktreeRequest) -> CoreResult<WorktreeInfo> {
    let repo = expand_tilde(&PathBuf::from(&req.repo_path));
    let wt_dir = worktrees_dir(&repo);
    let wt_path = wt_dir.join(&req.name);

    if wt_path.exists() {
        return Err(CoreError::Conflict(format!(
            "worktree already exists: {}",
            wt_path.display()
        )));
    }

    std::fs::create_dir_all(&wt_dir)?;

    let branch_name = req.branch.unwrap_or_else(|| format!("houston/{}", req.name));

    let output = Command::new("git")
        .args(["worktree", "add", "-b", &branch_name])
        .arg(&wt_path)
        .current_dir(&repo)
        .output()
        .await
        .map_err(|e| CoreError::Internal(format!("failed to run git worktree add: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(CoreError::BadRequest(format!(
            "git worktree add failed: {stderr}"
        )));
    }

    Ok(WorktreeInfo {
        path: wt_path.to_string_lossy().to_string(),
        branch: branch_name,
        is_main: false,
    })
}

pub async fn remove_worktree(req: RemoveWorktreeRequest) -> CoreResult<()> {
    let repo = expand_tilde(&PathBuf::from(&req.repo_path));
    let output = Command::new("git")
        .args(["worktree", "remove", "--force", &req.worktree_path])
        .current_dir(&repo)
        .output()
        .await
        .map_err(|e| CoreError::Internal(format!("failed to run git worktree remove: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(CoreError::BadRequest(format!(
            "git worktree remove failed: {stderr}"
        )));
    }
    Ok(())
}

pub async fn list_worktrees(req: ListWorktreesRequest) -> CoreResult<Vec<WorktreeInfo>> {
    let repo = expand_tilde(&PathBuf::from(&req.repo_path));
    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(&repo)
        .output()
        .await
        .map_err(|e| CoreError::Internal(format!("failed to run git worktree list: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(CoreError::BadRequest(format!(
            "git worktree list failed: {stderr}"
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let canonical_repo = std::fs::canonicalize(&repo).unwrap_or_else(|_| repo.clone());
    let is_main_path = |p: &str| -> bool {
        let pb = PathBuf::from(p);
        let canonical = std::fs::canonicalize(&pb).unwrap_or(pb);
        canonical == canonical_repo
    };

    let mut worktrees = Vec::new();
    let mut current_path = String::new();
    let mut current_branch = String::new();
    let mut is_bare = false;

    for line in stdout.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            current_path = path.to_string();
            current_branch.clear();
            is_bare = false;
        } else if let Some(branch) = line.strip_prefix("branch refs/heads/") {
            current_branch = branch.to_string();
        } else if line == "bare" {
            is_bare = true;
        } else if line.is_empty() && !current_path.is_empty() {
            if !is_bare {
                let is_main = is_main_path(&current_path);
                worktrees.push(WorktreeInfo {
                    path: current_path.clone(),
                    branch: current_branch.clone(),
                    is_main,
                });
            }
            current_path.clear();
        }
    }
    if !current_path.is_empty() && !is_bare {
        let is_main = is_main_path(&current_path);
        worktrees.push(WorktreeInfo {
            path: current_path,
            branch: current_branch,
            is_main,
        });
    }

    Ok(worktrees)
}

// ── Shell ─────────────────────────────────────────────────────────

pub async fn run_shell(req: RunShellRequest) -> CoreResult<String> {
    let agent_root = expand_tilde(&PathBuf::from(&req.agent_path));
    if !agent_root.is_dir() {
        return Err(CoreError::NotFound(format!(
            "agent directory does not exist: {}",
            agent_root.display()
        )));
    }

    let dir = expand_tilde(&PathBuf::from(&req.path));
    if !dir.exists() {
        return Err(CoreError::NotFound(format!(
            "directory does not exist: {}",
            dir.display()
        )));
    }

    let policy = SessionPolicy::for_working_dir(agent_root.clone(), None);
    if !policy.allows_path(&dir) {
        return Err(CoreError::PermissionDenied(format!(
            "shell cwd {} is outside agent root {}",
            dir.display(),
            agent_root.display()
        )));
    }

    // Run through the platform shell so pipes, globs, and builtins work. There
    // is no `sh` on Windows, so use `cmd /C`; everywhere else use `sh -c`.
    #[cfg(windows)]
    let mut command = {
        let mut c = Command::new("cmd");
        c.args(["/C", &req.command]);
        c
    };
    #[cfg(not(windows))]
    let mut command = {
        let mut c = Command::new("sh");
        c.args(["-c", &req.command]);
        c
    };

    command.current_dir(&dir).env(
        "PATH",
        houston_terminal_manager::claude_path::shell_path(),
    );

    let mut command =
        wrap_sandbox(command, &policy).map_err(|e| CoreError::Unavailable(e.to_string()))?;

    let output = command
        .output()
        .await
        .map_err(|e| CoreError::Internal(format!("failed to run command: {e}")))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        return Err(CoreError::BadRequest(format!("{stderr}\n{stdout}")));
    }

    Ok(stdout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Unit tests exercise shell routing, not OS sandbox availability in CI.
    static SANDBOX_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct SandboxOff {
        _lock: std::sync::MutexGuard<'static, ()>,
    }

    impl SandboxOff {
        fn new() -> Self {
            let lock = SANDBOX_TEST_LOCK.lock().expect("sandbox test lock");
            std::env::set_var("HOUSTON_SANDBOX", "off");
            Self { _lock: lock }
        }
    }

    impl Drop for SandboxOff {
        fn drop(&mut self) {
            std::env::remove_var("HOUSTON_SANDBOX");
        }
    }

    async fn git(dir: &Path, args: &[&str]) {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .await
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    }

    async fn init_repo(tmp: &TempDir) -> PathBuf {
        let repo = tmp.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        git(&repo, &["init", "-b", "main"]).await;
        git(&repo, &["config", "user.email", "test@test.test"]).await;
        git(&repo, &["config", "user.name", "Test"]).await;
        git(&repo, &["config", "commit.gpgsign", "false"]).await;
        std::fs::write(repo.join("README.md"), "hello").unwrap();
        git(&repo, &["add", "."]).await;
        git(&repo, &["commit", "-m", "init"]).await;
        repo
    }

    #[tokio::test]
    async fn create_list_remove_worktree() {
        let tmp = TempDir::new().unwrap();
        let repo = init_repo(&tmp).await;
        let repo_str = repo.to_string_lossy().to_string();

        let info = create_worktree(CreateWorktreeRequest {
            repo_path: repo_str.clone(),
            name: "feature-x".into(),
            branch: None,
        })
        .await
        .unwrap();
        assert_eq!(info.branch, "houston/feature-x");
        assert!(!info.is_main);
        assert!(PathBuf::from(&info.path).exists());

        let list = list_worktrees(ListWorktreesRequest { repo_path: repo_str.clone() })
            .await
            .unwrap();
        // Two entries: the main repo + the new worktree.
        assert_eq!(list.len(), 2);
        assert!(list.iter().any(|w| w.is_main));
        assert!(list.iter().any(|w| w.branch == "houston/feature-x"));

        // Duplicate create → Conflict.
        let err = create_worktree(CreateWorktreeRequest {
            repo_path: repo_str.clone(),
            name: "feature-x".into(),
            branch: None,
        })
        .await
        .unwrap_err();
        assert!(matches!(err, CoreError::Conflict(_)));

        remove_worktree(RemoveWorktreeRequest {
            repo_path: repo_str.clone(),
            worktree_path: info.path.clone(),
        })
        .await
        .unwrap();
        assert!(!PathBuf::from(&info.path).exists());
    }

    #[tokio::test]
    async fn run_shell_echo() {
        let _sandbox_off = SandboxOff::new();
        let tmp = TempDir::new().unwrap();
        let agent = tmp.path().to_string_lossy().to_string();
        let out = run_shell(RunShellRequest {
            agent_path: agent.clone(),
            path: agent,
            command: "echo hello".into(),
        })
        .await
        .unwrap();
        assert_eq!(out.trim(), "hello");
    }

    #[tokio::test]
    async fn run_shell_missing_dir() {
        let _sandbox_off = SandboxOff::new();
        let tmp = TempDir::new().unwrap();
        let agent = tmp.path().to_string_lossy().to_string();
        let err = run_shell(RunShellRequest {
            agent_path: agent,
            path: "/definitely/not/a/real/path/zz".into(),
            command: "echo x".into(),
        })
        .await
        .unwrap_err();
        assert!(matches!(err, CoreError::NotFound(_)));
    }

    #[tokio::test]
    async fn run_shell_nonzero_exits_as_bad_request() {
        let _sandbox_off = SandboxOff::new();
        let tmp = TempDir::new().unwrap();
        let agent = tmp.path().to_string_lossy().to_string();
        let err = run_shell(RunShellRequest {
            agent_path: agent.clone(),
            path: agent,
            command: "false".into(),
        })
        .await
        .unwrap_err();
        assert!(matches!(err, CoreError::BadRequest(_)));
    }

    #[tokio::test]
    async fn shell_denied_outside_agent_root() {
        let _sandbox_off = SandboxOff::new();
        let tmp = TempDir::new().unwrap();
        let marketing = tmp.path().join("Marketing");
        let contabilidad = tmp.path().join("Contabilidad");
        std::fs::create_dir_all(&marketing).unwrap();
        std::fs::create_dir_all(&contabilidad).unwrap();

        let err = run_shell(RunShellRequest {
            agent_path: marketing.to_string_lossy().to_string(),
            path: contabilidad.to_string_lossy().to_string(),
            command: "echo x".into(),
        })
        .await
        .unwrap_err();
        assert!(matches!(err, CoreError::PermissionDenied(_)));
    }

    #[tokio::test]
    async fn shell_allowed_inside_agent_root() {
        let _sandbox_off = SandboxOff::new();
        let tmp = TempDir::new().unwrap();
        let agent = tmp.path().join("Marketing");
        let sub = agent.join("subdir");
        std::fs::create_dir_all(&sub).unwrap();
        let agent_str = agent.to_string_lossy().to_string();

        let out = run_shell(RunShellRequest {
            agent_path: agent_str.clone(),
            path: sub.to_string_lossy().to_string(),
            command: "echo ok".into(),
        })
        .await
        .unwrap();
        assert_eq!(out.trim(), "ok");
    }
}
