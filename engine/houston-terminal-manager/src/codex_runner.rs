//! Codex CLI session runner — counterpart of `claude_runner` for the
//! OpenAI / Codex provider.

use crate::cli_process::{run_cli_process, CliRunOutcome};
use crate::codex_command;
use crate::credential_staging::{stage_codex_home, StagedHome};
use crate::sandbox_cli_paths::enrich_policy_for_cli_spawn;
use crate::session_sandbox::apply_session_sandbox;
use crate::session_update::SessionUpdate;
use crate::types::SessionStatus;
use crate::Provider;
use houston_policy::SessionPolicy;
use tokio::process::Command;
use tokio::sync::mpsc;

/// Spawn a Codex CLI session (`codex exec --json --dangerously-bypass-approvals-and-sandbox`).
pub(crate) async fn spawn_codex(
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
) {
    let effort = effort.or_else(|| provider.default_effort().map(str::to_string));
    tracing::info!(
        "[houston:session] spawning codex exec --json (resume={:?}, model={:?}, effort={:?})",
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

    let cmd = build_codex_command(
        resume_session_id.as_deref(),
        working_dir.as_deref(),
        model.as_deref(),
        effort.as_deref(),
        system_prompt.as_deref(),
    );
    let Some((mut cmd, _staged_home)) =
        prepare_codex_spawn(tx, session_key, working_dir.as_deref(), cmd)
    else {
        return;
    };

    let outcome = run_cli_process(tx, &mut cmd, &prompt, provider).await;
    if outcome == CliRunOutcome::CodexResumeMissing && resume_session_id.is_some() {
        tracing::warn!("[houston:session] codex resume rollout missing; retrying with fresh thread");
        let _ = tx.send(SessionUpdate::ResumeInvalid);
        let fresh_cmd = build_codex_command(
            None,
            working_dir.as_deref(),
            model.as_deref(),
            effort.as_deref(),
            system_prompt.as_deref(),
        );
        let Some((mut fresh_cmd, _staged_home)) =
            prepare_codex_spawn(tx, session_key, working_dir.as_deref(), fresh_cmd)
        else {
            return;
        };
        run_cli_process(
            tx,
            &mut fresh_cmd,
            fresh_retry_prompt(&prompt, resume_fallback_prompt.as_deref()),
            provider,
        )
        .await;
    }
}

fn prepare_codex_spawn(
    tx: &mpsc::UnboundedSender<SessionUpdate>,
    session_key: &str,
    working_dir: Option<&std::path::Path>,
    cmd: Command,
) -> Option<(Command, StagedHome)> {
    let staged = match stage_codex_home(session_key) {
        Ok(home) => home,
        Err(e) => {
            let _ = tx.send(SessionUpdate::Status(SessionStatus::Error(format!(
                "Failed to prepare codex runtime home: {e}. \
                 Houston cannot spawn codex safely without it."
            ))));
            return None;
        }
    };

    let mut cmd = cmd;
    cmd.env("HOME", staged.path());
    cmd.env("CODEX_HOME", staged.path().join(".codex"));
    #[cfg(windows)]
    cmd.env("USERPROFILE", staged.path());

    let cmd = if let Some(dir) = working_dir {
        let codex_bin = houston_cli_bundle::bundled_codex_path()
            .unwrap_or_else(|| std::path::PathBuf::from("codex"));
        let policy = enrich_policy_for_cli_spawn(
            SessionPolicy::for_working_dir(dir.to_path_buf(), None),
            &codex_bin,
            staged.path(),
        );
        apply_session_sandbox(tx, cmd, &policy)?
    } else {
        cmd
    };

    Some((cmd, staged))
}

fn fresh_retry_prompt<'a>(prompt: &'a str, resume_fallback_prompt: Option<&'a str>) -> &'a str {
    resume_fallback_prompt.unwrap_or(prompt)
}

fn build_codex_command(
    resume_session_id: Option<&str>,
    working_dir: Option<&std::path::Path>,
    model: Option<&str>,
    effort: Option<&str>,
    system_prompt: Option<&str>,
) -> Command {
    let bin = houston_cli_bundle::bundled_codex_path()
        .unwrap_or_else(|| std::path::PathBuf::from("codex"));
    let mut cmd = Command::new(&bin);
    cmd.env("PATH", super::claude_path::shell_path());
    cmd.args(codex_command::build_args(
        resume_session_id,
        working_dir,
        model,
        effort,
        system_prompt,
    ));
    if let Some(dir) = working_dir {
        cmd.current_dir(dir);
    }
    cmd.env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "core.hooksPath")
        .env("GIT_CONFIG_VALUE_0", if cfg!(windows) { "NUL" } else { "/dev/null" });
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_retry_uses_recovery_prompt_when_available() {
        assert_eq!(
            fresh_retry_prompt("latest", Some("recovered history + latest")),
            "recovered history + latest"
        );
        assert_eq!(fresh_retry_prompt("latest", None), "latest");
    }
}
