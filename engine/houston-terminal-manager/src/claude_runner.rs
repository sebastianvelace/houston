use super::types::{FeedItem, SessionStatus};
use crate::claude_install_path;
use crate::cli_process::{run_cli_process, CliRunOutcome};
use crate::credential_staging::{stage_claude_home, StagedHome};
use crate::provider_error::MALFORMED_PROVIDER_JSON_MESSAGE;
use crate::provider_error_kind::ProviderError;
use crate::sandbox_cli_paths::enrich_policy_for_cli_spawn;
use crate::session_sandbox::apply_session_sandbox;
use crate::session_update::SessionUpdate;
use crate::Provider;
use houston_policy::SessionPolicy;
use std::ffi::OsString;
use tokio::process::Command;
use tokio::sync::mpsc;

/// Absolute path to the Houston-managed `claude` if the runtime installer
/// dropped it (`~/.local/bin/claude` on Unix,
/// `%LOCALAPPDATA%\Programs\claude\claude.exe` on Windows). Falls back to
/// the bare name `"claude"` (PATH lookup) only when the installer hasn't
/// run yet, e.g. dev checkouts without `cli-deps.json`.
///
/// Spawning the absolute path matters: we pin a specific claude-code
/// version in `cli-deps.json` and pass flags
/// (`--include-partial-messages`, `--dangerously-skip-permissions`, ...)
/// that only newer versions support. PATH lookup can hit an older
/// `claude` from npm-global, homebrew, or a prior install, which then
/// rejects the flag with `error: unknown option '--include-partial-messages'`
/// and the session dies before producing any output.
fn claude_command_name() -> OsString {
    if claude_install_path::is_installed() {
        claude_install_path::cli_path().into_os_string()
    } else {
        OsString::from("claude")
    }
}

/// Spawn a Claude CLI session (`claude -p --output-format stream-json`).
#[allow(clippy::too_many_arguments)]
pub(crate) async fn spawn_claude(
    tx: &mpsc::UnboundedSender<SessionUpdate>,
    session_key: &str,
    provider: Provider,
    prompt: String,
    resume_session_id: Option<String>,
    resume_fallback_prompt: Option<String>,
    working_dir: Option<std::path::PathBuf>,
    model: Option<String>,
    effort: Option<String>,
    system_prompt: Option<String>,
    mcp_config: Option<std::path::PathBuf>,
    disable_builtin_tools: bool,
    disable_all_tools: bool,
) {
    tracing::info!(
        "[houston:session] spawning claude -p (resume={:?}, model={:?}, effort={:?})",
        resume_session_id,
        model,
        effort,
    );

    if let Some(ref dir) = working_dir {
        if !dir.is_dir() {
            let _ = tx.send(SessionUpdate::Status(SessionStatus::Error(format!(
                "Working directory not found: {}. Was it deleted?",
                dir.display()
            ))));
            return;
        }
    }

    let mut cmd = Command::new(claude_command_name());
    configure_claude_command(
        &mut cmd,
        resume_session_id.as_deref(),
        working_dir.as_deref(),
        model.as_deref(),
        effort.as_deref(),
        system_prompt.as_deref(),
        mcp_config.as_deref(),
        disable_builtin_tools,
        disable_all_tools,
    );
    let Some((mut cmd, _staged_home)) = prepare_claude_spawn(
        tx,
        session_key,
        working_dir.as_deref(),
        cmd,
    ) else {
        return;
    };
    let outcome = run_cli_process(tx, &mut cmd, &prompt, provider).await;
    if should_retry_fresh_after_resume_failure(outcome, resume_session_id.as_deref()) {
        tracing::warn!(
            "[houston:session] claude resume failed ({outcome:?}); retrying fresh"
        );
        let _ = tx.send(SessionUpdate::ResumeInvalid);
        let retry_prompt = fresh_retry_prompt(&prompt, resume_fallback_prompt.as_deref());
        retry_fresh(
            tx,
            session_key,
            provider,
            retry_prompt,
            working_dir.as_deref(),
            model.as_deref(),
            effort.as_deref(),
            system_prompt.as_deref(),
            mcp_config.as_deref(),
            disable_builtin_tools,
            disable_all_tools,
        )
        .await;
    } else if outcome == CliRunOutcome::ProviderRequestMalformedJson {
        // Malformed-JSON without a resume to clear: tell the user
        // explicitly so they can edit the prompt and try again.
        send_malformed_provider_json_status(tx);
    } else if outcome == CliRunOutcome::ClaudeResumeCorrupted {
        // Corrupted-resume signature fired but we had no `--resume` to
        // strip. That means claude itself bombed at startup for some
        // unrelated reason — surface a typed `SpawnFailed` so the user
        // sees a "Report bug" card instead of a silent hang.
        let _ = tx.send(SessionUpdate::Feed(FeedItem::ProviderError(
            ProviderError::SpawnFailed {
                provider: provider.id().to_string(),
                cli_name: provider.cli_name().to_string(),
                message: "claude exited at startup with error_during_execution".to_string(),
            },
        )));
        let _ = tx.send(SessionUpdate::Status(SessionStatus::Error(
            "claude failed to start".to_string(),
        )));
    }
}

#[allow(clippy::too_many_arguments)]
async fn retry_fresh(
    tx: &mpsc::UnboundedSender<SessionUpdate>,
    session_key: &str,
    provider: Provider,
    prompt: &str,
    working_dir: Option<&std::path::Path>,
    model: Option<&str>,
    effort: Option<&str>,
    system_prompt: Option<&str>,
    mcp_config: Option<&std::path::Path>,
    disable_builtin_tools: bool,
    disable_all_tools: bool,
) {
    let mut fresh_cmd = Command::new(claude_command_name());
    configure_claude_command(
        &mut fresh_cmd,
        None,
        working_dir,
        model,
        effort,
        system_prompt,
        mcp_config,
        disable_builtin_tools,
        disable_all_tools,
    );
    let Some((mut fresh_cmd, _staged_home)) = prepare_claude_spawn(
        tx,
        session_key,
        working_dir,
        fresh_cmd,
    ) else {
        return;
    };
    let retry_outcome = run_cli_process(tx, &mut fresh_cmd, prompt, provider).await;
    if retry_outcome == CliRunOutcome::ProviderRequestMalformedJson {
        send_malformed_provider_json_status(tx);
    } else if retry_outcome == CliRunOutcome::ClaudeResumeCorrupted {
        // Defensive: the fresh retry has no `--resume`, so the
        // corrupted-resume signature firing here means claude is
        // crashing at startup for an unrelated reason. cli_process
        // skipped its normal failed-exit emission, so surface the
        // failure ourselves rather than leaving the user staring at a
        // spinner.
        let _ = tx.send(SessionUpdate::Feed(FeedItem::ProviderError(
            ProviderError::SpawnFailed {
                provider: provider.id().to_string(),
                cli_name: provider.cli_name().to_string(),
                message: "claude exited at startup with error_during_execution".to_string(),
            },
        )));
        let _ = tx.send(SessionUpdate::Status(SessionStatus::Error(
            "claude failed to start".to_string(),
        )));
    }
}

fn prepare_claude_spawn(
    tx: &mpsc::UnboundedSender<SessionUpdate>,
    session_key: &str,
    working_dir: Option<&std::path::Path>,
    cmd: Command,
) -> Option<(Command, StagedHome)> {
    let staged = match stage_claude_home(session_key) {
        Ok(home) => home,
        Err(e) => {
            let _ = tx.send(SessionUpdate::Status(SessionStatus::Error(format!(
                "Failed to prepare claude runtime home: {e}. \
                 Houston cannot spawn claude safely without it."
            ))));
            return None;
        }
    };

    let mut cmd = cmd;
    cmd.env("HOME", staged.path());
    #[cfg(windows)]
    cmd.env("USERPROFILE", staged.path());

    let cmd = if let Some(dir) = working_dir {
        let policy = enrich_policy_for_cli_spawn(
            SessionPolicy::for_working_dir(dir.to_path_buf(), None),
            &claude_install_path::cli_path(),
            staged.path(),
        );
        apply_session_sandbox(tx, cmd, &policy)?
    } else {
        cmd
    };

    Some((cmd, staged))
}

#[allow(clippy::too_many_arguments)]
fn configure_claude_command(
    cmd: &mut Command,
    resume_session_id: Option<&str>,
    working_dir: Option<&std::path::Path>,
    model: Option<&str>,
    effort: Option<&str>,
    system_prompt: Option<&str>,
    mcp_config: Option<&std::path::Path>,
    disable_builtin_tools: bool,
    disable_all_tools: bool,
) {
    cmd.env("PATH", super::claude_path::shell_path());
    cmd.arg("-p")
        .arg("--output-format")
        .arg("stream-json")
        .arg("--verbose")
        .arg("--include-partial-messages");

    if disable_all_tools {
        cmd.arg("--allowedTools").arg("");
    } else {
        cmd.arg("--dangerously-skip-permissions");
        if disable_builtin_tools {
            cmd.arg("--disallowedTools")
                .arg("Edit")
                .arg("Write")
                .arg("NotebookEdit");
        }
    }

    if let Some(m) = model {
        cmd.arg("--model").arg(m);
    }
    if let Some(e) = effort {
        cmd.arg("--effort").arg(e);
    }
    if let Some(sp) = system_prompt {
        cmd.arg("--system-prompt").arg(sp);
    }
    if let Some(mcp) = mcp_config {
        cmd.arg("--mcp-config").arg(mcp);
    }
    if let Some(session_id) = resume_session_id {
        cmd.arg("--resume").arg(session_id);
    }

    cmd.env_remove("CLAUDE_CODE_ENTRYPOINT");
    cmd.env_remove("CLAUDECODE");

    // Neutralize git hook execution — a malicious hook in a repo the agent
    // clones or operates on could escape the session with arbitrary commands.
    // /dev/null on Unix, NUL on Windows: git treats a nonexistent hooksPath
    // as "no hooks" without erroring.
    cmd.env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "core.hooksPath")
        .env("GIT_CONFIG_VALUE_0", if cfg!(windows) { "NUL" } else { "/dev/null" });

    if let Some(dir) = working_dir {
        cmd.current_dir(dir);
    }
}

/// Two failure modes share the "retry without `--resume`" recovery path:
/// 1. `ProviderRequestMalformedJson` — Anthropic API rejected the resumed
///    transcript as having an unpaired UTF-16 surrogate (a single bad
///    emoji or pasted character anywhere in history poisons it forever).
/// 2. `ClaudeResumeCorrupted` — the on-disk transcript JSONL at
///    `~/.claude/projects/<encoded-cwd>/<session-id>.jsonl` is structurally
///    broken (truncated trailing line, dangling tool_use without
///    tool_result). The CLI crashes before contacting the API.
///
/// In both cases the cure is the same: clear the persisted session id,
/// re-spawn `claude -p` without `--resume`, and let the user continue.
/// Without a resume id there is nothing to strip — fall through to the
/// outer error-surfacing branches so the user still gets feedback.
fn should_retry_fresh_after_resume_failure(
    outcome: CliRunOutcome,
    resume_session_id: Option<&str>,
) -> bool {
    matches!(
        outcome,
        CliRunOutcome::ProviderRequestMalformedJson | CliRunOutcome::ClaudeResumeCorrupted
    ) && resume_session_id.is_some()
}

fn fresh_retry_prompt<'a>(prompt: &'a str, resume_fallback_prompt: Option<&'a str>) -> &'a str {
    resume_fallback_prompt.unwrap_or(prompt)
}

fn send_malformed_provider_json_status(tx: &mpsc::UnboundedSender<SessionUpdate>) {
    let _ = tx.send(SessionUpdate::Status(SessionStatus::Error(
        MALFORMED_PROVIDER_JSON_MESSAGE.to_string(),
    )));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retries_malformed_provider_json_only_for_resume() {
        assert!(should_retry_fresh_after_resume_failure(
            CliRunOutcome::ProviderRequestMalformedJson,
            Some("claude-session-id"),
        ));
        assert!(!should_retry_fresh_after_resume_failure(
            CliRunOutcome::ProviderRequestMalformedJson,
            None,
        ));
    }

    #[test]
    fn retries_corrupted_resume_only_when_resume_id_present() {
        assert!(should_retry_fresh_after_resume_failure(
            CliRunOutcome::ClaudeResumeCorrupted,
            Some("claude-session-id"),
        ));
        // No resume to strip — the runner surfaces a SpawnFailed card instead.
        assert!(!should_retry_fresh_after_resume_failure(
            CliRunOutcome::ClaudeResumeCorrupted,
            None,
        ));
    }

    #[test]
    fn does_not_retry_other_outcomes() {
        assert!(!should_retry_fresh_after_resume_failure(
            CliRunOutcome::Failed,
            Some("claude-session-id"),
        ));
        assert!(!should_retry_fresh_after_resume_failure(
            CliRunOutcome::Completed,
            Some("claude-session-id"),
        ));
        assert!(!should_retry_fresh_after_resume_failure(
            CliRunOutcome::CodexResumeMissing,
            Some("claude-session-id"),
        ));
    }

    #[test]
    fn fresh_retry_uses_recovery_prompt_when_available() {
        assert_eq!(
            fresh_retry_prompt("latest", Some("recovered history + latest")),
            "recovered history + latest"
        );
        assert_eq!(fresh_retry_prompt("latest", None), "latest");
    }
}
