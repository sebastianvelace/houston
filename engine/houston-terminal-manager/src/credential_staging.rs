//! Per-session HOME staging for Claude and Codex CLI subprocesses.
//!
//! Mirrors the Gemini pattern in `gemini_home.rs`: spawn with `HOME` pointed
//! at a Houston-managed runtime directory containing only symlinks to the
//! minimum auth files. Cross-session project memory under `~/.claude/projects`
//! and `~/.codex/sessions` is excluded.

use crate::gemini_home::{houston_data_root, resolve_real_home};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum StagingError {
    Io(io::Error),
}

impl fmt::Display for StagingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for StagingError {}

impl From<io::Error> for StagingError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

/// RAII guard — removes the staged HOME tree on drop.
pub struct StagedHome {
    path: PathBuf,
}

impl StagedHome {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for StagedHome {
    fn drop(&mut self) {
        if let Err(e) = cleanup_staged_home(&self.path) {
            tracing::warn!(
                path = %self.path.display(),
                "failed to remove staged runtime home: {e}"
            );
        }
    }
}

pub fn stage_claude_home(session_key: &str) -> Result<StagedHome, StagingError> {
    let real_home = resolve_real_home()?;
    let runtime_home = staging_dir("claude-home", session_key);
    prepare_runtime_home(&runtime_home)?;

    let runtime_claude = runtime_home.join(".claude");
    fs::create_dir_all(&runtime_claude)?;
    set_owner_only(&runtime_home)?;
    set_owner_only(&runtime_claude)?;

    let real_claude = real_home.join(".claude");
    ensure_symlink(
        &real_claude.join(".credentials.json"),
        &runtime_claude.join(".credentials.json"),
    )?;

    remove_if_exists(&runtime_claude.join("projects"))?;
    remove_if_exists(&runtime_home.join("CLAUDE.md"))?;

    Ok(StagedHome {
        path: runtime_home,
    })
}

pub fn stage_codex_home(session_key: &str) -> Result<StagedHome, StagingError> {
    let real_home = resolve_real_home()?;
    let runtime_home = staging_dir("codex-home", session_key);
    prepare_runtime_home(&runtime_home)?;

    let runtime_codex = runtime_home.join(".codex");
    fs::create_dir_all(&runtime_codex)?;
    set_owner_only(&runtime_home)?;
    set_owner_only(&runtime_codex)?;

    let real_codex = real_home.join(".codex");
    ensure_symlink(
        &real_codex.join("auth.json"),
        &runtime_codex.join("auth.json"),
    )?;

    write_if_changed(
        &runtime_codex.join("config.toml"),
        "[sessions]\narchive = false\n",
    )?;

    remove_if_exists(&runtime_codex.join("sessions"))?;

    Ok(StagedHome {
        path: runtime_home,
    })
}

pub fn cleanup_staged_home(path: &Path) -> io::Result<()> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}

fn staging_dir(provider: &str, session_key: &str) -> PathBuf {
    houston_data_root()
        .join("runtime")
        .join(provider)
        .join(sanitize_session_key(session_key))
}

fn sanitize_session_key(session_key: &str) -> String {
    session_key
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn prepare_runtime_home(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)
}

fn remove_if_exists(path: &Path) -> io::Result<()> {
    if fs::symlink_metadata(path).is_ok() {
        if path.is_dir() {
            fs::remove_dir_all(path)?;
        } else {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn set_owner_only(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn set_owner_only(_path: &Path) -> io::Result<()> {
    Ok(())
}

fn tmp_sibling(path: &Path) -> io::Result<PathBuf> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no parent"))?;
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?;
    let mut tmp = std::ffi::OsString::from(".");
    tmp.push(name);
    tmp.push(".houston-tmp");
    Ok(parent.join(tmp))
}

#[cfg(unix)]
fn ensure_symlink(target: &Path, link: &Path) -> io::Result<()> {
    use std::os::unix::fs::symlink;
    let tmp = tmp_sibling(link)?;
    let _ = fs::remove_file(&tmp);
    symlink(target, &tmp)?;
    fs::rename(&tmp, link)
}

#[cfg(windows)]
fn ensure_symlink(target: &Path, link: &Path) -> io::Result<()> {
    use std::os::windows::fs::symlink_file;
    let tmp = tmp_sibling(link)?;
    let _ = fs::remove_file(&tmp);
    if symlink_file(target, &tmp).is_ok() {
        return fs::rename(&tmp, link);
    }
    match fs::copy(target, &tmp) {
        Ok(_) => fs::rename(&tmp, link),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let _ = fs::remove_file(link);
            Ok(())
        }
        Err(e) => Err(e),
    }
}

fn write_if_changed(path: &Path, content: &str) -> io::Result<()> {
    if let Ok(existing) = fs::read_to_string(path) {
        if existing == content {
            return Ok(());
        }
    }
    let tmp = tmp_sibling(path)?;
    let _ = fs::remove_file(&tmp);
    fs::write(&tmp, content)?;
    #[cfg(unix)]
    set_owner_only_file(&tmp)?;
    fs::rename(&tmp, path)
}

#[cfg(unix)]
fn set_owner_only_file(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn set_owner_only_file(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn with_houston_home<F: FnOnce()>(tmp: &TempDir, f: F) {
        std::env::set_var("HOUSTON_HOME", tmp.path());
        f();
        std::env::remove_var("HOUSTON_HOME");
    }

    #[test]
    fn staging_dir_does_not_contain_credentials() {
        let tmp = TempDir::new().unwrap();
        let real_home = tmp.path().join("real-home");
        let claude = real_home.join(".claude");
        fs::create_dir_all(&claude).unwrap();
        fs::write(
            claude.join(".credentials.json"),
            r#"{"claudeAiOauth":{"accessToken":"secret-token"}}"#,
        )
        .unwrap();

        std::env::set_var("HOME", &real_home);
        with_houston_home(&tmp, || {
            let staged = stage_claude_home("session-1").expect("stage claude");
            for entry in walk_files(staged.path()) {
                if entry.extension().is_some_and(|e| e == "json") && entry.is_file() {
                    let meta = fs::symlink_metadata(&entry).unwrap();
                    assert!(
                        meta.file_type().is_symlink(),
                        "credential paths must be symlinks, not copied plaintext: {}",
                        entry.display()
                    );
                }
            }
        });
        std::env::remove_var("HOME");
    }

    #[test]
    fn cleanup_removes_dir() {
        let tmp = TempDir::new().unwrap();
        let real_home = tmp.path().join("real-home");
        fs::create_dir_all(real_home.join(".claude")).unwrap();
        std::env::set_var("HOME", &real_home);
        with_houston_home(&tmp, || {
            let staged = stage_claude_home("cleanup-test").expect("stage");
            let path = staged.path().to_path_buf();
            drop(staged);
            assert!(!path.exists(), "cleanup must remove staging dir");
        });
        std::env::remove_var("HOME");
    }

    #[test]
    fn runner_uses_staged_home() {
        let tmp = TempDir::new().unwrap();
        let real_home = tmp.path().join("real-home");
        fs::create_dir_all(real_home.join(".codex")).unwrap();
        std::env::set_var("HOME", &real_home);
        with_houston_home(&tmp, || {
            let staged = stage_codex_home("runner-key").expect("stage codex");
            let mut cmd = tokio::process::Command::new("true");
            cmd.env("HOME", staged.path());
            let home_env = cmd
                .as_std()
                .get_envs()
                .find(|(key, _)| *key == "HOME")
                .and_then(|(_, value)| value.map(|v| v.to_os_string()));
            assert_eq!(home_env, Some(staged.path().as_os_str().to_os_string()));
        });
        std::env::remove_var("HOME");
    }

    fn walk_files(root: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    out.push(path);
                }
            }
        }
        out
    }
}
