use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

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
use codex_plugin::ExecutorPluginHookSource;
use codex_plugin::PluginHookSource;
use codex_plugin::PluginId;
use codex_protocol::ThreadId;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookExecutionMode;
use codex_protocol::protocol::HookHandlerType;
use codex_protocol::protocol::HookOutputEntry;
use codex_protocol::protocol::HookOutputEntryKind;
use codex_protocol::protocol::HookRunStatus;
use codex_protocol::protocol::HookSource;
use codex_protocol::protocol::HookTrustStatus;
use futures::FutureExt;
use futures::future::BoxFuture;
use pretty_assertions::assert_eq;
use tempfile::tempdir;
use tokio::sync::Notify;

use super::ClaudeHooksEngine;
use super::CommandHookRuntime;
use super::CommandShell;
use super::ConfiguredHandler;
use super::ConfiguredHandlerKind;
use super::HandlerSourcePath;
use super::HookListEntryHandler;
use crate::events::interrupt::InterruptRequest;
use crate::events::pre_tool_use::PreToolUseRequest;
use crate::events::stop::StopHookTarget;
use crate::events::stop::StopRequest;
use crate::mcp::HookMcpCall;
use crate::mcp::HookMcpExecutor;

fn cwd() -> AbsolutePathBuf {
    AbsolutePathBuf::current_dir().expect("current dir")
}

fn command_runtime(shell: CommandShell) -> CommandHookRuntime {
    let (result_sender, _result_receiver) = async_channel::unbounded();
    CommandHookRuntime::new(
        shell,
        Arc::new(std::env::vars_os().collect()),
        ThreadId::new(),
        result_sender,
    )
}

pub(crate) fn mcp_executor() -> Arc<dyn HookMcpExecutor> {
    Arc::new(StaticMcpExecutor {
        calls: Arc::new(Mutex::new(Vec::new())),
        output: String::new(),
        outputs_by_tool: HashMap::new(),
    })
}

#[test]
fn permission_request_timeout_only_counts_synchronous_handlers() {
    let mut engine = ClaudeHooksEngine::new(
        /*enabled*/ true,
        /*bypass_hook_trust*/ false,
        /*config_layer_stack*/ None,
        Vec::new(),
        Vec::new(),
        command_runtime(CommandShell {
            program: String::new(),
            args: Vec::new(),
        }),
        mcp_executor(),
    );
    let command = "echo synchronous permission hook";
    let synchronous_handler = ConfiguredHandler {
        event_name: HookEventName::PermissionRequest,
        matcher: None,
        timeout_sec: 5,
        status_message: None,
        additional_context_limit: Default::default(),
        source_path: cwd().join("hooks.json").into(),
        source: HookSource::User,
        display_order: 0,
        kind: ConfiguredHandlerKind::Command {
            command: command.to_string(),
            r#async: false,
            env: HashMap::new(),
        },
    };
    let asynchronous_handler = ConfiguredHandler {
        timeout_sec: 600,
        kind: ConfiguredHandlerKind::Command {
            command: command.to_string(),
            r#async: true,
            env: HashMap::new(),
        },
        ..synchronous_handler.clone()
    };

    engine.handlers = vec![synchronous_handler, asynchronous_handler.clone()];
    assert_eq!(
        engine.max_permission_request_timeout(),
        Duration::from_secs(5)
    );

    engine.handlers = vec![asynchronous_handler];
    assert_eq!(engine.max_permission_request_timeout(), Duration::ZERO);
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
                additional_context_limit: None,
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
                RequirementSource::LegacyManagedConfigTomlFromMdm,
            )),
            managed_hooks: managed_hooks.clone().map(|hooks| {
                ConstrainedWithSource::new(
                    Constrained::allow_any(hooks),
                    Some(RequirementSource::LegacyManagedConfigTomlFromMdm),
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

fn required_hooks_stack(
    managed_hooks: ManagedHooksRequirementsToml,
    source: RequirementSource,
) -> ConfigLayerStack {
    ConfigLayerStack::new(
        Vec::new(),
        ConfigRequirements {
            managed_hooks: Some(ConstrainedWithSource::new(
                Constrained::allow_any(managed_hooks.clone()),
                Some(source),
            )),
            ..ConfigRequirements::default()
        },
        ConfigRequirementsToml {
            hooks: Some(managed_hooks),
            ..ConfigRequirementsToml::default()
        },
    )
    .expect("config layer stack")
}

#[test]
fn required_managed_hooks_allow_disabled_hooks_feature() {
    let temp = tempdir().expect("create temp dir");
    let managed_hooks =
        managed_hooks_for_current_platform(temp.path(), pre_tool_use_hook_events("echo managed"));
    let config_layer_stack = required_hooks_stack(
        managed_hooks,
        RequirementSource::LegacyManagedConfigTomlFromMdm,
    );

    let (hooks, _result_receiver) = crate::Hooks::new(
        crate::HooksConfig {
            feature_enabled: false,
            config_layer_stack: Some(config_layer_stack),
            ..Default::default()
        },
        ThreadId::new(),
        mcp_executor(),
    )
    .expect("disabled hooks feature should not enforce managed requirements hooks");

    assert!(hooks.startup_warnings().is_empty());
}

#[test]
fn required_managed_hooks_reject_invalid_matchers() {
    let temp = tempdir().expect("create temp dir");
    let mut events = pre_tool_use_hook_events("echo managed");
    events.pre_tool_use[0].matcher = Some("[".to_string());
    let config_layer_stack = required_hooks_stack(
        managed_hooks_for_current_platform(temp.path(), events),
        RequirementSource::LegacyManagedConfigTomlFromMdm,
    );

    let error = crate::Hooks::new(
        crate::HooksConfig {
            feature_enabled: true,
            config_layer_stack: Some(config_layer_stack),
            ..Default::default()
        },
        ThreadId::new(),
        mcp_executor(),
    )
    .err()
    .expect("invalid required managed matcher should reject startup");

    assert!(error.to_string().contains("invalid matcher"));
}

#[test]
fn required_managed_hooks_allow_invalid_matchers_without_handlers() {
    let temp = tempdir().expect("create temp dir");
    let mut events = pre_tool_use_hook_events("echo managed");
    events.pre_tool_use.push(MatcherGroup {
        matcher: Some("[".to_string()),
        hooks: Vec::new(),
    });
    let config_layer_stack = required_hooks_stack(
        managed_hooks_for_current_platform(temp.path(), events),
        RequirementSource::LegacyManagedConfigTomlFromMdm,
    );

    let (hooks, _result_receiver) = crate::Hooks::new(
        crate::HooksConfig {
            feature_enabled: true,
            config_layer_stack: Some(config_layer_stack),
            ..Default::default()
        },
        ThreadId::new(),
        mcp_executor(),
    )
    .expect("an empty matcher group should not prevent required managed hooks from loading");

    assert_eq!(hooks.startup_warnings().len(), 1);
    assert!(hooks.startup_warnings()[0].contains("invalid matcher"));
}

#[test]
fn required_managed_hooks_reject_empty_commands() {
    let temp = tempdir().expect("create temp dir");
    let config_layer_stack = required_hooks_stack(
        managed_hooks_for_current_platform(temp.path(), pre_tool_use_hook_events("  ")),
        RequirementSource::LegacyManagedConfigTomlFromMdm,
    );

    let error = crate::Hooks::new(
        crate::HooksConfig {
            feature_enabled: true,
            config_layer_stack: Some(config_layer_stack),
            ..Default::default()
        },
        ThreadId::new(),
        mcp_executor(),
    )
    .err()
    .expect("empty required managed command should reject startup");

    assert!(error.to_string().contains("skipping empty hook command"));
}

#[test]
fn required_managed_hooks_reject_unsupported_handler_types() {
    let temp = tempdir().expect("create temp dir");
    let events = HookEventsToml {
        pre_tool_use: vec![MatcherGroup {
            matcher: Some("^Bash$".to_string()),
            hooks: vec![HookHandlerConfig::Prompt {}],
        }],
        ..Default::default()
    };
    let config_layer_stack = required_hooks_stack(
        managed_hooks_for_current_platform(temp.path(), events),
        RequirementSource::LegacyManagedConfigTomlFromMdm,
    );

    let error = crate::Hooks::new(
        crate::HooksConfig {
            feature_enabled: true,
            config_layer_stack: Some(config_layer_stack),
            ..Default::default()
        },
        ThreadId::new(),
        mcp_executor(),
    )
    .err()
    .expect("unsupported required managed handler should reject startup");

    assert!(
        error
            .to_string()
            .contains("prompt hooks are not supported yet")
    );
}

#[test]
fn required_managed_mcp_hooks_reject_empty_targets() {
    let temp = tempdir().expect("create temp dir");
    let events = HookEventsToml {
        pre_tool_use: vec![MatcherGroup {
            matcher: Some("^Bash$".to_string()),
            hooks: vec![HookHandlerConfig::McpTool {
                server: "policy".to_string(),
                tool: " ".to_string(),
                input: Default::default(),
                timeout_sec: None,
                status_message: None,
            }],
        }],
        ..Default::default()
    };
    let config_layer_stack = required_hooks_stack(
        managed_hooks_for_current_platform(temp.path(), events),
        RequirementSource::LegacyManagedConfigTomlFromMdm,
    );

    let error = crate::Hooks::new(
        crate::HooksConfig {
            feature_enabled: true,
            config_layer_stack: Some(config_layer_stack),
            ..Default::default()
        },
        ThreadId::new(),
        mcp_executor(),
    )
    .err()
    .expect("invalid required managed MCP hook should reject startup");

    assert!(
        error
            .to_string()
            .contains("server and tool must not be empty")
    );
}

#[test]
fn required_managed_session_end_mcp_hooks_reject_startup() {
    let temp = tempdir().expect("create temp dir");
    let events = HookEventsToml {
        session_end: vec![MatcherGroup {
            matcher: None,
            hooks: vec![HookHandlerConfig::McpTool {
                server: "policy".to_string(),
                tool: "check".to_string(),
                input: Default::default(),
                timeout_sec: None,
                status_message: None,
            }],
        }],
        ..Default::default()
    };
    let config_layer_stack = required_hooks_stack(
        managed_hooks_for_current_platform(temp.path(), events),
        RequirementSource::LegacyManagedConfigTomlFromMdm,
    );

    let error = crate::Hooks::new(
        crate::HooksConfig {
            feature_enabled: true,
            config_layer_stack: Some(config_layer_stack),
            ..Default::default()
        },
        ThreadId::new(),
        mcp_executor(),
    )
    .err()
    .expect("required managed SessionEnd MCP hook should reject startup");

    assert!(
        error
            .to_string()
            .contains("SessionEnd MCP hooks are not supported")
    );
}

#[test]
fn required_managed_hooks_with_unknown_source_still_reject_discovery_failures() {
    let temp = tempdir().expect("create temp dir");
    let config_layer_stack = required_hooks_stack(
        managed_hooks_for_current_platform(temp.path(), pre_tool_use_hook_events("")),
        RequirementSource::Unknown,
    );

    let error = crate::Hooks::new(
        crate::HooksConfig {
            feature_enabled: true,
            config_layer_stack: Some(config_layer_stack),
            ..Default::default()
        },
        ThreadId::new(),
        mcp_executor(),
    )
    .err()
    .expect("unknown-source managed requirements hook should still reject startup");

    assert!(error.to_string().contains("skipping empty hook command"));
}

#[test]
fn valid_required_managed_hooks_allow_startup() {
    let temp = tempdir().expect("create temp dir");
    let config_layer_stack = required_hooks_stack(
        managed_hooks_for_current_platform(temp.path(), pre_tool_use_hook_events("echo managed")),
        RequirementSource::LegacyManagedConfigTomlFromMdm,
    );

    let (hooks, _result_receiver) = crate::Hooks::new(
        crate::HooksConfig {
            feature_enabled: true,
            config_layer_stack: Some(config_layer_stack),
            ..Default::default()
        },
        ThreadId::new(),
        mcp_executor(),
    )
    .expect("valid managed requirements hook should allow startup");

    assert!(hooks.startup_warnings().is_empty());
}

#[test]
fn managed_config_layer_hook_failures_remain_startup_warnings() {
    let temp = tempdir().expect("create temp dir");
    let config_path =
        AbsolutePathBuf::try_from(temp.path().join("config.toml")).expect("absolute config path");
    let config_layer_stack = ConfigLayerStack::new(
        vec![ConfigLayerEntry::new(
            ConfigLayerSource::System { file: config_path },
            config_toml_with_pre_tool_use("  "),
        )],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("config layer stack");

    let (hooks, _result_receiver) = crate::Hooks::new(
        crate::HooksConfig {
            feature_enabled: true,
            config_layer_stack: Some(config_layer_stack),
            ..Default::default()
        },
        ThreadId::new(),
        mcp_executor(),
    )
    .expect("managed config layer hooks should remain optional");

    assert_eq!(hooks.startup_warnings().len(), 1);
    assert!(hooks.startup_warnings()[0].contains("skipping empty hook command"));
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
                    additional_context_limit: None,
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
                Some(RequirementSource::LegacyManagedConfigTomlFromMdm),
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
        /*bypass_hook_trust*/ false,
        Some(&config_layer_stack),
        Vec::new(),
        Vec::new(),
        command_runtime(CommandShell {
            program: String::new(),
            args: Vec::new(),
        }),
        mcp_executor(),
    );

    assert!(engine.warnings().is_empty());
    assert_eq!(engine.handlers.len(), 1);
    assert_eq!(
        engine.handlers[0].source,
        HookSource::LegacyManagedConfigMdm
    );
    let listed = crate::list_hooks(crate::HooksConfig {
        legacy_notify_argv: None,
        feature_enabled: true,
        bypass_hook_trust: false,
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
        subagent: None,
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
            subagent: None,
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
                    additional_context_limit: None,
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
                Some(RequirementSource::LegacyManagedConfigTomlFromMdm),
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
        /*bypass_hook_trust*/ false,
        Some(&config_layer_stack),
        Vec::new(),
        Vec::new(),
        command_runtime(CommandShell {
            program: String::new(),
            args: Vec::new(),
        }),
        mcp_executor(),
    );

    let outcome = engine
        .run_pre_tool_use(PreToolUseRequest {
            session_id: ThreadId::new(),
            turn_id: "turn-1".to_string(),
            subagent: None,
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
                    additional_context_limit: None,
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
        /*bypass_hook_trust*/ false,
        Some(&config_layer_stack),
        Vec::new(),
        Vec::new(),
        command_runtime(CommandShell {
            program: String::new(),
            args: Vec::new(),
        }),
        mcp_executor(),
    );

    assert_eq!(engine.handlers.len(), 1);
    assert_eq!(engine.handlers[0].source, HookSource::Unknown);
    let discovered = super::discovery::discover_handlers(
        Some(&config_layer_stack),
        Vec::new(),
        Vec::new(),
        /*bypass_hook_trust*/ false,
    );
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
                    additional_context_limit: None,
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
            ConfigLayerSource::User {
                file: config_path,
                profile: None,
            },
            user_config,
        )],
        ConfigRequirements {
            managed_hooks: Some(ConstrainedWithSource::new(
                Constrained::allow_any(managed_hooks.clone()),
                Some(RequirementSource::LegacyManagedConfigTomlFromMdm),
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
        /*bypass_hook_trust*/ false,
        Some(&config_layer_stack),
        Vec::new(),
        Vec::new(),
        command_runtime(CommandShell {
            program: String::new(),
            args: Vec::new(),
        }),
        mcp_executor(),
    );

    assert_eq!(engine.handlers.len(), 1);
    assert_eq!(
        engine.handlers[0].source,
        HookSource::LegacyManagedConfigMdm
    );
    let discovered = super::discovery::discover_handlers(
        Some(&config_layer_stack),
        Vec::new(),
        Vec::new(),
        /*bypass_hook_trust*/ false,
    );
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
                    profile: None,
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
        /*bypass_hook_trust*/ false,
        Some(&config_layer_stack),
        Vec::new(),
        Vec::new(),
        command_runtime(CommandShell {
            program: String::new(),
            args: Vec::new(),
        }),
        mcp_executor(),
    );

    assert_eq!(engine.handlers.len(), 1);
    assert_eq!(
        engine.handlers[0].source,
        HookSource::LegacyManagedConfigFile
    );
    let discovered = super::discovery::discover_handlers(
        Some(&config_layer_stack),
        Vec::new(),
        Vec::new(),
        /*bypass_hook_trust*/ false,
    );
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
        /*bypass_hook_trust*/ false,
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
            ConfigLayerSource::User {
                file: config_path,
                profile: None,
            },
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
                    additional_context_limit: None,
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
                Some(RequirementSource::LegacyManagedConfigTomlFromMdm),
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
        /*bypass_hook_trust*/ false,
        Some(&config_layer_stack),
        Vec::new(),
        Vec::new(),
        command_runtime(CommandShell {
            program: String::new(),
            args: Vec::new(),
        }),
        mcp_executor(),
    );

    assert!(engine.warnings().is_empty());
    let cwd = cwd();
    let preview = engine.preview_pre_tool_use(&PreToolUseRequest {
        session_id: ThreadId::new(),
        turn_id: "turn-1".to_string(),
        subagent: None,
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
    assert_eq!(
        engine.handlers[0].kind,
        ConfiguredHandlerKind::Command {
            command: "echo hi".to_string(),
            r#async: false,
            env: HashMap::new(),
        }
    );
    assert_eq!(
        engine.handlers[0].source_path,
        AbsolutePathBuf::try_from(missing_dir)
            .expect("absolute missing dir")
            .into()
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
            ConfigLayerSource::User {
                file: config_path,
                profile: None,
            },
            config_toml_with_pre_tool_use("python3 /tmp/user-hook.py"),
        )],
        requirements,
        requirements_toml,
    )
    .expect("config layer stack");

    let engine = ClaudeHooksEngine::new(
        /*enabled*/ true,
        /*bypass_hook_trust*/ false,
        Some(&config_layer_stack),
        Vec::new(),
        Vec::new(),
        command_runtime(CommandShell {
            program: String::new(),
            args: Vec::new(),
        }),
        mcp_executor(),
    );

    assert!(engine.warnings().is_empty());
    assert!(engine.handlers.is_empty());
    let discovered = super::discovery::discover_handlers(
        Some(&config_layer_stack),
        Vec::new(),
        Vec::new(),
        /*bypass_hook_trust*/ false,
    );
    assert_eq!(discovered.hook_entries.len(), 1);
    assert!(!discovered.hook_entries[0].is_managed);
    assert_eq!(
        discovered.hook_entries[0].handler,
        HookListEntryHandler::Command {
            command: "python3 /tmp/user-hook.py".to_string(),
            r#async: false,
        }
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
            ConfigLayerSource::User {
                file: config_path,
                profile: None,
            },
            config_toml,
        )],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("config layer stack");

    let engine = ClaudeHooksEngine::new(
        /*enabled*/ true,
        /*bypass_hook_trust*/ false,
        Some(&config_layer_stack),
        Vec::new(),
        Vec::new(),
        command_runtime(CommandShell {
            program: String::new(),
            args: Vec::new(),
        }),
        mcp_executor(),
    );

    assert!(engine.warnings().is_empty());
    assert!(engine.handlers.is_empty());
    let discovered = super::discovery::discover_handlers(
        Some(&config_layer_stack),
        Vec::new(),
        Vec::new(),
        /*bypass_hook_trust*/ false,
    );
    assert_eq!(discovered.hook_entries.len(), 1);
    assert!(!discovered.hook_entries[0].is_managed);
    assert_eq!(
        discovered.hook_entries[0].handler,
        HookListEntryHandler::Command {
            command: "python3 /tmp/user-hook.py".to_string(),
            r#async: false,
        }
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
            ConfigLayerSource::User {
                file: config_path,
                profile: None,
            },
            config_toml_with_pre_tool_use("python3 /tmp/toml-hook.py"),
        )],
        requirements,
        requirements_toml,
    )
    .expect("config layer stack");

    let engine = ClaudeHooksEngine::new(
        /*enabled*/ true,
        /*bypass_hook_trust*/ false,
        Some(&config_layer_stack),
        Vec::new(),
        Vec::new(),
        command_runtime(CommandShell {
            program: String::new(),
            args: Vec::new(),
        }),
        mcp_executor(),
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
        /*bypass_hook_trust*/ false,
        Some(&config_layer_stack),
        plugin_hook_sources,
        Vec::new(),
        command_runtime(CommandShell {
            program: String::new(),
            args: Vec::new(),
        }),
        mcp_executor(),
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
        /*bypass_hook_trust*/ false,
        Some(&config_layer_stack),
        Vec::new(),
        Vec::new(),
        command_runtime(CommandShell {
            program: String::new(),
            args: Vec::new(),
        }),
        mcp_executor(),
    );

    assert!(engine.warnings().is_empty());
    assert_eq!(
        engine
            .handlers
            .iter()
            .map(|handler| match &handler.kind {
                ConfiguredHandlerKind::Command { command, .. } => Some(command.as_str()),
                ConfiguredHandlerKind::McpTool { .. } => None,
            })
            .collect::<Vec<_>>(),
        vec![
            Some("python3 /tmp/requirements-hook.py"),
            Some("python3 /tmp/mdm-hook.py"),
            Some("python3 /tmp/system-hook.py"),
            Some("python3 /tmp/legacy-file-hook.py"),
            Some("python3 /tmp/legacy-mdm-hook.py"),
        ]
    );
    let discovered = super::discovery::discover_handlers(
        Some(&config_layer_stack),
        Vec::new(),
        Vec::new(),
        /*bypass_hook_trust*/ false,
    );
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
        /*bypass_hook_trust*/ false,
        Some(&config_layer_stack),
        Vec::new(),
        Vec::new(),
        command_runtime(CommandShell {
            program: String::new(),
            args: Vec::new(),
        }),
        mcp_executor(),
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
        subagent: None,
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

#[test]
fn profile_user_layers_load_shared_hooks_json_once() {
    let temp = tempdir().expect("create temp dir");
    let config_path =
        AbsolutePathBuf::try_from(temp.path().join("config.toml")).expect("absolute config path");
    let profile_path = AbsolutePathBuf::try_from(temp.path().join("work.config.toml"))
        .expect("absolute profile path");
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
    let config_layer_stack = ConfigLayerStack::new(
        vec![
            ConfigLayerEntry::new(
                ConfigLayerSource::User {
                    file: config_path,
                    profile: None,
                },
                TomlValue::Table(Default::default()),
            ),
            ConfigLayerEntry::new(
                ConfigLayerSource::User {
                    file: profile_path,
                    profile: Some("work".to_string()),
                },
                TomlValue::Table(Default::default()),
            ),
        ],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("config layer stack");

    let engine = ClaudeHooksEngine::new(
        /*enabled*/ true,
        /*bypass_hook_trust*/ true,
        Some(&config_layer_stack),
        Vec::new(),
        Vec::new(),
        command_runtime(CommandShell {
            program: String::new(),
            args: Vec::new(),
        }),
        mcp_executor(),
    );

    assert!(engine.warnings().is_empty());
    assert_eq!(engine.handlers.len(), 1);
    let preview = engine.preview_pre_tool_use(&PreToolUseRequest {
        session_id: ThreadId::new(),
        turn_id: "turn-1".to_string(),
        subagent: None,
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
    assert_eq!(preview[0].source_path, hooks_json_path);

    let listed = crate::list_hooks(crate::HooksConfig {
        feature_enabled: true,
        bypass_hook_trust: true,
        config_layer_stack: Some(config_layer_stack),
        ..Default::default()
    });
    assert!(listed.warnings.is_empty());
    assert_eq!(listed.hooks.len(), 1);
    assert_eq!(listed.hooks[0].source_path, hooks_json_path);
}

#[test]
fn malformed_hooks_json_is_reported_as_startup_warning() {
    let temp = tempdir().expect("create temp dir");
    let config_path =
        AbsolutePathBuf::try_from(temp.path().join("config.toml")).expect("absolute config path");
    let hooks_json_path =
        AbsolutePathBuf::try_from(temp.path().join("hooks.json")).expect("absolute hooks path");
    fs::write(
        hooks_json_path.as_path(),
        r#"{
          "SessionStart": [
            {
              "hooks": [
                {
                  "type": "command",
                  "command": "python3 /tmp/session-start.py"
                }
              ]
            }
          ]
        }"#,
    )
    .expect("write hooks.json");
    let config_layer_stack = ConfigLayerStack::new(
        vec![ConfigLayerEntry::new(
            ConfigLayerSource::System { file: config_path },
            TomlValue::Table(Default::default()),
        )],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("config layer stack");

    let engine = ClaudeHooksEngine::new(
        /*enabled*/ true,
        /*bypass_hook_trust*/ false,
        Some(&config_layer_stack),
        Vec::new(),
        Vec::new(),
        command_runtime(CommandShell {
            program: String::new(),
            args: Vec::new(),
        }),
        mcp_executor(),
    );

    assert!(engine.handlers.is_empty());
    assert_eq!(engine.warnings().len(), 1);
    assert!(engine.warnings()[0].contains("failed to parse hooks config"));
    assert!(
        engine.warnings()[0].contains(&hooks_json_path.display().to_string()),
        "warning should identify the malformed file: {}",
        engine.warnings()[0]
    );
    assert!(engine.warnings()[0].contains("unknown field `SessionStart`"));
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
                    additional_context_limit: None,
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
        /*bypass_hook_trust*/ false,
        Some(&config_layer_stack),
        plugin_hook_sources.clone(),
        Vec::new(),
        command_runtime(CommandShell {
            program: String::new(),
            args: Vec::new(),
        }),
        mcp_executor(),
    );

    let preview = engine.preview_pre_tool_use(&PreToolUseRequest {
        session_id: ThreadId::new(),
        turn_id: "turn-1".to_string(),
        subagent: None,
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
        bypass_hook_trust: false,
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
            subagent: None,
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
    assert_eq!(
        outcome.hook_events[0].run.status,
        HookRunStatus::Completed,
        "hook entries: {:#?}",
        outcome.hook_events[0].run.entries
    );
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
                    additional_context_limit: None,
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
        /*bypass_hook_trust*/ false,
        Some(&config_layer_stack),
        plugin_hook_sources,
        Vec::new(),
        command_runtime(CommandShell {
            program: String::new(),
            args: Vec::new(),
        }),
        mcp_executor(),
    );

    assert_eq!(
        engine.handlers[0].kind,
        ConfiguredHandlerKind::Command {
            command: format!(
                "run {} {} {} {}",
                plugin_root.display(),
                plugin_root.display(),
                plugin_data_root.display(),
                plugin_data_root.display()
            ),
            r#async: false,
            env: HashMap::from([
                ("PLUGIN_ROOT".to_string(), plugin_root.display().to_string()),
                (
                    "CLAUDE_PLUGIN_ROOT".to_string(),
                    plugin_root.display().to_string(),
                ),
                (
                    "PLUGIN_DATA".to_string(),
                    plugin_data_root.display().to_string(),
                ),
                (
                    "CLAUDE_PLUGIN_DATA".to_string(),
                    plugin_data_root.display().to_string(),
                ),
            ]),
        }
    );
}

#[test]
fn plugin_hook_load_warnings_are_startup_warnings() {
    let engine = ClaudeHooksEngine::new(
        /*enabled*/ true,
        /*bypass_hook_trust*/ false,
        /*config_layer_stack*/ None,
        Vec::new(),
        vec!["failed plugin hook".to_string()],
        command_runtime(CommandShell {
            program: String::new(),
            args: Vec::new(),
        }),
        mcp_executor(),
    );

    assert_eq!(engine.warnings(), &["failed plugin hook".to_string()]);
}

struct StaticMcpExecutor {
    calls: Arc<Mutex<Vec<HookMcpCall>>>,
    output: String,
    outputs_by_tool: HashMap<String, String>,
}

impl HookMcpExecutor for StaticMcpExecutor {
    fn execute(&self, call: HookMcpCall) -> BoxFuture<'_, anyhow::Result<String>> {
        async move {
            let output = self
                .outputs_by_tool
                .get(&call.tool)
                .unwrap_or(&self.output)
                .clone();
            self.calls.lock().expect("lock MCP calls").push(call);
            Ok(output)
        }
        .boxed()
    }
}

fn executor_stop_hook_fixture() -> (
    ClaudeHooksEngine,
    Arc<Mutex<Vec<HookMcpCall>>>,
    StopRequest,
    HookMcpCall,
    ExecutorPluginHookSource,
) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let executor = Arc::new(StaticMcpExecutor {
        calls: Arc::clone(&calls),
        output: r#"{"decision":"block","reason":"keep going"}"#.to_string(),
        outputs_by_tool: HashMap::from([(
            "terminate".to_string(),
            r#"{"continue":false,"stopReason":"done"}"#.to_string(),
        )]),
    });
    let mut engine = ClaudeHooksEngine::new(
        /*enabled*/ true,
        /*bypass_hook_trust*/ false,
        /*config_layer_stack*/ None,
        Vec::new(),
        Vec::new(),
        command_runtime(CommandShell {
            program: String::new(),
            args: Vec::new(),
        }),
        executor,
    );
    let source = ExecutorPluginHookSource {
        plugin_id: PluginId::parse("computer-use@openai-bundled").expect("valid plugin ID"),
        environment_id: "executor-a".to_string(),
        plugin_root: "file:///plugins/computer-use"
            .parse()
            .expect("valid plugin root URI"),
        manifest_path: "file:///plugins/computer-use/.codex-plugin/plugin.json"
            .parse()
            .expect("valid plugin manifest URI"),
        source_relative_path: ".codex-plugin/plugin.json#hooks[0]".to_string(),
        hooks: HookEventsToml {
            stop: vec![MatcherGroup {
                matcher: None,
                hooks: vec![HookHandlerConfig::McpTool {
                    server: "node_repl".to_string(),
                    tool: "turn_ended".to_string(),
                    input: serde_json::from_value(serde_json::json!({
                        "turn_id": "${turn_id}",
                    }))
                    .expect("executor hook input"),
                    timeout_sec: None,
                    status_message: None,
                }],
            }],
            ..Default::default()
        },
    };
    engine.set_executor_hooks(vec![source.clone()]);
    assert_eq!(
        engine.handlers,
        vec![ConfiguredHandler {
            event_name: HookEventName::Stop,
            matcher: None,
            timeout_sec: 5,
            status_message: None,
            additional_context_limit: Default::default(),
            source_path: HandlerSourcePath::ExecutorScoped {
                plugin_id: PluginId::parse("computer-use@openai-bundled").expect("valid plugin ID"),
                environment_id: "executor-a".to_string(),
                manifest_path: "file:///plugins/computer-use/.codex-plugin/plugin.json"
                    .parse()
                    .expect("valid plugin manifest URI"),
                source_relative_path: ".codex-plugin/plugin.json#hooks[0]".to_string(),
            },
            source: HookSource::Plugin,
            display_order: 0,
            kind: ConfiguredHandlerKind::McpTool {
                server: "node_repl".to_string(),
                tool: "turn_ended".to_string(),
                input: serde_json::from_value(serde_json::json!({
                    "turn_id": "${turn_id}",
                }))
                .expect("executor hook input"),
            },
        }]
    );
    assert_eq!(
        engine.handlers[0].execution_mode(),
        HookExecutionMode::Async
    );
    assert!(!engine.handlers[0].can_apply_control_effects());
    let request_metadata = Some(serde_json::Map::from_iter([(
        "x-codex-turn-metadata".to_string(),
        serde_json::json!({ "turn_id": "turn-1" }),
    )]));
    let request = StopRequest {
        session_id: ThreadId::new(),
        turn_id: "turn-1".to_string(),
        cwd: cwd(),
        transcript_path: None,
        model: "test-model".to_string(),
        permission_mode: "default".to_string(),
        request_metadata: request_metadata.clone(),
        stop_hook_active: false,
        last_assistant_message: None,
        target: StopHookTarget::Stop,
    };

    let expected_executor_call = HookMcpCall {
        server: "node_repl".to_string(),
        tool: "turn_ended".to_string(),
        environment_id: Some("executor-a".to_string()),
        metadata: request_metadata,
        input: serde_json::from_value(serde_json::json!({ "turn_id": "turn-1" }))
            .expect("expanded executor hook input"),
        timeout: Duration::from_secs(5),
    };

    (engine, calls, request, expected_executor_call, source)
}

async fn wait_for_mcp_calls(calls: &Arc<Mutex<Vec<HookMcpCall>>>, count: usize) {
    tokio::time::timeout(Duration::from_secs(1), async {
        while calls.lock().expect("lock MCP calls").len() < count {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("executor hook should complete in the background");
}

#[tokio::test]
async fn executor_stop_hooks_run_unless_regular_hooks_block_without_stopping() {
    let (mut engine, calls, request, expected_executor_call, _source) =
        executor_stop_hook_fixture();

    assert_eq!(engine.preview_stop(&request), Vec::new());
    let outcome = engine.run_stop(request.clone()).await;
    wait_for_mcp_calls(&calls, /*count*/ 1).await;
    assert!(!outcome.should_block);
    assert!(outcome.hook_events.is_empty());
    assert_eq!(
        *calls.lock().expect("lock MCP calls"),
        vec![expected_executor_call.clone()]
    );

    engine.handlers.push(ConfiguredHandler {
        event_name: HookEventName::Stop,
        matcher: None,
        timeout_sec: 30,
        status_message: None,
        additional_context_limit: Default::default(),
        source_path: cwd().join("hooks.json").into(),
        source: HookSource::User,
        display_order: 0,
        kind: ConfiguredHandlerKind::McpTool {
            server: "security".to_string(),
            tool: "check".to_string(),
            input: Default::default(),
        },
    });

    let outcome = engine.run_stop(request.clone()).await;
    assert!(outcome.should_block);
    assert_eq!(outcome.hook_events.len(), 1);
    assert_eq!(
        *calls.lock().expect("lock MCP calls"),
        vec![
            expected_executor_call.clone(),
            HookMcpCall {
                server: "security".to_string(),
                tool: "check".to_string(),
                environment_id: None,
                metadata: None,
                input: Default::default(),
                timeout: Duration::from_secs(30),
            },
        ]
    );

    engine.handlers.push(ConfiguredHandler {
        event_name: HookEventName::Stop,
        matcher: None,
        timeout_sec: 30,
        status_message: None,
        additional_context_limit: Default::default(),
        source_path: cwd().join("hooks.json").into(),
        source: HookSource::User,
        display_order: 1,
        kind: ConfiguredHandlerKind::McpTool {
            server: "security".to_string(),
            tool: "terminate".to_string(),
            input: Default::default(),
        },
    });

    let outcome = engine.run_stop(request).await;
    wait_for_mcp_calls(&calls, /*count*/ 5).await;
    assert!(outcome.should_stop);
    assert!(!outcome.should_block);
    assert_eq!(outcome.hook_events.len(), 2);
    assert_eq!(
        *calls.lock().expect("lock MCP calls"),
        vec![
            expected_executor_call.clone(),
            HookMcpCall {
                server: "security".to_string(),
                tool: "check".to_string(),
                environment_id: None,
                metadata: None,
                input: Default::default(),
                timeout: Duration::from_secs(30),
            },
            HookMcpCall {
                server: "security".to_string(),
                tool: "check".to_string(),
                environment_id: None,
                metadata: None,
                input: Default::default(),
                timeout: Duration::from_secs(30),
            },
            HookMcpCall {
                server: "security".to_string(),
                tool: "terminate".to_string(),
                environment_id: None,
                metadata: None,
                input: Default::default(),
                timeout: Duration::from_secs(30),
            },
            expected_executor_call,
        ]
    );
}

#[test]
fn executor_stop_hooks_register_only_the_first_environment_and_handler() {
    let (mut engine, _, _, _, mut first_source) = executor_stop_hook_fixture();
    let expected_handlers = engine.handlers.clone();
    let first_group = &mut first_source.hooks.stop[0];
    let mut second_handler = first_group.hooks[0].clone();
    let HookHandlerConfig::McpTool { tool, .. } = &mut second_handler else {
        panic!("executor Stop handler should be an MCP tool");
    };
    *tool = "second_turn_ended".to_string();
    first_group.hooks.push(second_handler);
    let mut second_source = first_source.clone();
    second_source.environment_id = "executor-b".to_string();
    engine.set_executor_hooks(vec![first_source, second_source]);

    assert_eq!(engine.handlers, expected_handlers);
}

#[tokio::test]
async fn memory_consolidation_stop_preserves_policy_and_executor_cleanup() {
    for (source, runs_policy) in [
        (HookSource::User, false),
        (HookSource::Project, false),
        (HookSource::SessionFlags, false),
        (HookSource::Plugin, false),
        (HookSource::System, true),
        (HookSource::Mdm, true),
        (HookSource::CloudRequirements, true),
        (HookSource::CloudManagedConfig, true),
        (HookSource::LegacyManagedConfigFile, true),
        (HookSource::LegacyManagedConfigMdm, true),
        (HookSource::Unknown, true),
    ] {
        let (mut engine, calls, mut request, expected_executor_call, _source) =
            executor_stop_hook_fixture();
        request.target = StopHookTarget::MemoryConsolidation;
        let policy_call = HookMcpCall {
            server: "security".to_string(),
            tool: "check".to_string(),
            environment_id: None,
            metadata: None,
            input: Default::default(),
            timeout: Duration::from_secs(5),
        };
        let mut handler = engine.handlers[0].clone();
        handler.source_path = cwd().join("hooks.json").into();
        handler.source = source;
        handler.kind = ConfiguredHandlerKind::McpTool {
            server: policy_call.server.clone(),
            tool: policy_call.tool.clone(),
            input: policy_call.input.clone(),
        };
        engine.handlers.push(handler);
        assert_eq!(
            engine.preview_stop(&request).len(),
            usize::from(runs_policy)
        );

        let outcome = engine.run_stop(request).await;
        let expected_calls = if runs_policy {
            vec![policy_call, expected_executor_call]
        } else {
            vec![expected_executor_call]
        };
        wait_for_mcp_calls(&calls, expected_calls.len()).await;
        assert_eq!(*calls.lock().expect("lock MCP calls"), expected_calls);
        assert_eq!(outcome.should_block, runs_policy);
    }
}

#[tokio::test]
async fn executor_stop_hooks_do_not_delay_stop_completion() {
    struct BlockingMcpExecutor {
        started: Arc<Notify>,
        release: Arc<Notify>,
    }

    impl HookMcpExecutor for BlockingMcpExecutor {
        fn execute(&self, _call: HookMcpCall) -> BoxFuture<'_, anyhow::Result<String>> {
            async move {
                self.started.notify_one();
                self.release.notified().await;
                Ok(String::new())
            }
            .boxed()
        }
    }

    let (mut engine, _, request, _, _) = executor_stop_hook_fixture();
    let started = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    engine.mcp_executor = Arc::new(BlockingMcpExecutor {
        started: Arc::clone(&started),
        release: Arc::clone(&release),
    });

    let outcome = tokio::time::timeout(Duration::from_millis(100), engine.run_stop(request))
        .await
        .expect("executor hook must not delay Stop completion");
    assert!(!outcome.should_block);
    tokio::time::timeout(Duration::from_secs(1), started.notified())
        .await
        .expect("executor hook should start in the background");
    release.notify_one();
}

#[tokio::test]
async fn mcp_tool_hooks_expand_event_input_and_apply_pre_tool_decisions() {
    let temp = tempdir().expect("create temp dir");
    let config_path =
        AbsolutePathBuf::try_from(temp.path().join("config.toml")).expect("absolute config path");
    fs::write(
        temp.path().join("hooks.json"),
        serde_json::json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "Bash",
                    "hooks": [{
                        "type": "mcp_tool",
                        "server": "security",
                        "tool": "scan",
                        "input": { "command": "${tool_input.command}" },
                        "timeout": 20,
                    }],
                }],
            },
        })
        .to_string(),
    )
    .expect("write MCP hooks.json");
    let config_layer_stack = ConfigLayerStack::new(
        vec![ConfigLayerEntry::new(
            ConfigLayerSource::User {
                file: config_path,
                profile: None,
            },
            TomlValue::Table(Default::default()),
        )],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("config layer stack");

    let request = PreToolUseRequest {
        session_id: ThreadId::new(),
        turn_id: "turn-1".to_string(),
        subagent: None,
        cwd: cwd(),
        transcript_path: None,
        model: "gpt-test".to_string(),
        permission_mode: "default".to_string(),
        tool_name: "Bash".to_string(),
        matcher_aliases: Vec::new(),
        tool_use_id: "tool-1".to_string(),
        tool_input: serde_json::json!({ "command": "rm important.txt" }),
    };
    let calls = Arc::new(Mutex::new(Vec::new()));
    let executor = StaticMcpExecutor {
        calls: Arc::clone(&calls),
        output: serde_json::json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "deny",
                "permissionDecisionReason": "blocked by MCP scanner",
            },
        })
        .to_string(),
        outputs_by_tool: HashMap::new(),
    };
    let engine = ClaudeHooksEngine::new(
        /*enabled*/ true,
        /*bypass_hook_trust*/ true,
        Some(&config_layer_stack),
        Vec::new(),
        Vec::new(),
        command_runtime(CommandShell {
            program: String::new(),
            args: Vec::new(),
        }),
        Arc::new(executor),
    );
    let outcome = engine.run_pre_tool_use(request).await;

    assert!(outcome.should_block);
    assert_eq!(
        outcome.block_reason.as_deref(),
        Some("blocked by MCP scanner")
    );
    assert_eq!(
        outcome.hook_events[0].run.handler_type,
        HookHandlerType::McpTool
    );
    assert_eq!(
        outcome.hook_events[0].run.execution_mode,
        codex_protocol::protocol::HookExecutionMode::Sync
    );
    assert_eq!(
        *calls.lock().expect("lock MCP calls"),
        vec![HookMcpCall {
            server: "security".to_string(),
            tool: "scan".to_string(),
            environment_id: None,
            metadata: None,
            input: serde_json::from_value(serde_json::json!({
                "command": "rm important.txt",
            }))
            .expect("object input"),
            timeout: Duration::from_secs(20),
        }]
    );
}

#[tokio::test]
async fn mcp_interrupt_hooks_expand_event_input_and_bound_timeout() {
    let temp = tempdir().expect("create temp dir");
    let config_path =
        AbsolutePathBuf::try_from(temp.path().join("config.toml")).expect("absolute config path");
    fs::write(
        temp.path().join("hooks.json"),
        serde_json::json!({
            "hooks": {
                "Interrupt": [{
                    "hooks": [{
                        "type": "mcp_tool",
                        "server": "security",
                        "tool": "notify",
                        "input": {
                            "event": "${hook_event_name}",
                            "turn_id": "${turn_id}",
                            "permission_mode": "${permission_mode}",
                        },
                        "timeout": 20,
                    }],
                }],
            },
        })
        .to_string(),
    )
    .expect("write MCP Interrupt hooks.json");
    let config_layer_stack = ConfigLayerStack::new(
        vec![ConfigLayerEntry::new(
            ConfigLayerSource::User {
                file: config_path,
                profile: None,
            },
            TomlValue::Table(Default::default()),
        )],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("config layer stack");

    let calls = Arc::new(Mutex::new(Vec::new()));
    let executor = StaticMcpExecutor {
        calls: Arc::clone(&calls),
        output: serde_json::json!({
            "systemMessage": "interrupt observed",
        })
        .to_string(),
        outputs_by_tool: HashMap::new(),
    };
    let engine = ClaudeHooksEngine::new(
        /*enabled*/ true,
        /*bypass_hook_trust*/ true,
        Some(&config_layer_stack),
        Vec::new(),
        Vec::new(),
        command_runtime(CommandShell {
            program: String::new(),
            args: Vec::new(),
        }),
        Arc::new(executor),
    );
    let outcome = engine
        .run_interrupt(InterruptRequest {
            session_id: ThreadId::new(),
            turn_id: "turn-1".to_string(),
            cwd: cwd(),
            transcript_path: None,
            model: "gpt-test".to_string(),
            permission_mode: "default".to_string(),
        })
        .await;

    assert_eq!(outcome.hook_events.len(), 1);
    assert_eq!(
        outcome.hook_events[0].run.handler_type,
        HookHandlerType::McpTool
    );
    assert_eq!(outcome.hook_events[0].run.status, HookRunStatus::Completed);
    assert_eq!(
        outcome.hook_events[0].run.entries,
        vec![HookOutputEntry {
            kind: HookOutputEntryKind::Warning,
            text: "interrupt observed".to_string(),
        }]
    );
    assert_eq!(
        *calls.lock().expect("lock MCP calls"),
        vec![HookMcpCall {
            server: "security".to_string(),
            tool: "notify".to_string(),
            environment_id: None,
            metadata: None,
            input: serde_json::from_value(serde_json::json!({
                "event": "Interrupt",
                "turn_id": "turn-1",
                "permission_mode": "default",
            }))
            .expect("object input"),
            timeout: Duration::from_secs(3),
        }]
    );
}
