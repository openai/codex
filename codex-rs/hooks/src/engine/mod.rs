pub(crate) mod command_runner;
pub(crate) mod discovery;
pub(crate) mod dispatcher;
pub(crate) mod output_parser;
pub(crate) mod schema_loader;

use codex_config::ConfigLayerStack;
use codex_protocol::protocol::HookRunSummary;
use codex_protocol::protocol::HookSource;
use codex_utils_absolute_path::AbsolutePathBuf;

use crate::events::permission_request::PermissionRequestOutcome;
use crate::events::permission_request::PermissionRequestRequest;
use crate::events::post_tool_use::PostToolUseOutcome;
use crate::events::post_tool_use::PostToolUseRequest;
use crate::events::pre_tool_use::PreToolUseOutcome;
use crate::events::pre_tool_use::PreToolUseRequest;
use crate::events::session_start::SessionStartOutcome;
use crate::events::session_start::SessionStartRequest;
use crate::events::stop::StopOutcome;
use crate::events::stop::StopRequest;
use crate::events::user_prompt_submit::UserPromptSubmitOutcome;
use crate::events::user_prompt_submit::UserPromptSubmitRequest;

#[derive(Debug, Clone)]
pub(crate) struct CommandShell {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConfiguredHandler {
    pub event_name: codex_protocol::protocol::HookEventName,
    pub is_managed: bool,
    pub matcher: Option<String>,
    pub command: String,
    pub timeout_sec: u64,
    pub status_message: Option<String>,
    pub source_path: AbsolutePathBuf,
    pub source: HookSource,
    pub display_order: i64,
}

impl ConfiguredHandler {
    pub fn run_id(&self) -> String {
        format!(
            "{}:{}:{}",
            self.event_name_label(),
            self.display_order,
            self.source_path.display()
        )
    }

    fn event_name_label(&self) -> &'static str {
        match self.event_name {
            codex_protocol::protocol::HookEventName::PreToolUse => "pre-tool-use",
            codex_protocol::protocol::HookEventName::PermissionRequest => "permission-request",
            codex_protocol::protocol::HookEventName::PostToolUse => "post-tool-use",
            codex_protocol::protocol::HookEventName::SessionStart => "session-start",
            codex_protocol::protocol::HookEventName::UserPromptSubmit => "user-prompt-submit",
            codex_protocol::protocol::HookEventName::Stop => "stop",
        }
    }
}

#[derive(Clone)]
pub(crate) struct ClaudeHooksEngine {
    handlers: Vec<ConfiguredHandler>,
    warnings: Vec<String>,
    shell: CommandShell,
}

impl ClaudeHooksEngine {
    pub(crate) fn new(
        enabled: bool,
        config_layer_stack: Option<&ConfigLayerStack>,
        shell: CommandShell,
    ) -> Self {
        if !enabled {
            return Self {
                handlers: Vec::new(),
                warnings: Vec::new(),
                shell,
            };
        }

        let _ = schema_loader::generated_hook_schemas();
        let discovered = discovery::discover_handlers(config_layer_stack);
        Self {
            handlers: discovered.handlers,
            warnings: discovered.warnings,
            shell,
        }
    }

    pub(crate) fn warnings(&self) -> &[String] {
        &self.warnings
    }

    pub(crate) fn preview_session_start(
        &self,
        request: &SessionStartRequest,
    ) -> Vec<HookRunSummary> {
        crate::events::session_start::preview(&self.handlers, request)
    }

    pub(crate) fn preview_pre_tool_use(&self, request: &PreToolUseRequest) -> Vec<HookRunSummary> {
        crate::events::pre_tool_use::preview(&self.handlers, request)
    }

    pub(crate) fn preview_permission_request(
        &self,
        request: &PermissionRequestRequest,
    ) -> Vec<HookRunSummary> {
        crate::events::permission_request::preview(&self.handlers, request)
    }

    pub(crate) fn preview_post_tool_use(
        &self,
        request: &PostToolUseRequest,
    ) -> Vec<HookRunSummary> {
        crate::events::post_tool_use::preview(&self.handlers, request)
    }

    pub(crate) async fn run_session_start(
        &self,
        request: SessionStartRequest,
        turn_id: Option<String>,
    ) -> SessionStartOutcome {
        crate::events::session_start::run(&self.handlers, &self.shell, request, turn_id).await
    }

    pub(crate) async fn run_pre_tool_use(&self, request: PreToolUseRequest) -> PreToolUseOutcome {
        crate::events::pre_tool_use::run(&self.handlers, &self.shell, request).await
    }

    pub(crate) async fn run_permission_request(
        &self,
        request: PermissionRequestRequest,
    ) -> PermissionRequestOutcome {
        crate::events::permission_request::run(&self.handlers, &self.shell, request).await
    }

    pub(crate) async fn run_post_tool_use(
        &self,
        request: PostToolUseRequest,
    ) -> PostToolUseOutcome {
        crate::events::post_tool_use::run(&self.handlers, &self.shell, request).await
    }

    pub(crate) fn preview_user_prompt_submit(
        &self,
        request: &UserPromptSubmitRequest,
    ) -> Vec<HookRunSummary> {
        crate::events::user_prompt_submit::preview(&self.handlers, request)
    }

    pub(crate) async fn run_user_prompt_submit(
        &self,
        request: UserPromptSubmitRequest,
    ) -> UserPromptSubmitOutcome {
        crate::events::user_prompt_submit::run(&self.handlers, &self.shell, request).await
    }

    pub(crate) fn preview_stop(&self, request: &StopRequest) -> Vec<HookRunSummary> {
        crate::events::stop::preview(&self.handlers, request)
    }

    pub(crate) async fn run_stop(&self, request: StopRequest) -> StopOutcome {
        crate::events::stop::run(&self.handlers, &self.shell, request).await
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

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
    use codex_protocol::ThreadId;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    use super::ClaudeHooksEngine;
    use super::CommandShell;
    use crate::events::pre_tool_use::PreToolUseRequest;

    fn cwd() -> AbsolutePathBuf {
        AbsolutePathBuf::current_dir().expect("current dir")
    }

    fn managed_hooks_for_current_platform<P: Into<std::path::PathBuf>>(
        managed_dir: P,
        hooks: HookEventsToml,
    ) -> ManagedHooksRequirementsToml {
        let managed_dir = managed_dir.into();
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
            CommandShell {
                program: String::new(),
                args: Vec::new(),
            },
        );

        assert!(engine.warnings().is_empty());
        assert_eq!(engine.handlers.len(), 1);
        assert!(engine.handlers[0].is_managed);
        let cwd = cwd();
        let preview = engine.preview_pre_tool_use(&PreToolUseRequest {
            session_id: ThreadId::new(),
            turn_id: "turn-1".to_string(),
            cwd: cwd.clone(),
            transcript_path: None,
            model: "gpt-test".to_string(),
            permission_mode: "default".to_string(),
            tool_name: "Bash".to_string(),
            tool_use_id: "tool-1".to_string(),
            command: "echo hello".to_string(),
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
                tool_use_id: "tool-1".to_string(),
                command: "echo hello".to_string(),
            })
            .await;

        assert!(!outcome.should_block);
        let log_contents = fs::read_to_string(log_path).expect("read managed hook log");
        assert!(log_contents.contains("\"hook_event_name\": \"PreToolUse\""));
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
                    tool_use_id: "tool-1".to_string(),
                    command: "echo hello".to_string(),
                })
                .is_empty()
        );
    }

    #[test]
    fn discovers_hooks_from_json_and_toml_in_the_same_layer() {
        let temp = tempdir().expect("create temp dir");
        let config_path = AbsolutePathBuf::try_from(temp.path().join("config.toml"))
            .expect("absolute config path");
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
        let Some(TomlValue::Array(hooks_array)) = pre_tool_use_group_entries.get_mut("hooks")
        else {
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
            tool_use_id: "tool-1".to_string(),
            command: "echo hello".to_string(),
        });
        assert_eq!(preview.len(), 2);
        assert!(engine.handlers.iter().all(|handler| !handler.is_managed));
        assert_eq!(preview[0].source_path, hooks_json_path);
        assert_eq!(preview[1].source_path, config_path);
    }
}
