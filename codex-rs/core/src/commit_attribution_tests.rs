use super::configure_git_hooks_env;
use super::configure_git_hooks_env_for_config;
use super::injected_git_config_env;
use super::resolve_attribution_value;
use crate::config::test_config;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::fs;
use tempfile::tempdir;

#[test]
fn blank_attribution_disables_hook_env_injection() {
    let tmp = tempdir().expect("create temp dir");
    let mut env = HashMap::new();

    configure_git_hooks_env(&mut env, tmp.path(), Some(""));

    assert!(env.is_empty());
}

#[test]
fn default_attribution_uses_codex_trailer() {
    let tmp = tempdir().expect("create temp dir");
    let mut env = HashMap::new();

    configure_git_hooks_env(&mut env, tmp.path(), None);

    let hook_path = tmp
        .path()
        .join("hooks")
        .join("commit-attribution")
        .join("prepare-commit-msg");
    let script = fs::read_to_string(hook_path).expect("read generated hook");

    assert_eq!(env.get("GIT_CONFIG_COUNT"), Some(&"1".to_string()));
    assert_eq!(
        env.get("GIT_CONFIG_KEY_0"),
        Some(&"core.hooksPath".to_string())
    );
    assert!(script.contains("Co-authored-by=Codex <noreply@openai.com>"));
    assert!(script.contains("existing_hook=\"$existing_hooks_path/prepare-commit-msg\""));
}

#[test]
fn generated_hooks_preserve_other_commit_hooks() {
    let tmp = tempdir().expect("create temp dir");
    let mut env = HashMap::new();

    configure_git_hooks_env(&mut env, tmp.path(), None);

    let hooks_dir = tmp.path().join("hooks").join("commit-attribution");
    for hook_name in [
        "applypatch-msg",
        "commit-msg",
        "post-commit",
        "pre-applypatch",
        "pre-commit",
        "pre-merge-commit",
        "prepare-commit-msg",
    ] {
        let script = fs::read_to_string(hooks_dir.join(hook_name)).expect("read generated hook");
        assert!(script.contains(&format!(
            "existing_hook=\"$existing_hooks_path/{hook_name}\""
        )));
    }
}

#[test]
fn resolve_value_handles_default_custom_and_blank() {
    assert_eq!(
        resolve_attribution_value(None),
        Some("Codex <noreply@openai.com>".to_string())
    );
    assert_eq!(
        resolve_attribution_value(Some("MyAgent <me@example.com>")),
        Some("MyAgent <me@example.com>".to_string())
    );
    assert_eq!(
        resolve_attribution_value(Some("MyAgent")),
        Some("MyAgent".to_string())
    );
    assert_eq!(resolve_attribution_value(Some("   ")), None);
}

#[test]
fn custom_attribution_writes_custom_hook_script() {
    let tmp = tempdir().expect("create temp dir");
    let mut env = HashMap::new();

    configure_git_hooks_env(&mut env, tmp.path(), Some("AgentX <agent@example.com>"));

    let hook_path = tmp
        .path()
        .join("hooks")
        .join("commit-attribution")
        .join("prepare-commit-msg");
    let script = fs::read_to_string(hook_path).expect("read generated hook");

    assert!(script.contains("Co-authored-by=AgentX <agent@example.com>"));
    assert!(script.contains("existing_hook=\"$existing_hooks_path/prepare-commit-msg\""));
    assert!(script.contains("\"$existing_hook\" \"$@\""));
    let pre_commit = fs::read_to_string(
        tmp.path()
            .join("hooks")
            .join("commit-attribution")
            .join("pre-commit"),
    )
    .expect("read generated pre-commit hook");
    assert!(!pre_commit.contains("Co-authored-by=AgentX <agent@example.com>"));
}

#[test]
fn helper_configures_env_from_config() {
    let tmp = tempdir().expect("create temp dir");
    let mut config = test_config();
    config.codex_home = tmp.path().to_path_buf();
    config.commit_attribution = Some("AgentX <agent@example.com>".to_string());
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
            .contains("commit-attribution")
    );
}

#[test]
fn injected_git_config_env_returns_sorted_runtime_overrides() {
    let env = HashMap::from([
        ("IGNORED".to_string(), "value".to_string()),
        ("GIT_CONFIG_VALUE_1".to_string(), "beta".to_string()),
        ("GIT_CONFIG_KEY_0".to_string(), "core.hooksPath".to_string()),
        ("GIT_CONFIG_COUNT".to_string(), "2".to_string()),
        ("GIT_CONFIG_VALUE_0".to_string(), "/tmp/hooks".to_string()),
    ]);

    assert_eq!(
        injected_git_config_env(&env),
        vec![
            ("GIT_CONFIG_COUNT".to_string(), "2".to_string()),
            ("GIT_CONFIG_KEY_0".to_string(), "core.hooksPath".to_string()),
            ("GIT_CONFIG_VALUE_0".to_string(), "/tmp/hooks".to_string()),
            ("GIT_CONFIG_VALUE_1".to_string(), "beta".to_string()),
        ]
    );
}
