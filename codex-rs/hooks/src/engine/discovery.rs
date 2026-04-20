use codex_config::ConfigLayerSource;
use codex_config::ConfigLayerStack;
use codex_config::ConfigLayerStackOrdering;
use codex_protocol::protocol::HookSource;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::fs;

use super::ConfiguredHandler;
use super::config::HookHandlerConfig;
use super::config::HooksFile;
use super::config::MatcherGroup;
use crate::events::common::matcher_pattern_for_event;
use crate::events::common::validate_matcher_pattern;

pub(crate) struct DiscoveryResult {
    pub handlers: Vec<ConfiguredHandler>,
    pub warnings: Vec<String>,
}

pub(crate) fn discover_handlers(config_layer_stack: Option<&ConfigLayerStack>) -> DiscoveryResult {
    let Some(config_layer_stack) = config_layer_stack else {
        return DiscoveryResult {
            handlers: Vec::new(),
            warnings: Vec::new(),
        };
    };

    let mut handlers = Vec::new();
    let mut warnings = Vec::new();
    let mut display_order = 0_i64;
    let allow_managed_hooks_only = config_layer_stack
        .requirements()
        .allow_managed_hooks_only
        .as_ref()
        .is_some_and(|requirement| requirement.value);

    for layer in config_layer_stack.get_layers(
        ConfigLayerStackOrdering::LowestPrecedenceFirst,
        /*include_disabled*/ false,
    ) {
        let Some(folder) = layer.config_folder() else {
            continue;
        };
        let source_path = folder.join("hooks.json");
        if !source_path.as_path().is_file() {
            continue;
        }
        if allow_managed_hooks_only && !layer.is_managed() {
            warnings.push(format!(
                "skipping hooks config {} because `allow_managed_hooks_only` is enabled",
                source_path.display()
            ));
            continue;
        }

        let contents = match fs::read_to_string(source_path.as_path()) {
            Ok(contents) => contents,
            Err(err) => {
                warnings.push(format!(
                    "failed to read hooks config {}: {err}",
                    source_path.display()
                ));
                continue;
            }
        };

        let parsed: HooksFile = match serde_json::from_str(&contents) {
            Ok(parsed) => parsed,
            Err(err) => {
                warnings.push(format!(
                    "failed to parse hooks config {}: {err}",
                    source_path.display()
                ));
                continue;
            }
        };

        let super::config::HookEvents {
            pre_tool_use,
            permission_request,
            post_tool_use,
            session_start,
            user_prompt_submit,
            stop,
        } = parsed.hooks;

        for (event_name, groups) in [
            (
                codex_protocol::protocol::HookEventName::PreToolUse,
                pre_tool_use,
            ),
            (
                codex_protocol::protocol::HookEventName::PermissionRequest,
                permission_request,
            ),
            (
                codex_protocol::protocol::HookEventName::PostToolUse,
                post_tool_use,
            ),
            (
                codex_protocol::protocol::HookEventName::SessionStart,
                session_start,
            ),
            (
                codex_protocol::protocol::HookEventName::UserPromptSubmit,
                user_prompt_submit,
            ),
            (codex_protocol::protocol::HookEventName::Stop, stop),
        ] {
            append_matcher_groups(
                &mut handlers,
                &mut warnings,
                &mut display_order,
                &source_path,
                hook_source_for_config_layer_source(&layer.name),
                event_name,
                groups,
            );
        }
    }

    if !handlers.is_empty() {
        let mut source_paths = handlers
            .iter()
            .map(|handler| handler.source_path.display().to_string())
            .collect::<Vec<_>>();
        source_paths.sort();
        source_paths.dedup();
        warnings.push(format!(
            "Loaded {} lifecycle hook(s) from {}. Hooks run arbitrary shell commands outside the sandbox; review hooks.json changes before continuing.",
            handlers.len(),
            source_paths.join(", ")
        ));
    }

    DiscoveryResult { handlers, warnings }
}

fn append_matcher_groups(
    handlers: &mut Vec<ConfiguredHandler>,
    warnings: &mut Vec<String>,
    display_order: &mut i64,
    source_path: &AbsolutePathBuf,
    source: HookSource,
    event_name: codex_protocol::protocol::HookEventName,
    groups: Vec<MatcherGroup>,
) {
    for group in groups {
        let matcher = matcher_pattern_for_event(event_name, group.matcher.as_deref());
        if let Some(matcher) = matcher
            && let Err(err) = validate_matcher_pattern(matcher)
        {
            warnings.push(format!(
                "invalid matcher {matcher:?} in {}: {err}",
                source_path.display()
            ));
            continue;
        }

        for handler in group.hooks {
            match handler {
                HookHandlerConfig::Command {
                    command,
                    timeout_sec,
                    r#async,
                    status_message,
                } => {
                    if r#async {
                        warnings.push(format!(
                            "skipping async hook in {}: async hooks are not supported yet",
                            source_path.display()
                        ));
                        continue;
                    }
                    if command.trim().is_empty() {
                        warnings.push(format!(
                            "skipping empty hook command in {}",
                            source_path.display()
                        ));
                        continue;
                    }
                    let timeout_sec = timeout_sec.unwrap_or(600).max(1);
                    handlers.push(ConfiguredHandler {
                        event_name,
                        matcher: matcher.map(ToOwned::to_owned),
                        command,
                        timeout_sec,
                        status_message,
                        source_path: source_path.clone(),
                        source,
                        display_order: *display_order,
                    });
                    *display_order += 1;
                }
                HookHandlerConfig::Prompt {} => warnings.push(format!(
                    "skipping prompt hook in {}: prompt hooks are not supported yet",
                    source_path.display()
                )),
                HookHandlerConfig::Agent {} => warnings.push(format!(
                    "skipping agent hook in {}: agent hooks are not supported yet",
                    source_path.display()
                )),
            }
        }
    }
}

fn hook_source_for_config_layer_source(source: &ConfigLayerSource) -> HookSource {
    match source {
        ConfigLayerSource::System { .. } => HookSource::System,
        ConfigLayerSource::User { .. } => HookSource::User,
        ConfigLayerSource::Project { .. } => HookSource::Project,
        ConfigLayerSource::Mdm { .. } => HookSource::Mdm,
        ConfigLayerSource::SessionFlags => HookSource::SessionFlags,
        ConfigLayerSource::LegacyManagedConfigTomlFromFile { .. } => {
            HookSource::LegacyManagedConfigFile
        }
        ConfigLayerSource::LegacyManagedConfigTomlFromMdm => HookSource::LegacyManagedConfigMdm,
    }
}

#[cfg(test)]
mod tests {
    use codex_config::ConfigLayerSource;
    use codex_protocol::protocol::HookEventName;
    use codex_protocol::protocol::HookSource;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use codex_utils_absolute_path::test_support::test_path_buf;
    use pretty_assertions::assert_eq;

    use super::ConfiguredHandler;
    use super::HookHandlerConfig;
    use super::MatcherGroup;
    use super::append_matcher_groups;

    fn source_path() -> AbsolutePathBuf {
        test_path_buf("/tmp/hooks.json").abs()
    }

    fn hook_source() -> HookSource {
        HookSource::User
    }

    fn command_group(matcher: Option<&str>) -> MatcherGroup {
        MatcherGroup {
            matcher: matcher.map(str::to_string),
            hooks: vec![HookHandlerConfig::Command {
                command: "echo hello".to_string(),
                timeout_sec: None,
                r#async: false,
                status_message: None,
            }],
        }
    }

    #[test]
    fn user_prompt_submit_ignores_invalid_matcher_during_discovery() {
        let mut handlers = Vec::new();
        let mut warnings = Vec::new();
        let mut display_order = 0;

        append_matcher_groups(
            &mut handlers,
            &mut warnings,
            &mut display_order,
            &source_path(),
            hook_source(),
            HookEventName::UserPromptSubmit,
            vec![command_group(Some("["))],
        );

        assert_eq!(warnings, Vec::<String>::new());
        assert_eq!(
            handlers,
            vec![ConfiguredHandler {
                event_name: HookEventName::UserPromptSubmit,
                matcher: None,
                command: "echo hello".to_string(),
                timeout_sec: 600,
                status_message: None,
                source_path: source_path(),
                source: hook_source(),
                display_order: 0,
            }]
        );
    }

    #[test]
    fn pre_tool_use_keeps_valid_matcher_during_discovery() {
        let mut handlers = Vec::new();
        let mut warnings = Vec::new();
        let mut display_order = 0;

        append_matcher_groups(
            &mut handlers,
            &mut warnings,
            &mut display_order,
            &source_path(),
            hook_source(),
            HookEventName::PreToolUse,
            vec![command_group(Some("^Bash$"))],
        );

        assert_eq!(warnings, Vec::<String>::new());
        assert_eq!(
            handlers,
            vec![ConfiguredHandler {
                event_name: HookEventName::PreToolUse,
                matcher: Some("^Bash$".to_string()),
                command: "echo hello".to_string(),
                timeout_sec: 600,
                status_message: None,
                source_path: source_path(),
                source: hook_source(),
                display_order: 0,
            }]
        );
    }

    #[test]
    fn pre_tool_use_treats_star_matcher_as_match_all() {
        let mut handlers = Vec::new();
        let mut warnings = Vec::new();
        let mut display_order = 0;

        append_matcher_groups(
            &mut handlers,
            &mut warnings,
            &mut display_order,
            &source_path(),
            hook_source(),
            HookEventName::PreToolUse,
            vec![command_group(Some("*"))],
        );

        assert_eq!(warnings, Vec::<String>::new());
        assert_eq!(handlers.len(), 1);
        assert_eq!(handlers[0].matcher.as_deref(), Some("*"));
    }

    #[test]
    fn post_tool_use_keeps_valid_matcher_during_discovery() {
        let mut handlers = Vec::new();
        let mut warnings = Vec::new();
        let mut display_order = 0;

        append_matcher_groups(
            &mut handlers,
            &mut warnings,
            &mut display_order,
            &source_path(),
            hook_source(),
            HookEventName::PostToolUse,
            vec![command_group(Some("Edit|Write"))],
        );

        assert_eq!(warnings, Vec::<String>::new());
        assert_eq!(handlers.len(), 1);
        assert_eq!(handlers[0].event_name, HookEventName::PostToolUse);
        assert_eq!(handlers[0].matcher.as_deref(), Some("Edit|Write"));
    }

    #[test]
    fn hook_source_for_config_layer_source_discards_source_details() {
        let config_file = test_path_buf("/tmp/.codex/config.toml").abs();
        let dot_codex_folder = test_path_buf("/tmp/worktree/.codex").abs();

        assert_eq!(
            super::hook_source_for_config_layer_source(&ConfigLayerSource::System {
                file: config_file.clone(),
            }),
            HookSource::System,
        );
        assert_eq!(
            super::hook_source_for_config_layer_source(&ConfigLayerSource::User {
                file: config_file.clone(),
            }),
            HookSource::User,
        );
        assert_eq!(
            super::hook_source_for_config_layer_source(&ConfigLayerSource::Project {
                dot_codex_folder
            }),
            HookSource::Project,
        );
        assert_eq!(
            super::hook_source_for_config_layer_source(&ConfigLayerSource::Mdm {
                domain: "com.openai.codex".to_string(),
                key: "config".to_string(),
            }),
            HookSource::Mdm,
        );
        assert_eq!(
            super::hook_source_for_config_layer_source(&ConfigLayerSource::SessionFlags),
            HookSource::SessionFlags,
        );
        assert_eq!(
            super::hook_source_for_config_layer_source(
                &ConfigLayerSource::LegacyManagedConfigTomlFromFile { file: config_file },
            ),
            HookSource::LegacyManagedConfigFile,
        );
        assert_eq!(
            super::hook_source_for_config_layer_source(
                &ConfigLayerSource::LegacyManagedConfigTomlFromMdm,
            ),
            HookSource::LegacyManagedConfigMdm,
        );
    }
}
