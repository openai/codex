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
use codex_config::TomlValue;
use codex_plugin::PluginHookSource;
use codex_plugin::PluginId;
use codex_protocol::ThreadId;
use codex_protocol::protocol::HookOutputEntryKind;
use codex_protocol::protocol::HookRunStatus;
use codex_protocol::protocol::HookSource;
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
    assert!(engine.handlers[0].source.is_managed());
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
    assert!(engine.handlers[0].source.is_managed());
    let discovered =
        super::discovery::discover_handlers(Some(&config_layer_stack), Vec::new(), Vec::new());
    assert_eq!(discovered.hook_entries.len(), 2);
    assert_eq!(discovered.hook_entries[0].key, managed_disabled_key);
    assert_eq!(discovered.hook_entries[0].enabled, true);
    assert!(discovered.hook_entries[0].is_managed);
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
    assert!(engine.handlers[0].source.is_managed());
    let discovered =
        super::discovery::discover_handlers(Some(&config_layer_stack), Vec::new(), Vec::new());
    assert_eq!(discovered.hook_entries.len(), 1);
    assert_eq!(discovered.hook_entries[0].key, managed_key);
    assert_eq!(discovered.hook_entries[0].enabled, true);
    assert!(discovered.hook_entries[0].is_managed);
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

#[test]
fn requirements_managed_hooks_warn_when_managed_dir_is_missing() {
    let temp = tempdir().expect("create temp dir");
    let missing_dir = temp.path().join("missing-managed-hooks");
    let managed_hooks = managed_hooks_for_current_platform(
        missing_dir.clone(),
        HookEventsToml {
            pre_tool_use: vec![MatcherGroup {
                matcher: Some("^Bash$".to_string()),
                hooks: vec![HookHandlerConfig::Command {
                    command: format!("python3 {}", missing_dir.join("pre.py").display()),
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

    assert!(engine.warnings().iter().any(|warning| {
        warning.contains("managed hook directory")
            && warning.contains("does not exist")
            && warning.contains(&missing_dir.display().to_string())
    }));
    let cwd = cwd();
    assert!(
        engine
            .preview_pre_tool_use(&PreToolUseRequest {
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
            .is_empty()
    );
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
            ConfigLayerSource::User {
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
    assert!(
        engine
            .handlers
            .iter()
            .all(|handler| !handler.source.is_managed())
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
                    timeout_sec: Some(10),
                    r#async: false,
                    status_message: None,
                }],
            }],
            ..Default::default()
        },
    }];
    let engine = ClaudeHooksEngine::new(
        /*enabled*/ true,
        /*config_layer_stack*/ None,
        plugin_hook_sources,
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
                    command: "run ${PLUGIN_ROOT} ${CLAUDE_PLUGIN_ROOT} ${PLUGIN_DATA} ${CLAUDE_PLUGIN_DATA}"
                        .to_string(),
                    timeout_sec: Some(5),
                    r#async: false,
                    status_message: None,
                }],
            }],
            ..Default::default()
        },
    }];
    let engine = ClaudeHooksEngine::new(
        /*enabled*/ true,
        /*config_layer_stack*/ None,
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
