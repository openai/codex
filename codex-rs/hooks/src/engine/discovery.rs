use std::fs;
use std::path::Path;

use codex_config::ConfigLayerStack;
use codex_config::ConfigLayerStackOrdering;

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
                source_path.as_path(),
                event_name,
                groups,
            );
        }
    }

    DiscoveryResult { handlers, warnings }
}

fn append_group_handlers(
    handlers: &mut Vec<ConfiguredHandler>,
    warnings: &mut Vec<String>,
    display_order: &mut i64,
    source_path: &Path,
    event_name: codex_protocol::protocol::HookEventName,
    matcher: Option<&str>,
    group_handlers: Vec<HookHandlerConfig>,
) {
    if let Some(matcher) = matcher
        && let Err(err) = validate_matcher_pattern(matcher)
    {
        warnings.push(format!(
            "invalid matcher {matcher:?} in {}: {err}",
            source_path.display()
        ));
        return;
    }

    for handler in group_handlers {
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
                    source_path: source_path.to_path_buf(),
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

fn append_matcher_groups(
    handlers: &mut Vec<ConfiguredHandler>,
    warnings: &mut Vec<String>,
    display_order: &mut i64,
    source_path: &Path,
    event_name: codex_protocol::protocol::HookEventName,
    groups: Vec<MatcherGroup>,
) {
    for group in groups {
        append_group_handlers(
            handlers,
            warnings,
            display_order,
            source_path,
            event_name,
            matcher_pattern_for_event(event_name, group.matcher.as_deref()),
            group.hooks,
        );
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::path::PathBuf;

    use codex_protocol::protocol::HookEventName;
    use pretty_assertions::assert_eq;

    use super::ConfiguredHandler;
    use super::HookHandlerConfig;
    use super::append_group_handlers;
    use crate::events::common::matcher_pattern_for_event;

    #[test]
    fn user_prompt_submit_ignores_invalid_matcher_during_discovery() {
        let mut handlers = Vec::new();
        let mut warnings = Vec::new();
        let mut display_order = 0;

        append_group_handlers(
            &mut handlers,
            &mut warnings,
            &mut display_order,
            Path::new("/tmp/hooks.json"),
            HookEventName::UserPromptSubmit,
            matcher_pattern_for_event(HookEventName::UserPromptSubmit, Some("[")),
            vec![HookHandlerConfig::Command {
                command: "echo hello".to_string(),
                timeout_sec: None,
                r#async: false,
                status_message: None,
            }],
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
                source_path: PathBuf::from("/tmp/hooks.json"),
                display_order: 0,
            }]
        );
    }

    #[test]
    fn pre_tool_use_keeps_valid_matcher_during_discovery() {
        let mut handlers = Vec::new();
        let mut warnings = Vec::new();
        let mut display_order = 0;

        append_group_handlers(
            &mut handlers,
            &mut warnings,
            &mut display_order,
            Path::new("/tmp/hooks.json"),
            HookEventName::PreToolUse,
            matcher_pattern_for_event(HookEventName::PreToolUse, Some("^Bash$")),
            vec![HookHandlerConfig::Command {
                command: "echo hello".to_string(),
                timeout_sec: None,
                r#async: false,
                status_message: None,
            }],
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
                source_path: PathBuf::from("/tmp/hooks.json"),
                display_order: 0,
            }]
        );
    }

    #[test]
    fn pre_tool_use_treats_star_matcher_as_match_all() {
        let mut handlers = Vec::new();
        let mut warnings = Vec::new();
        let mut display_order = 0;

        append_group_handlers(
            &mut handlers,
            &mut warnings,
            &mut display_order,
            Path::new("/tmp/hooks.json"),
            HookEventName::PreToolUse,
            matcher_pattern_for_event(HookEventName::PreToolUse, Some("*")),
            vec![HookHandlerConfig::Command {
                command: "echo hello".to_string(),
                timeout_sec: None,
                r#async: false,
                status_message: None,
            }],
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

        append_group_handlers(
            &mut handlers,
            &mut warnings,
            &mut display_order,
            Path::new("/tmp/hooks.json"),
            HookEventName::PostToolUse,
            matcher_pattern_for_event(HookEventName::PostToolUse, Some("Edit|Write")),
            vec![HookHandlerConfig::Command {
                command: "echo hello".to_string(),
                timeout_sec: None,
                r#async: false,
                status_message: None,
            }],
        );

        assert_eq!(warnings, Vec::<String>::new());
        assert_eq!(handlers.len(), 1);
        assert_eq!(handlers[0].event_name, HookEventName::PostToolUse);
        assert_eq!(handlers[0].matcher.as_deref(), Some("Edit|Write"));
    }
}
