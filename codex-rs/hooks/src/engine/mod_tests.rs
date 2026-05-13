use std::collections::HashMap;
use std::fs;
use std::path::Path;

use codex_config::AbsolutePathBuf;
use codex_config::ConfigLayerEntry;
use codex_config::ConfigLayerSource;
use codex_config::ConfigLayerStack;
use codex_config::ConfigRequirements;
use codex_config::ConfigRequirementsToml;
use codex_config::Constrained;
use codex_config::ConstrainedWithSource;
use codex_config::HookEventsToml;
use codex_config::HookHandlerConfig;
use codex_config::ManagedHooksRequirementsToml;
use codex_config::MatcherGroup;
use codex_config::RequirementSource;
use codex_config::Sourced;
use codex_config::TomlValue;
use codex_plugin::PluginHookSource;
use codex_plugin::PluginId;
use codex_protocol::ThreadId;
use codex_protocol::protocol::HookOutputEntry;
use codex_protocol::protocol::HookOutputEntryKind;
use codex_protocol::protocol::HookRunStatus;
use codex_protocol::protocol::HookSource;
use codex_protocol::protocol::HookTrustStatus;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

use super::ClaudeHooksEngine;
use super::CommandShell;
use crate::events::pre_tool_use::PreToolUseRequest;

fn cwd() -> AbsolutePathBuf {
    AbsolutePathBuf::current_dir().expect("current dir")
}

fn managed_hooks_for_current_platform(
    managed_dir: impl AsRef<Path>,
    hooks: HookEventsToml,
) -> ManagedHooksRequirementsToml {
    let managed_dir = managed_dir.as_ref().to_path_buf();
    ManagedHooksRequirementsToml {
        managed_dir: if cfg!(windows) {
            None
        } else {
            Some(managed_dir.clone())
        },
        windows_managed_dir: if cfg!(windows) {
            Some(managed_dir)
        } else {
            None
        },
        hooks,
    }
}

fn pre_tool_use_hook_events(command: impl Into<String>) -> HookEventsToml {
    HookEventsToml {
        pre_tool_use: vec![MatcherGroup {
            matcher: Some("^Bash$".to_string()),
            hooks: vec![HookHandlerConfig::Command {
                command: command.into(),
                command_windows: None,
                timeout_sec: Some(10),
                r#async: false,
                status_message: Some("checking".to_string()),
            }],
        }],
        ..Default::default()
    }
}

fn config_toml_with_pre_tool_use(command: &str) -> TomlValue {
    let mut config_toml = TomlValue::Table(Default::default());
    let TomlValue::Table(config_table) = &mut config_toml else {
        unreachable!("config TOML root should be a table");
    };
    let mut hooks_table = TomlValue::Table(Default::default());
    let TomlValue::Table(hooks_entries) = &mut hooks_table else {
        unreachable!("hooks entry should be a table");
    };
    let mut pre_tool_use_group = TomlValue::Table(Default::default());
    let TomlValue::Table(pre_tool_use_group_entries) = &mut pre_tool_use_group else {
        unreachable!("PreToolUse group should be a table");
    };
    pre_tool_use_group_entries.insert(
        "matcher".to_string(),
        TomlValue::String("^Bash$".to_string()),
    );
    let mut handler = TomlValue::Table(Default::default());
    let TomlValue::Table(handler_entries) = &mut handler else {
        unreachable!("PreToolUse handler should be a table");
    };
    handler_entries.insert("type".to_string(), TomlValue::String("command".to_string()));
    handler_entries.insert(
        "command".to_string(),
        TomlValue::String(command.to_string()),
    );
    handler_entries.insert("timeout".to_string(), TomlValue::Integer(10));
    handler_entries.insert(
        "statusMessage".to_string(),
        TomlValue::String("checking".to_string()),
    );
    pre_tool_use_group_entries.insert("hooks".to_string(), TomlValue::Array(vec![handler]));
    hooks_entries.insert(
        "PreToolUse".to_string(),
        TomlValue::Array(vec![pre_tool_use_group]),
    );
    config_table.insert("hooks".to_string(), hooks_table);
    config_toml
}

fn requirements_with_managed_hooks_only(
    allow_managed_hooks_only: bool,
    managed_hooks: Option<ManagedHooksRequirementsToml>,
) -> (ConfigRequirements, ConfigRequirementsToml) {
    (
        ConfigRequirements {
            allow_managed_hooks_only: Some(Sourced::new(
                allow_managed_hooks_only,
                RequirementSource::CloudRequirements,
            )),
            managed_hooks: managed_hooks.clone().map(|hooks| {
                ConstrainedWithSource::new(
                    Constrained::allow_any(hooks),
                    Some(RequirementSource::CloudRequirements),
                )
            }),
            ..ConfigRequirements::default()
        },
        ConfigRequirementsToml {
            allow_managed_hooks_only: Some(allow_managed_hooks_only),
            hooks: managed_hooks,
            ..ConfigRequirementsToml::default()
        },
    )
}

#[tokio::test]
async fn requirements_managed_hooks_execute_from_managed_dir() {
    let temp = tempdir().expect("create temp dir");
    let managed_dir =
        AbsolutePathBuf::try_from(temp.path().join("managed-hooks")).expect("absolute path");
    fs::create_dir_all(managed_dir.as_path()).expect("create managed hooks dir");
    let script_path = managed_dir.join("pre_tool_use.py");
    let log_path = managed_dir.join("pre_tool_use_log.jsonl");
    fs::write(
        script_path.as_path(),
        format!(
            r#"import json
from pathlib import Path
import sys

payload = json.load(sys.stdin)
with Path(r"{log_path}").open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(payload) + "\n")
"#,
            log_path = log_path.display(),
        ),
    )
    .expect("write managed hook script");

    let managed_hooks = managed_hooks_for_current_platform(
        managed_dir.clone(),
        HookEventsToml {
            pre_tool_use: vec![MatcherGroup {
                matcher: Some("^Bash$".to_string()),
                hooks: vec![HookHandlerConfig::Command {
                    command: format!("python3 {}", script_path.display()),
                    command_windows: None,
                    timeout_sec: Some(10),
                    r#async: false,
                    status_message: Some("checking".to_string()),
                }],
            }],
            ..Default::default()
        },
    );
    let config_layer_stack = ConfigLayerStack::new(
        Vec::new(),
        ConfigRequirements {
            managed_hooks: Some(ConstrainedWithSource::new(
                Constrained::allow_any(managed_hooks.clone()),
                Some(RequirementSource::CloudRequirements),
            )),
            ..ConfigRequirements::default()
        },
        ConfigRequirementsToml {
            hooks: Some(managed_hooks),
            ..ConfigRequirementsToml::default()
        },
    )
    .expect("config layer stack");

    let engine = ClaudeHooksEngine::new(
        /*enabled*/ true,
        Some(&config_layer_stack),
        Vec::new(),
        Vec::new(),
        CommandShell {
            program: String::new(),
            args: Vec::new(),
        },
    );

    assert!(engine.warnings().is_empty());
    assert_eq!(engine.handlers.len(), 1);
    assert_eq!(engine.handlers[0].source, HookSource::CloudRequirements);
    let listed = crate::list_hooks(crate::HooksConfig {
        legacy_notify_argv: None,
        feature_enabled: true,
        config_layer_stack: Some(config_layer_stack.clone()),
        plugin_hook_sources: Vec::new(),
        plugin_hook_load_warnings: Vec::new(),
        shell_program: None,
        shell_args: Vec::new(),
    });
    assert!(listed.hooks[0].is_managed);
    let cwd = cwd();
    let preview = engine.preview_pre_tool_use(&PreToolUseRequest {
        session_id: ThreadId::new(),
        turn_id: "turn-1".to_string(),
        cwd: cwd.clone(),
        transcript_path: None,
        model: "gpt-test".to_string(),
        permission_mode: "default".to_string(),
        tool_name: "Bash".to_string(),
        matcher_aliases: Vec::new(),
        tool_use_id: "tool-1".to_string(),
        tool_input: serde_json::json!({ "command": "echo hello" }),
    });
    assert_eq!(preview.len(), 1);
    assert_eq!(preview[0].source_path, managed_dir);

    let outcome = engine
        .run_pre_tool_use(PreToolUseRequest {
            session_id: ThreadId::new(),
            turn_id: "turn-1".to_string(),
            cwd,
            transcript_path: None,
            model: "gpt-test".to_string(),
            permission_mode: "default".to_string(),
            tool_name: "Bash".to_string(),
            matcher_aliases: Vec::new(),
            tool_use_id: "tool-1".to_string(),
            tool_input: serde_json::json!({ "command": "echo hello" }),
        })
        .await;

    assert!(!outcome.should_block);
    let log_contents = fs::read_to_string(log_path).expect("read managed hook log");
    assert!(log_contents.contains("\"hook_event_name\": \"PreToolUse\""));
}

#[tokio::test]
async fn requirements_managed_hooks_execute_windows_command_override() {
    let temp = tempdir().expect("create temp dir");
    let managed_dir =
        AbsolutePathBuf::try_from(temp.path().join("managed-hooks")).expect("absolute path");
    fs::create_dir_all(managed_dir.as_path()).expect("create managed hooks dir");

    let managed_hooks = managed_hooks_for_current_platform(
        managed_dir,
        HookEventsToml {
            pre_tool_use: vec![MatcherGroup {
                matcher: Some("^Bash$".to_string()),
                hooks: vec![HookHandlerConfig::Command {
                    command: "exit 17".to_string(),
                    command_windows: Some("exit /B 19".to_string()),
                    timeout_sec: Some(10),
                    r#async: false,
                    status_message: Some("checking".to_string()),
                }],
            }],
            ..Default::default()
        },
    );
    let config_layer_stack = ConfigLayerStack::new(
        Vec::new(),
        ConfigRequirements {
            managed_hooks: Some(ConstrainedWithSource::new(
                Constrained::allow_any(managed_hooks.clone()),
                Some(RequirementSource::CloudRequirements),
            )),
            ..ConfigRequirements::default()
        },
        ConfigRequirementsToml {
            hooks: Some(managed_hooks),
            ..ConfigRequirementsToml::default()
        },
    )
    .expect("config layer stack");

    let engine = ClaudeHooksEngine::new(
        /*enabled*/ true,
        Some(&config_layer_stack),
        Vec::new(),
        Vec::new(),
        CommandShell {
            program: String::new(),
            args: Vec::new(),
        },
    );

    let outcome = engine
        .run_pre_tool_use(PreToolUseRequest {
            session_id: ThreadId::new(),
            turn_id: "turn-1".to_string(),
            cwd: cwd(),
            transcript_path: None,
            model: "gpt-test".to_string(),
            permission_mode: "default".to_string(),
            tool_name: "Bash".to_string(),
            matcher_aliases: Vec::new(),
            tool_use_id: "tool-1".to_string(),
            tool_input: serde_json::json!({ "command": "echo hello" }),
        })
        .await;

    assert!(!outcome.should_block);
    let expected_exit_code = if cfg!(windows) { 19 } else { 17 };
    assert_eq!(outcome.hook_events.len(), 1);
    assert_eq!(outcome.hook_events[0].run.status, HookRunStatus::Failed);
    assert_eq!(
        outcome.hook_events[0].run.entries,
        vec![HookOutputEntry {
            kind: HookOutputEntryKind::Error,
            text: format!("hook exited with code {expected_exit_code}"),
        }]
    );
}

#[test]
fn unknown_requirement_source_hooks_stay_managed() {
    let temp = tempdir().expect("create temp dir");
    let managed_dir =
        AbsolutePathBuf::try_from(temp.path().join("managed-hooks")).expect("absolute path");
    fs::create_dir_all(managed_dir.as_path()).expect("create managed hooks dir");
    let managed_hooks = managed_hooks_for_current_platform(
        managed_dir,
        HookEventsToml {
            pre_tool_use: vec![MatcherGroup {
                matcher: Some("^Bash$".to_string()),
                hooks: vec![HookHandlerConfig::Command {
                    command: "python3 /tmp/managed.py".to_string(),
                    command_windows: None,
                    timeout_sec: Some(10),
                    r#async: false,
                    status_message: Some("checking".to_string()),
                }],
            }],
            ..Default::default()
        },
    );
    let config_layer_stack = ConfigLayerStack::new(
        Vec::new(),
        ConfigRequirements {
            managed_hooks: Some(ConstrainedWithSource::new(
                Constrained::allow_any(managed_hooks.clone()),
                Some(RequirementSource::Unknown),
            )),
            ..ConfigRequirements::default()
        },
        ConfigRequirementsToml {
            hooks: Some(managed_hooks),
            ..ConfigRequirementsToml::default()
        },
    )
    .expect("config layer stack");

    let engine = ClaudeHooksEngine::new(
        /*enabled*/ true,
        Some(&config_layer_stack),
        Vec::new(),
        Vec::new(),
        CommandShell {
            program: String::new(),
            args: Vec::new(),
        },
    );

    assert_eq!(engine.handlers.len(), 1);
    assert_eq!(engine.handlers[0].source, HookSource::Unknown);
    let discovered =
        super::discovery::discover_handlers(Some(&config_layer_stack), Vec::new(), Vec::new());
    assert_eq!(discovered.hook_entries.len(), 1);
    assert_eq!(discovered.hook_entries[0].source, HookSource::Unknown);
    assert_eq!(discovered.hook_entries[0].enabled, true);
    assert_eq!(discovered.hook_entries[0].is_managed, true);
    assert_eq!(
        discovered.hook_entries[0].trust_status,
        HookTrustStatus::Managed
    );
}

#[test]
fn user_disablement_filters_non_managed_hooks_but_not_managed_hooks() {
    let temp = tempdir().expect("create temp dir");
    let managed_dir =
        AbsolutePathBuf::try_from(temp.path().join("managed-hooks")).expect("absolute path");
    fs::create_dir_all(managed_dir.as_path()).expect("create managed hooks dir");
    let managed_hooks = managed_hooks_for_current_platform(
        managed_dir.clone(),
        HookEventsToml {
            pre_tool_use: vec![MatcherGroup {
                matcher: Some("^Bash$".to_string()),
                hooks: vec![HookHandlerConfig::Command {
                    command: "python3 /tmp/managed.py".to_string(),
                    command_windows: None,
                    timeout_sec: Some(10),
                    r#async: false,
                    status_message: Some("checking".to_string()),
                }],
            }],
            ..Default::default()
        },
    );
    let config_path =
        AbsolutePathBuf::try_from(temp.path().join("config.toml")).expect("absolute path");
    let managed_disabled_key = format!("{}:pre_tool_use:0:0", managed_dir.display());
    let user_disabled_key = format!("{}:pre_tool_use:0:0", config_path.display());
    let user_config = config_with_pre_tool_use_hook_and_states(
        "python3 /tmp/user.py",
        [&managed_disabled_key, &user_disabled_key],
    );
    let config_layer_stack = ConfigLayerStack::new(
        vec![ConfigLayerEntry::new(
            ConfigLayerSource::User { file: config_path },
            user_config,
        )],
        ConfigRequirements {
            managed_hooks: Some(ConstrainedWithSource::new(
                Constrained::allow_any(managed_hooks.clone()),
                Some(RequirementSource::CloudRequirements),
            )),
            ..ConfigRequirements::default()
        },
        ConfigRequirementsToml {
            hooks: Some(managed_hooks),
            ..ConfigRequirementsToml::default()
        },
    )
    .expect("config layer stack");

    let engine = ClaudeHooksEngine::new(
        /*enabled*/ true,
        Some(&config_layer_stack),
        Vec::new(),
        Vec::new(),
        CommandShell {
            program: String::new(),
            args: Vec::new(),
        },
    );

    assert_eq!(engine.handlers.len(), 1);
    assert_eq!(engine.handlers[0].source, HookSource::CloudRequirements);
    let discovered =
        super::discovery::discover_handlers(Some(&config_layer_stack), Vec::new(), Vec::new());
    assert_eq!(discovered.hook_entries.len(), 2);
    assert_eq!(discovered.hook_entries[0].key, managed_disabled_key);
    assert_eq!(discovered.hook_entries[0].enabled, true);
    assert!(discovered.hook_entries[0].is_managed);
    assert_eq!(
        discovered.hook_entries[0].trust_status,
        HookTrustStatus::Managed
    );
    assert_eq!(discovered.hook_entries[1].key, user_disabled_key);
    assert_eq!(discovered.hook_entries[1].enabled, false);
    assert!(!discovered.hook_entries[1].is_managed);
}

#[test]
fn user_disablement_does_not_filter_managed_layer_hooks() {
    let temp = tempdir().expect("create temp dir");
    let managed_config_path =
        AbsolutePathBuf::try_from(temp.path().join("managed_config.toml")).expect("absolute path");
    let user_config_path =
        AbsolutePathBuf::try_from(temp.path().join("config.toml")).expect("absolute path");
    let managed_key = format!("{}:pre_tool_use:0:0", managed_config_path.display());

    let config_layer_stack = ConfigLayerStack::new(
        vec![
            ConfigLayerEntry::new(
                ConfigLayerSource::User {
                    file: user_config_path,
                },
                config_with_hook_state(&managed_key, /*enabled*/ false),
            ),
            ConfigLayerEntry::new(
                ConfigLayerSource::LegacyManagedConfigTomlFromFile {
                    file: managed_config_path,
                },
                config_with_pre_tool_use_hook("python3 /tmp/managed-layer.py"),
            ),
        ],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("config layer stack");

    let engine = ClaudeHooksEngine::new(
        /*enabled*/ true,
        Some(&config_layer_stack),
        Vec::new(),
        Vec::new(),
        CommandShell {
            program: String::new(),
            args: Vec::new(),
        },
    );

    assert_eq!(engine.handlers.len(), 1);
    assert_eq!(
        engine.handlers[0].source,
        HookSource::LegacyManagedConfigFile
    );
    let discovered =
        super::discovery::discover_handlers(Some(&config_layer_stack), Vec::new(), Vec::new());
    assert_eq!(discovered.hook_entries.len(), 1);
    assert_eq!(discovered.hook_entries[0].key, managed_key);
    assert_eq!(discovered.hook_entries[0].enabled, true);
    assert!(discovered.hook_entries[0].is_managed);
    assert_eq!(
        discovered.hook_entries[0].trust_status,
        HookTrustStatus::Managed
    );
}

fn config_with_hook_state(key: &str, enabled: bool) -> TomlValue {
    serde_json::from_value(serde_json::json!({
        "hooks": {
            "state": {
                (key): {
                    "enabled": enabled,
                },
            },
        },
    }))
    .expect("config TOML should deserialize")
}

fn config_with_pre_tool_use_hook_and_states<const N: usize>(
    command: &str,
    disabled_keys: [&str; N],
) -> TomlValue {
    let state = disabled_keys
        .into_iter()
        .map(|key| (key.to_string(), serde_json::json!({ "enabled": false })))
        .collect::<serde_json::Map<_, _>>();
    serde_json::from_value(serde_json::json!({
        "hooks": {
            "state": state,
            "PreToolUse": [{
                "hooks": [{
                    "type": "command",
                    "command": command,
                }],
            }],
        },
    }))
    .expect("config TOML should deserialize")
}

fn config_with_pre_tool_use_hook(command: &str) -> TomlValue {
    serde_json::from_value(serde_json::json!({
        "hooks": {
            "PreToolUse": [{
                "hooks": [{
                    "type": "command",
                    "command": command,
                }],
            }],
        },
    }))
    .expect("config TOML should deserialize")
}

fn trusted_plugin_hook_stack(
    config_path: AbsolutePathBuf,
    plugin_hook_sources: &[PluginHookSource],
) -> ConfigLayerStack {
    let discovered = super::discovery::discover_handlers(
        /*config_layer_stack*/ None,
        plugin_hook_sources.to_vec(),
        Vec::new(),
    );
    let state = discovered
        .hook_entries
        .into_iter()
        .map(|entry| {
            (
                entry.key,
                serde_json::json!({
                    "trusted_hash": entry.current_hash,
                }),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    let config = serde_json::from_value(serde_json::json!({
        "hooks": {
            "state": state,
        },
    }))
    .expect("config TOML should deserialize");

    ConfigLayerStack::new(
        vec![ConfigLayerEntry::new(
            ConfigLayerSource::User { file: config_path },
            config,
        )],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("config layer stack")
}

#[test]
fn requirements_managed_hooks_load_when_managed_dir_is_missing() {
    let temp = tempdir().expect("create temp dir");
    let missing_dir = temp.path().join("missing-managed-hooks");
    let managed_hooks = managed_hooks_for_current_platform(
        missing_dir.clone(),
        HookEventsToml {
            pre_tool_use: vec![MatcherGroup {
                matcher: Some("^Bash$".to_string()),
                hooks: vec![HookHandlerConfig::Command {
                    command: "echo hi".to_string(),
                    command_windows: None,
                    timeout_sec: Some(10),
                    r#async: false,
                    status_message: Some("checking".to_string()),
                }],
            }],
            ..Default::default()
        },
    );
    let config_layer_stack = ConfigLayerStack::new(
        Vec::new(),
        ConfigRequirements {
            managed_hooks: Some(ConstrainedWithSource::new(
                Constrained::allow_any(managed_hooks.clone()),
                Some(RequirementSource::CloudRequirements),
            )),
            ..ConfigRequirements::default()
        },
        ConfigRequirementsToml {
            hooks: Some(managed_hooks),
            ..ConfigRequirementsToml::default()
        },
    )
    .expect("config layer stack");

    let engine = ClaudeHooksEngine::new(
        /*enabled*/ true,
        Some(&config_layer_stack),
        Vec::new(),
        Vec::new(),
        CommandShell {
            program: String::new(),
            args: Vec::new(),
        },
    );

    assert!(engine.warnings().is_empty());
    let cwd = cwd();
    let preview = engine.preview_pre_tool_use(&PreToolUseRequest {
        session_id: ThreadId::new(),
        turn_id: "turn-1".to_string(),
        cwd,
        transcript_path: None,
        model: "gpt-test".to_string(),
        permission_mode: "default".to_string(),
        tool_name: "Bash".to_string(),
        matcher_aliases: Vec::new(),
        tool_use_id: "tool-1".to_string(),
        tool_input: serde_json::json!({ "command": "echo hello" }),
    });
    assert_eq!(preview.len(), 1);
    assert_eq!(engine.handlers[0].command, "echo hi");
    assert_eq!(
        engine.handlers[0].source_path,
        AbsolutePathBuf::try_from(missing_dir).expect("absolute missing dir")
    );
}

#[test]
fn allow_managed_hooks_only_false_keeps_unmanaged_hooks() {
    let temp = tempdir().expect("create temp dir");
    let config_path =
        AbsolutePathBuf::try_from(temp.path().join("config.toml")).expect("absolute config path");
    let (requirements, requirements_toml) = requirements_with_managed_hooks_only(
        /*allow_managed_hooks_only*/ false, /*managed_hooks*/ None,
    );
    let config_layer_stack = ConfigLayerStack::new(
        vec![ConfigLayerEntry::new(
            ConfigLayerSource::User { file: config_path },
            config_toml_with_pre_tool_use("python3 /tmp/user-hook.py"),
        )],
        requirements,
        requirements_toml,
    )
    .expect("config layer stack");

    let engine = ClaudeHooksEngine::new(
        /*enabled*/ true,
        Some(&config_layer_stack),
        Vec::new(),
        Vec::new(),
        CommandShell {
            program: String::new(),
            args: Vec::new(),
        },
    );

    assert!(engine.warnings().is_empty());
    assert!(engine.handlers.is_empty());
    let discovered =
        super::discovery::discover_handlers(Some(&config_layer_stack), Vec::new(), Vec::new());
    assert_eq!(discovered.hook_entries.len(), 1);
    assert!(!discovered.hook_entries[0].is_managed);
    assert_eq!(
        discovered.hook_entries[0].command.as_deref(),
        Some("python3 /tmp/user-hook.py")
    );
}

#[test]
fn allow_managed_hooks_only_in_config_toml_does_not_enable_policy() {
    let temp = tempdir().expect("create temp dir");
    let config_path =
        AbsolutePathBuf::try_from(temp.path().join("config.toml")).expect("absolute config path");
    let mut config_toml = config_toml_with_pre_tool_use("python3 /tmp/user-hook.py");
    let TomlValue::Table(config_table) = &mut config_toml else {
        unreachable!("config TOML root should be a table");
    };
    config_table.insert(
        "allow_managed_hooks_only".to_string(),
        TomlValue::Boolean(true),
    );
    let config_layer_stack = ConfigLayerStack::new(
        vec![ConfigLayerEntry::new(
            ConfigLayerSource::User { file: config_path },
            config_toml,
        )],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("config layer stack");

    let engine = ClaudeHooksEngine::new(
        /*enabled*/ true,
        Some(&config_layer_stack),
        Vec::new(),
        Vec::new(),
        CommandShell {
            program: String::new(),
            args: Vec::new(),
        },
    );

    assert!(engine.warnings().is_empty());
    assert!(engine.handlers.is_empty());
    let discovered =
        super::discovery::discover_handlers(Some(&config_layer_stack), Vec::new(), Vec::new());
    assert_eq!(discovered.hook_entries.len(), 1);
    assert!(!discovered.hook_entries[0].is_managed);
    assert_eq!(
        discovered.hook_entries[0].command.as_deref(),
        Some("python3 /tmp/user-hook.py")
    );
}

#[test]
fn allow_managed_hooks_only_skips_unmanaged_json_and_toml_hooks() {
    let temp = tempdir().expect("create temp dir");
    let config_path =
        AbsolutePathBuf::try_from(temp.path().join("config.toml")).expect("absolute config path");
    let hooks_json_path =
        AbsolutePathBuf::try_from(temp.path().join("hooks.json")).expect("absolute hooks path");
    fs::write(
        hooks_json_path.as_path(),
        r#"{
              "hooks": {
                "PreToolUse": [
                  {
                    "matcher": "^Bash$",
                    "hooks": [
                      {
                        "type": "command",
                        "command": "python3 /tmp/json-hook.py"
                      }
                    ]
                  }
                ]
              }
            }"#,
    )
    .expect("write hooks.json");
    let (requirements, requirements_toml) = requirements_with_managed_hooks_only(
        /*allow_managed_hooks_only*/ true, /*managed_hooks*/ None,
    );
    let config_layer_stack = ConfigLayerStack::new(
        vec![ConfigLayerEntry::new(
            ConfigLayerSource::User { file: config_path },
            config_toml_with_pre_tool_use("python3 /tmp/toml-hook.py"),
        )],
        requirements,
        requirements_toml,
    )
    .expect("config layer stack");

    let engine = ClaudeHooksEngine::new(
        /*enabled*/ true,
        Some(&config_layer_stack),
        Vec::new(),
        Vec::new(),
        CommandShell {
            program: String::new(),
            args: Vec::new(),
        },
    );

    assert!(engine.handlers.is_empty());
    assert!(engine.warnings().is_empty());
}

#[test]
fn allow_managed_hooks_only_skips_unmanaged_plugin_hooks() {
    let temp = tempdir().expect("create temp dir");
    let plugin_root =
        AbsolutePathBuf::try_from(temp.path().join("demo-plugin")).expect("plugin root");
    let plugin_data_root =
        AbsolutePathBuf::try_from(temp.path().join("plugin-data")).expect("plugin data root");
    let source_path = plugin_root.join("hooks/hooks.json");
    let plugin_id = PluginId::parse("demo-plugin@test-marketplace").expect("plugin id");
    let plugin_hook_sources = vec![PluginHookSource {
        plugin_id,
        plugin_root,
        plugin_data_root,
        source_path,
        source_relative_path: "hooks/hooks.json".to_string(),
        hooks: pre_tool_use_hook_events("python3 /tmp/plugin-hook.py"),
    }];
    let (requirements, requirements_toml) = requirements_with_managed_hooks_only(
        /*allow_managed_hooks_only*/ true, /*managed_hooks*/ None,
    );
    let config_layer_stack = ConfigLayerStack::new(Vec::new(), requirements, requirements_toml)
        .expect("config layer stack");

    let engine = ClaudeHooksEngine::new(
        /*enabled*/ true,
        Some(&config_layer_stack),
        plugin_hook_sources,
        Vec::new(),
        CommandShell {
            program: String::new(),
            args: Vec::new(),
        },
    );

    assert!(engine.handlers.is_empty());
    assert!(engine.warnings().is_empty());
}

#[test]
fn allow_managed_hooks_only_keeps_managed_requirement_and_config_layer_hooks() {
    let temp = tempdir().expect("create temp dir");
    let managed_dir =
        AbsolutePathBuf::try_from(temp.path().join("managed-hooks")).expect("absolute path");
    fs::create_dir_all(managed_dir.as_path()).expect("create managed hooks dir");
    let system_config_path =
        AbsolutePathBuf::try_from(temp.path().join("system").join("config.toml"))
            .expect("absolute system config path");
    let system_parent = system_config_path
        .as_path()
        .parent()
        .expect("system config parent");
    fs::create_dir_all(system_parent).expect("create system config dir");
    let legacy_config_path = AbsolutePathBuf::try_from(temp.path().join("managed_config.toml"))
        .expect("absolute legacy config path");

    let managed_hooks = managed_hooks_for_current_platform(
        managed_dir,
        pre_tool_use_hook_events("python3 /tmp/requirements-hook.py"),
    );
    let (requirements, requirements_toml) = requirements_with_managed_hooks_only(
        /*allow_managed_hooks_only*/ true,
        Some(managed_hooks),
    );
    let config_layer_stack = ConfigLayerStack::new(
        vec![
            ConfigLayerEntry::new(
                ConfigLayerSource::Mdm {
                    domain: "com.openai.codex".to_string(),
                    key: "config".to_string(),
                },
                config_toml_with_pre_tool_use("python3 /tmp/mdm-hook.py"),
            ),
            ConfigLayerEntry::new(
                ConfigLayerSource::System {
                    file: system_config_path,
                },
                config_toml_with_pre_tool_use("python3 /tmp/system-hook.py"),
            ),
            ConfigLayerEntry::new(
                ConfigLayerSource::LegacyManagedConfigTomlFromFile {
                    file: legacy_config_path,
                },
                config_toml_with_pre_tool_use("python3 /tmp/legacy-file-hook.py"),
            ),
            ConfigLayerEntry::new(
                ConfigLayerSource::LegacyManagedConfigTomlFromMdm,
                config_toml_with_pre_tool_use("python3 /tmp/legacy-mdm-hook.py"),
            ),
        ],
        requirements,
        requirements_toml,
    )
    .expect("config layer stack");

    let engine = ClaudeHooksEngine::new(
        /*enabled*/ true,
        Some(&config_layer_stack),
        Vec::new(),
        Vec::new(),
        CommandShell {
            program: String::new(),
            args: Vec::new(),
        },
    );

    assert!(engine.warnings().is_empty());
    assert_eq!(
        engine
            .handlers
            .iter()
            .map(|handler| handler.command.as_str())
            .collect::<Vec<_>>(),
        vec![
            "python3 /tmp/requirements-hook.py",
            "python3 /tmp/mdm-hook.py",
            "python3 /tmp/system-hook.py",
            "python3 /tmp/legacy-file-hook.py",
            "python3 /tmp/legacy-mdm-hook.py",
        ]
    );
    let discovered =
        super::discovery::discover_handlers(Some(&config_layer_stack), Vec::new(), Vec::new());
    assert!(discovered.hook_entries.iter().all(|entry| entry.is_managed));
}

#[test]
fn discovers_hooks_from_json_and_toml_in_the_same_layer() {
    let temp = tempdir().expect("create temp dir");
    let config_path =
        AbsolutePathBuf::try_from(temp.path().join("config.toml")).expect("absolute config path");
    let hooks_json_path =
        AbsolutePathBuf::try_from(temp.path().join("hooks.json")).expect("absolute hooks path");
    fs::write(
        hooks_json_path.as_path(),
        r#"{
              "hooks": {
                "PreToolUse": [
                  {
                    "matcher": "^Bash$",
                    "hooks": [
                      {
                        "type": "command",
                        "command": "python3 /tmp/json-hook.py"
                      }
                    ]
                  }
                ]
              }
            }"#,
    )
    .expect("write hooks.json");
    let mut config_toml = TomlValue::Table(Default::default());
    let TomlValue::Table(config_table) = &mut config_toml else {
        unreachable!("config TOML root should be a table");
    };
    let mut hooks_table = TomlValue::Table(Default::default());
    let TomlValue::Table(hooks_entries) = &mut hooks_table else {
        unreachable!("hooks entry should be a table");
    };
    let mut pre_tool_use_group = TomlValue::Table(Default::default());
    let TomlValue::Table(pre_tool_use_group_entries) = &mut pre_tool_use_group else {
        unreachable!("PreToolUse group should be a table");
    };
    pre_tool_use_group_entries.insert(
        "matcher".to_string(),
        TomlValue::String("^Bash$".to_string()),
    );
    pre_tool_use_group_entries.insert(
        "hooks".to_string(),
        TomlValue::Array(vec![TomlValue::Table(Default::default())]),
    );
    let Some(TomlValue::Array(hooks_array)) = pre_tool_use_group_entries.get_mut("hooks") else {
        unreachable!("PreToolUse hooks should be an array");
    };
    let Some(TomlValue::Table(handler_entries)) = hooks_array.first_mut() else {
        unreachable!("PreToolUse handler should be a table");
    };
    handler_entries.insert("type".to_string(), TomlValue::String("command".to_string()));
    handler_entries.insert(
        "command".to_string(),
        TomlValue::String("python3 /tmp/toml-hook.py".to_string()),
    );
    hooks_entries.insert(
        "PreToolUse".to_string(),
        TomlValue::Array(vec![pre_tool_use_group]),
    );
    config_table.insert("hooks".to_string(), hooks_table);
    let config_layer_stack = ConfigLayerStack::new(
        vec![ConfigLayerEntry::new(
            ConfigLayerSource::System {
                file: config_path.clone(),
            },
            config_toml,
        )],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("config layer stack");

    let engine = ClaudeHooksEngine::new(
        /*enabled*/ true,
        Some(&config_layer_stack),
        Vec::new(),
        Vec::new(),
        CommandShell {
            program: String::new(),
            args: Vec::new(),
        },
    );

    assert!(engine.warnings().iter().any(|warning| {
        warning.contains("loading hooks from both")
            && warning.contains(&hooks_json_path.display().to_string())
            && warning.contains(&config_path.display().to_string())
    }));

    let cwd = cwd();
    let preview = engine.preview_pre_tool_use(&PreToolUseRequest {
        session_id: ThreadId::new(),
        turn_id: "turn-1".to_string(),
        cwd,
        transcript_path: None,
        model: "gpt-test".to_string(),
        permission_mode: "default".to_string(),
        tool_name: "Bash".to_string(),
        matcher_aliases: Vec::new(),
        tool_use_id: "tool-1".to_string(),
        tool_input: serde_json::json!({ "command": "echo hello" }),
    });
    assert_eq!(preview.len(), 2);
    assert_eq!(
        engine
            .handlers
            .iter()
            .map(|handler| handler.source)
            .collect::<Vec<_>>(),
        vec![HookSource::System, HookSource::System]
    );
    assert_eq!(preview[0].source_path, hooks_json_path);
    assert_eq!(preview[1].source_path, config_path);
}

#[tokio::test]
async fn plugin_hook_sources_run_with_plugin_env_and_plugin_source() {
    let temp = tempdir().expect("create temp dir");
    let plugin_root =
        AbsolutePathBuf::try_from(temp.path().join("demo-plugin")).expect("plugin root");
    let plugin_data_root =
        AbsolutePathBuf::try_from(temp.path().join("plugin-data")).expect("plugin data root");
    fs::create_dir_all(plugin_root.join("hooks")).expect("create hooks dir");
    let source_path = plugin_root.join("hooks/hooks.json");
    let script_path = plugin_root.join("hooks/write_env.py");
    fs::write(
        script_path.as_path(),
        r#"import json
import os
print(json.dumps({
    "systemMessage": json.dumps({
        "plugin": os.environ.get("PLUGIN_ROOT"),
        "claude": os.environ.get("CLAUDE_PLUGIN_ROOT"),
    })
}))
"#,
    )
    .expect("write hook script");
    let plugin_id = PluginId::parse("demo-plugin@test-marketplace").expect("plugin id");
    let plugin_hook_sources = vec![PluginHookSource {
        plugin_id,
        plugin_root: plugin_root.clone(),
        plugin_data_root: plugin_data_root.clone(),
        source_path: source_path.clone(),
        source_relative_path: "hooks/hooks.json".to_string(),
        hooks: HookEventsToml {
            pre_tool_use: vec![MatcherGroup {
                matcher: Some("Bash".to_string()),
                hooks: vec![HookHandlerConfig::Command {
                    command: format!("python3 {}", script_path.display()),
                    command_windows: None,
                    timeout_sec: Some(10),
                    r#async: false,
                    status_message: None,
                }],
            }],
            ..Default::default()
        },
    }];
    let config_layer_stack = trusted_plugin_hook_stack(
        AbsolutePathBuf::try_from(temp.path().join("config.toml")).expect("absolute config path"),
        &plugin_hook_sources,
    );
    let engine = ClaudeHooksEngine::new(
        /*enabled*/ true,
        Some(&config_layer_stack),
        plugin_hook_sources.clone(),
        Vec::new(),
        CommandShell {
            program: String::new(),
            args: Vec::new(),
        },
    );

    let preview = engine.preview_pre_tool_use(&PreToolUseRequest {
        session_id: ThreadId::new(),
        turn_id: "turn-1".to_string(),
        cwd: cwd(),
        transcript_path: None,
        model: "gpt-test".to_string(),
        permission_mode: "default".to_string(),
        tool_name: "Bash".to_string(),
        matcher_aliases: Vec::new(),
        tool_use_id: "tool-1".to_string(),
        tool_input: serde_json::json!({ "command": "echo hello" }),
    });
    assert_eq!(preview.len(), 1);
    assert_eq!(preview[0].source, HookSource::Plugin);
    assert_eq!(preview[0].source_path, source_path);
    let listed = crate::list_hooks(crate::HooksConfig {
        legacy_notify_argv: None,
        feature_enabled: true,
        config_layer_stack: None,
        plugin_hook_sources,
        plugin_hook_load_warnings: Vec::new(),
        shell_program: None,
        shell_args: Vec::new(),
    });
    assert_eq!(
        listed.hooks[0].plugin_id.as_deref(),
        Some("demo-plugin@test-marketplace")
    );

    let outcome = engine
        .run_pre_tool_use(PreToolUseRequest {
            session_id: ThreadId::new(),
            turn_id: "turn-1".to_string(),
            cwd: cwd(),
            transcript_path: None,
            model: "gpt-test".to_string(),
            permission_mode: "default".to_string(),
            tool_name: "Bash".to_string(),
            matcher_aliases: Vec::new(),
            tool_use_id: "tool-1".to_string(),
            tool_input: serde_json::json!({ "command": "echo hello" }),
        })
        .await;

    assert_eq!(outcome.hook_events.len(), 1);
    assert_eq!(outcome.hook_events[0].run.source, HookSource::Plugin);
    assert_eq!(outcome.hook_events[0].run.status, HookRunStatus::Completed);
    assert_eq!(outcome.hook_events[0].run.entries.len(), 1);
    assert_eq!(
        outcome.hook_events[0].run.entries[0].kind,
        HookOutputEntryKind::Warning
    );
    let logged: serde_json::Value =
        serde_json::from_str(&outcome.hook_events[0].run.entries[0].text)
            .expect("parse env payload");
    assert_eq!(
        logged,
        serde_json::json!({
            "plugin": plugin_root.display().to_string(),
            "claude": plugin_root.display().to_string(),
        })
    );
}

#[test]
fn plugin_hook_sources_expand_plugin_placeholders() {
    let temp = tempdir().expect("create temp dir");
    let plugin_root =
        AbsolutePathBuf::try_from(temp.path().join("demo-plugin")).expect("plugin root");
    let plugin_data_root =
        AbsolutePathBuf::try_from(temp.path().join("plugin-data")).expect("plugin data root");
    let source_path = plugin_root.join("hooks/hooks.json");
    let plugin_id = PluginId::parse("demo-plugin@test-marketplace").expect("plugin id");
    let plugin_hook_sources = vec![PluginHookSource {
        plugin_id,
        plugin_root: plugin_root.clone(),
        plugin_data_root: plugin_data_root.clone(),
        source_path,
        source_relative_path: "hooks/hooks.json".to_string(),
        hooks: HookEventsToml {
            pre_tool_use: vec![MatcherGroup {
                matcher: Some("Bash".to_string()),
                hooks: vec![HookHandlerConfig::Command {
                    command:
                        "run ${PLUGIN_ROOT} ${CLAUDE_PLUGIN_ROOT} ${PLUGIN_DATA} ${CLAUDE_PLUGIN_DATA}"
                            .to_string(),
                    command_windows: None,
                    timeout_sec: Some(5),
                    r#async: false,
                    status_message: None,
                }],
            }],
            ..Default::default()
        },
    }];
    let config_layer_stack = trusted_plugin_hook_stack(
        AbsolutePathBuf::try_from(temp.path().join("config.toml")).expect("absolute config path"),
        &plugin_hook_sources,
    );
    let engine = ClaudeHooksEngine::new(
        /*enabled*/ true,
        Some(&config_layer_stack),
        plugin_hook_sources,
        Vec::new(),
        CommandShell {
            program: String::new(),
            args: Vec::new(),
        },
    );

    assert_eq!(
        engine.handlers[0].command,
        format!(
            "run {} {} {} {}",
            plugin_root.display(),
            plugin_root.display(),
            plugin_data_root.display(),
            plugin_data_root.display()
        )
    );
    assert_eq!(
        engine.handlers[0].env,
        HashMap::from([
            ("PLUGIN_ROOT".to_string(), plugin_root.display().to_string()),
            (
                "CLAUDE_PLUGIN_ROOT".to_string(),
                plugin_root.display().to_string()
            ),
            (
                "PLUGIN_DATA".to_string(),
                plugin_data_root.display().to_string()
            ),
            (
                "CLAUDE_PLUGIN_DATA".to_string(),
                plugin_data_root.display().to_string()
            ),
        ])
    );
}

#[test]
fn plugin_hook_load_warnings_are_startup_warnings() {
    let engine = ClaudeHooksEngine::new(
        /*enabled*/ true,
        /*config_layer_stack*/ None,
        Vec::new(),
        vec!["failed plugin hook".to_string()],
        CommandShell {
            program: String::new(),
            args: Vec::new(),
        },
    );

    assert_eq!(engine.warnings(), &["failed plugin hook".to_string()]);
}
