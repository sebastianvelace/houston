//! Seatbelt profile generation for the macOS sandbox backend.

use houston_policy::{houston_data_root, SessionPolicy};
use std::path::{Path, PathBuf};

/// Build a Seatbelt profile string for `sandbox_init` / `sandbox-exec`.
pub fn render_profile(policy: &SessionPolicy, real_home: &Path, houston_data: &Path) -> String {
    let agent_root = &policy.working_dir;
    let agent = escape_subpath(agent_root);
    let home = escape_subpath(real_home);
    let houston = escape_subpath(houston_data);
    let workspaces = escape_subpath(&houston_data.join("workspaces"));

    let mut profile = format!(
        "(version 1)\n\
(deny default)\n\
(allow file-read* file-write* (subpath \"{agent}\"))\n\
(allow file-read* (subpath \"/usr\") (subpath \"/bin\") (subpath \"/lib\") \
(subpath \"/private/tmp\") (subpath \"/private/var/folders\") \
(subpath \"/System\") (subpath \"/Library\") (subpath \"/Applications\"))\n\
(deny file-read* file-write* (subpath \"{workspaces}\"))\n\
(allow file-read* file-write* (subpath \"{agent}\"))\n\
(deny file-read* file-write* (subpath \"{home}/.claude\") (subpath \"{home}/.codex\") \
(subpath \"{home}/.gemini\"))\n\
(deny file-read* file-write* (subpath \"{houston}\"))\n\
(allow file-read* file-write* (subpath \"{agent}\"))\n\
(allow process*)\n"
    );

    for denied in &policy.denied_prefixes {
        if denied.starts_with(houston_data) {
            let sub = escape_subpath(denied);
            profile.push_str(&format!(
                "(deny file-read* file-write* (subpath \"{sub}\"))\n"
            ));
        }
    }

    profile
}

pub fn profile_inputs(_policy: &SessionPolicy) -> (PathBuf, PathBuf) {
    let real_home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    let houston_data = houston_data_root();
    (real_home, houston_data)
}

fn escape_subpath(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "\\\\")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn profile_contains_agent_root() {
        let agent = PathBuf::from("/Users/me/.houston/workspaces/Co/Marketing");
        let policy = SessionPolicy::for_working_dir(agent.clone(), Some(PathBuf::from("/Users/me/.houston/workspaces/Co")));
        let profile = render_profile(
            &policy,
            &PathBuf::from("/Users/me"),
            &PathBuf::from("/Users/me/.houston"),
        );
        assert!(profile.contains("/Users/me/.houston/workspaces/Co/Marketing"));
    }

    #[test]
    fn profile_denies_credential_paths() {
        let policy = SessionPolicy::for_working_dir(PathBuf::from("/tmp/agent"), None);
        let profile = render_profile(
            &policy,
            &PathBuf::from("/Users/me"),
            &PathBuf::from("/Users/me/.houston"),
        );
        assert!(profile.contains("/Users/me/.claude"));
        assert!(profile.contains("/Users/me/.codex"));
        assert!(profile.contains("/Users/me/.gemini"));
    }

    #[test]
    fn profile_denies_sibling_agents() {
        let agent = PathBuf::from("/Users/me/.houston/workspaces/Co/Marketing");
        let ws = PathBuf::from("/Users/me/.houston/workspaces/Co");
        let policy = SessionPolicy::for_working_dir(agent, Some(ws));
        let profile = render_profile(
            &policy,
            &PathBuf::from("/Users/me"),
            &PathBuf::from("/Users/me/.houston"),
        );
        assert!(profile.contains("/Users/me/.houston/workspaces"));
    }

    #[test]
    fn profile_inputs_use_policy_paths() {
        let policy = SessionPolicy::for_working_dir(PathBuf::from("/tmp/agent"), None);
        let (home, houston) = profile_inputs(&policy);
        assert!(home.exists() || home == PathBuf::from("/"));
        assert!(houston.ends_with(".houston") || houston.ends_with(".dev-houston"));
    }
}
