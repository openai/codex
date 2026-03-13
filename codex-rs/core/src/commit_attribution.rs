use std::collections::HashMap;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;

use crate::config::Config;

const DEFAULT_ATTRIBUTION_VALUE: &str = "Codex <noreply@openai.com>";
const PREPARE_COMMIT_MSG_HOOK_NAME: &str = "prepare-commit-msg";
const COMMIT_HOOK_NAMES: &[&str] = &[
    "applypatch-msg",
    "commit-msg",
    "post-commit",
    "pre-applypatch",
    "pre-commit",
    "pre-merge-commit",
    PREPARE_COMMIT_MSG_HOOK_NAME,
];

pub(crate) fn configure_git_hooks_env_for_config(
    env: &mut HashMap<String, String>,
    config: &Config,
) {
    configure_git_hooks_env(
        env,
        config.codex_home.as_path(),
        config.commit_attribution.as_deref(),
    );
}

pub(crate) fn configure_git_hooks_env(
    env: &mut HashMap<String, String>,
    codex_home: &Path,
    config_attribution: Option<&str>,
) {
    let Some(value) = resolve_attribution_value(config_attribution) else {
        return;
    };

    let Ok(hooks_path) = ensure_codex_hook_scripts(codex_home, &value) else {
        return;
    };

    set_git_runtime_config(env, "core.hooksPath", hooks_path.to_string_lossy().as_ref());
}

pub(crate) fn injected_git_config_env(env: &HashMap<String, String>) -> Vec<(String, String)> {
    let mut pairs = env
        .iter()
        .filter(|(key, _)| {
            *key == "GIT_CONFIG_COUNT"
                || key.starts_with("GIT_CONFIG_KEY_")
                || key.starts_with("GIT_CONFIG_VALUE_")
        })
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<Vec<_>>();
    pairs.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
    pairs
}

fn resolve_attribution_value(config_attribution: Option<&str>) -> Option<String> {
    match config_attribution {
        Some(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        None => Some(DEFAULT_ATTRIBUTION_VALUE.to_string()),
    }
}

fn ensure_codex_hook_scripts(codex_home: &Path, value: &str) -> std::io::Result<PathBuf> {
    let hooks_dir = codex_home.join("hooks").join("commit-attribution");
    fs::create_dir_all(&hooks_dir)?;

    for hook_name in COMMIT_HOOK_NAMES {
        let script = build_hook_script(hook_name, value);
        let hook_path = hooks_dir.join(hook_name);
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
    }

    Ok(hooks_dir)
}

fn build_hook_script(hook_name: &str, value: &str) -> String {
    let escaped_value = value.replace('\'', "'\"'\"'");
    let prepare_commit_msg_body = if hook_name == PREPARE_COMMIT_MSG_HOOK_NAME {
        format!(
            "\nmsg_file=\"${{1:-}}\"\nif [[ -n \"$msg_file\" && -f \"$msg_file\" ]]; then\n  git interpret-trailers \\\n    --in-place \\\n    --if-exists doNothing \\\n    --if-missing add \\\n    --trailer 'Co-authored-by={escaped_value}' \\\n    \"$msg_file\" || true\nfi\n"
        )
    } else {
        String::new()
    };

    format!(
        "#!/usr/bin/env bash\nset -euo pipefail\n\nunset GIT_CONFIG_COUNT\nwhile IFS='=' read -r name _; do\n  case \"$name\" in\n    GIT_CONFIG_KEY_*|GIT_CONFIG_VALUE_*) unset \"$name\" ;;\n  esac\ndone < <(env)\n\nexisting_hooks_path=\"$(git config --path core.hooksPath 2>/dev/null || true)\"\nif [[ -z \"$existing_hooks_path\" ]]; then\n  git_dir=\"$(git rev-parse --git-common-dir 2>/dev/null || git rev-parse --git-dir 2>/dev/null || true)\"\n  if [[ -n \"$git_dir\" ]]; then\n    existing_hooks_path=\"$git_dir/hooks\"\n  fi\nfi\n\nif [[ -n \"$existing_hooks_path\" ]]; then\n  existing_hook=\"$existing_hooks_path/{hook_name}\"\n  if [[ -x \"$existing_hook\" && \"$existing_hook\" != \"$0\" ]]; then\n    \"$existing_hook\" \"$@\"\n  fi\nfi\n{prepare_commit_msg_body}"
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
#[path = "commit_attribution_tests.rs"]
mod tests;
