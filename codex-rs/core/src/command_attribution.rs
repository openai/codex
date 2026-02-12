use std::collections::HashMap;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;

use crate::config::Config;

const DEFAULT_ATTRIBUTION_LABEL: &str = "Codex";
const PREPARE_COMMIT_MSG_HOOK_NAME: &str = "prepare-commit-msg";

pub(crate) fn configure_git_hooks_env_for_config(
    env: &mut HashMap<String, String>,
    config: &Config,
) {
    configure_git_hooks_env(
        env,
        config.codex_home.as_path(),
        config.command_attribution.as_deref(),
    );
}

pub(crate) fn configure_git_hooks_env(
    env: &mut HashMap<String, String>,
    codex_home: &Path,
    config_attribution: Option<&str>,
) {
    let Some(label) = resolve_attribution_label(config_attribution) else {
        return;
    };

    let Ok(hooks_path) = ensure_codex_hook_scripts(codex_home, &label) else {
        return;
    };

    set_git_runtime_config(env, "core.hooksPath", hooks_path.to_string_lossy().as_ref());
}

fn resolve_attribution_label(config_attribution: Option<&str>) -> Option<String> {
    match config_attribution {
        Some(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        None => Some(DEFAULT_ATTRIBUTION_LABEL.to_string()),
    }
}

fn ensure_codex_hook_scripts(codex_home: &Path, label: &str) -> std::io::Result<PathBuf> {
    let hooks_dir = codex_home.join("hooks").join("command-attribution");
    fs::create_dir_all(&hooks_dir)?;

    let script = build_hook_script(label);
    let hook_path = hooks_dir.join(PREPARE_COMMIT_MSG_HOOK_NAME);
    let should_write = match fs::read_to_string(&hook_path) {
        Ok(existing) => existing != script,
        Err(_) => true,
    };

    if should_write {
        fs::write(&hook_path, script.as_bytes())?;
    }

    #[cfg(unix)]
    {
        let mut perms = fs::metadata(&hook_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&hook_path, perms)?;
    }

    Ok(hooks_dir)
}

fn build_hook_script(label: &str) -> String {
    let escaped_label = label.replace('\'', "'\"'\"'");
    format!(
        "#!/usr/bin/env bash\nset -euo pipefail\n\nmsg_file=\"${{1:-}}\"\nif [[ -n \"$msg_file\" && -f \"$msg_file\" ]]; then\n  git interpret-trailers \\\n    --in-place \\\n    --if-exists doNothing \\\n    --if-missing add \\\n    --trailer 'Co-authored-by={escaped_label} <noreply@openai.com>' \\\n    \"$msg_file\" || true\nfi\n\nunset GIT_CONFIG_COUNT\nwhile IFS='=' read -r name _; do\n  case \"$name\" in\n    GIT_CONFIG_KEY_*|GIT_CONFIG_VALUE_*) unset \"$name\" ;;\n  esac\ndone < <(env)\n\nexisting_hooks_path=\"$(git config --path core.hooksPath 2>/dev/null || true)\"\nif [[ -z \"$existing_hooks_path\" ]]; then\n  git_dir=\"$(git rev-parse --git-common-dir 2>/dev/null || git rev-parse --git-dir 2>/dev/null || true)\"\n  if [[ -n \"$git_dir\" ]]; then\n    existing_hooks_path=\"$git_dir/hooks\"\n  fi\nfi\n\nif [[ -n \"$existing_hooks_path\" ]]; then\n  existing_hook=\"$existing_hooks_path/{PREPARE_COMMIT_MSG_HOOK_NAME}\"\n  if [[ -x \"$existing_hook\" && \"$existing_hook\" != \"$0\" ]]; then\n    \"$existing_hook\" \"$@\"\n  fi\nfi\n"
    )
}

fn set_git_runtime_config(env: &mut HashMap<String, String>, key: &str, value: &str) {
    let mut index = env
        .get("GIT_CONFIG_COUNT")
        .and_then(|count| count.parse::<usize>().ok())
        .unwrap_or(0);

    while env.contains_key(&format!("GIT_CONFIG_KEY_{index}"))
        || env.contains_key(&format!("GIT_CONFIG_VALUE_{index}"))
    {
        index += 1;
    }

    env.insert(format!("GIT_CONFIG_KEY_{index}"), key.to_string());
    env.insert(format!("GIT_CONFIG_VALUE_{index}"), value.to_string());
    env.insert("GIT_CONFIG_COUNT".to_string(), (index + 1).to_string());
}

#[cfg(test)]
mod tests {
    use super::configure_git_hooks_env;
    use super::configure_git_hooks_env_for_config;
    use super::resolve_attribution_label;
    use crate::config::test_config;
    use std::collections::HashMap;
    use tempfile::tempdir;

    #[test]
    fn blank_attribution_disables_hook_env_injection() {
        let tmp = tempdir().expect("create temp dir");
        let mut env = HashMap::new();

        configure_git_hooks_env(&mut env, tmp.path(), Some(""));

        assert!(env.is_empty());
    }

    #[test]
    fn default_attribution_injects_hooks_path() {
        let tmp = tempdir().expect("create temp dir");
        let mut env = HashMap::new();

        configure_git_hooks_env(&mut env, tmp.path(), None);

        assert_eq!(env.get("GIT_CONFIG_COUNT"), Some(&"1".to_string()));
        assert_eq!(
            env.get("GIT_CONFIG_KEY_0"),
            Some(&"core.hooksPath".to_string())
        );
        assert!(
            env.get("GIT_CONFIG_VALUE_0")
                .expect("missing hooks path")
                .contains("command-attribution")
        );
    }

    #[test]
    fn resolve_label_handles_default_custom_and_blank() {
        assert_eq!(resolve_attribution_label(None), Some("Codex".to_string()));
        assert_eq!(
            resolve_attribution_label(Some("MyAgent")),
            Some("MyAgent".to_string())
        );
        assert_eq!(resolve_attribution_label(Some("   ")), None);
    }

    #[test]
    fn helper_configures_env_from_config() {
        let tmp = tempdir().expect("create temp dir");
        let mut config = test_config();
        config.codex_home = tmp.path().to_path_buf();
        config.command_attribution = Some("AgentX".to_string());
        let mut env = HashMap::new();

        configure_git_hooks_env_for_config(&mut env, &config);

        assert_eq!(env.get("GIT_CONFIG_COUNT"), Some(&"1".to_string()));
        assert_eq!(
            env.get("GIT_CONFIG_KEY_0"),
            Some(&"core.hooksPath".to_string())
        );
        assert!(
            env.get("GIT_CONFIG_VALUE_0")
                .expect("missing hooks path")
                .contains("command-attribution")
        );
    }
}
