//! Skill hook registration.
//!
//! Converts skill frontmatter hooks to [`HookDefinition`]s and handles
//! registration and cleanup with the hook registry.
//!
//! ## Lifecycle
//!
//! 1. When a skill starts, call [`register_skill_hooks`] to add hooks
//! 2. The hooks are registered with [`HookSource::Skill`] scope
//! 3. When the skill ends, call [`cleanup_skill_hooks`] to remove them

use cocode_hooks::HookDefinition;
use cocode_hooks::HookEventType;
use cocode_hooks::HookHandler;
use cocode_hooks::HookMatcher;
use cocode_hooks::HookRegistry;
use cocode_hooks::HookSource;
use tracing::debug;
use tracing::warn;

use crate::interface::SkillHookConfig;
use crate::interface::SkillHookMatcher;
use crate::interface::SkillInterface;

/// Converts a [`SkillInterface`] hook configuration into [`HookDefinition`]s.
///
/// Returns a vector of hook definitions that can be registered with a registry.
pub fn convert_skill_hooks(interface: &SkillInterface) -> Vec<HookDefinition> {
    let Some(ref hooks_map) = interface.hooks else {
        return Vec::new();
    };

    let mut definitions = Vec::new();

    for (event_type_str, configs) in hooks_map {
        // Parse the event type
        let event_type = match parse_event_type(event_type_str) {
            Some(et) => et,
            None => {
                warn!(
                    skill = %interface.name,
                    event_type = %event_type_str,
                    "Unknown hook event type, skipping"
                );
                continue;
            }
        };

        for (idx, config) in configs.iter().enumerate() {
            if let Some(def) = convert_single_hook(&interface.name, event_type.clone(), config, idx)
            {
                definitions.push(def);
            }
        }
    }

    debug!(
        skill = %interface.name,
        hook_count = definitions.len(),
        "Converted skill hooks"
    );

    definitions
}

/// Register hooks from a skill interface with the registry.
///
/// Returns the number of hooks successfully registered.
pub fn register_skill_hooks(registry: &HookRegistry, interface: &SkillInterface) -> i32 {
    let definitions = convert_skill_hooks(interface);
    let count = definitions.len() as i32;

    for def in definitions {
        registry.register(def);
    }

    debug!(
        skill = %interface.name,
        count,
        "Registered skill hooks"
    );

    count
}

/// Remove all hooks registered by a specific skill.
pub fn cleanup_skill_hooks(registry: &HookRegistry, skill_name: &str) {
    registry.remove_hooks_by_source_name(skill_name);

    debug!(skill = skill_name, "Cleaned up skill hooks");
}

/// Parse an event type string into a [`HookEventType`].
fn parse_event_type(s: &str) -> Option<HookEventType> {
    // Support both PascalCase (from TOML keys) and snake_case
    match s {
        "PreToolUse" | "pre_tool_use" => Some(HookEventType::PreToolUse),
        "PostToolUse" | "post_tool_use" => Some(HookEventType::PostToolUse),
        "PostToolUseFailure" | "post_tool_use_failure" => Some(HookEventType::PostToolUseFailure),
        "UserPromptSubmit" | "user_prompt_submit" => Some(HookEventType::UserPromptSubmit),
        "SessionStart" | "session_start" => Some(HookEventType::SessionStart),
        "SessionEnd" | "session_end" => Some(HookEventType::SessionEnd),
        "Stop" | "stop" => Some(HookEventType::Stop),
        "SubagentStart" | "subagent_start" => Some(HookEventType::SubagentStart),
        "SubagentStop" | "subagent_stop" => Some(HookEventType::SubagentStop),
        "PreCompact" | "pre_compact" => Some(HookEventType::PreCompact),
        "Notification" | "notification" => Some(HookEventType::Notification),
        "PermissionRequest" | "permission_request" => Some(HookEventType::PermissionRequest),
        _ => None,
    }
}

/// Convert a single skill hook config to a hook definition.
fn convert_single_hook(
    skill_name: &str,
    event_type: HookEventType,
    config: &SkillHookConfig,
    index: usize,
) -> Option<HookDefinition> {
    // Determine the handler type
    let handler = if let Some(ref command) = config.command {
        HookHandler::Command {
            command: command.clone(),
            args: config.args.clone().unwrap_or_default(),
        }
    } else {
        warn!(
            skill = %skill_name,
            index,
            "Skill hook has no command, skipping"
        );
        return None;
    };

    // Convert the matcher
    let matcher = config.matcher.as_ref().map(convert_matcher);

    let hook_name = format!("{}:hook:{}", skill_name, index);

    Some(HookDefinition {
        name: hook_name,
        event_type,
        matcher,
        handler,
        source: HookSource::Skill {
            name: skill_name.to_string(),
        },
        enabled: true,
        timeout_secs: config.timeout_secs,
        once: config.once,
    })
}

/// Convert a skill hook matcher to a hook matcher.
fn convert_matcher(matcher: &SkillHookMatcher) -> HookMatcher {
    match matcher {
        SkillHookMatcher::Exact { value } => HookMatcher::Exact {
            value: value.clone(),
        },
        SkillHookMatcher::Wildcard { pattern } => HookMatcher::Wildcard {
            pattern: pattern.clone(),
        },
        SkillHookMatcher::Regex { pattern } => HookMatcher::Regex {
            pattern: pattern.clone(),
        },
        SkillHookMatcher::Or { matchers } => HookMatcher::Or {
            matchers: matchers.iter().map(convert_matcher).collect(),
        },
        SkillHookMatcher::All => HookMatcher::All,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_interface_with_hooks(hooks: HashMap<String, Vec<SkillHookConfig>>) -> SkillInterface {
        SkillInterface {
            name: "test-skill".to_string(),
            description: "A test skill".to_string(),
            prompt_file: None,
            prompt_inline: Some("test prompt".to_string()),
            allowed_tools: None,
            hooks: Some(hooks),
        }
    }

    #[test]
    fn test_parse_event_type_pascal_case() {
        assert_eq!(
            parse_event_type("PreToolUse"),
            Some(HookEventType::PreToolUse)
        );
        assert_eq!(
            parse_event_type("PostToolUse"),
            Some(HookEventType::PostToolUse)
        );
        assert_eq!(
            parse_event_type("SessionStart"),
            Some(HookEventType::SessionStart)
        );
    }

    #[test]
    fn test_parse_event_type_snake_case() {
        assert_eq!(
            parse_event_type("pre_tool_use"),
            Some(HookEventType::PreToolUse)
        );
        assert_eq!(
            parse_event_type("post_tool_use"),
            Some(HookEventType::PostToolUse)
        );
        assert_eq!(
            parse_event_type("session_start"),
            Some(HookEventType::SessionStart)
        );
    }

    #[test]
    fn test_parse_event_type_unknown() {
        assert_eq!(parse_event_type("unknown_event"), None);
        assert_eq!(parse_event_type(""), None);
    }

    #[test]
    fn test_convert_skill_hooks_empty() {
        let interface = SkillInterface {
            name: "test".to_string(),
            description: "Test".to_string(),
            prompt_file: None,
            prompt_inline: Some("test".to_string()),
            allowed_tools: None,
            hooks: None,
        };
        let defs = convert_skill_hooks(&interface);
        assert!(defs.is_empty());
    }

    #[test]
    fn test_convert_skill_hooks_single() {
        let mut hooks = HashMap::new();
        hooks.insert(
            "PreToolUse".to_string(),
            vec![SkillHookConfig {
                matcher: Some(SkillHookMatcher::Exact {
                    value: "Write".to_string(),
                }),
                command: Some("npm run lint".to_string()),
                args: Some(vec!["--fix".to_string()]),
                timeout_secs: 60,
                once: true,
            }],
        );

        let interface = make_interface_with_hooks(hooks);
        let defs = convert_skill_hooks(&interface);

        assert_eq!(defs.len(), 1);
        let def = &defs[0];
        assert_eq!(def.name, "test-skill:hook:0");
        assert_eq!(def.event_type, HookEventType::PreToolUse);
        assert!(def.once);
        assert_eq!(def.timeout_secs, 60);

        if let HookHandler::Command { command, args } = &def.handler {
            assert_eq!(command, "npm run lint");
            assert_eq!(args, &vec!["--fix".to_string()]);
        } else {
            panic!("Expected Command handler");
        }

        if let Some(HookMatcher::Exact { value }) = &def.matcher {
            assert_eq!(value, "Write");
        } else {
            panic!("Expected Exact matcher");
        }

        if let HookSource::Skill { name } = &def.source {
            assert_eq!(name, "test-skill");
        } else {
            panic!("Expected Skill source");
        }
    }

    #[test]
    fn test_convert_skill_hooks_multiple() {
        let mut hooks = HashMap::new();
        hooks.insert(
            "PreToolUse".to_string(),
            vec![
                SkillHookConfig {
                    matcher: None,
                    command: Some("echo pre".to_string()),
                    args: None,
                    timeout_secs: 30,
                    once: false,
                },
                SkillHookConfig {
                    matcher: None,
                    command: Some("echo pre2".to_string()),
                    args: None,
                    timeout_secs: 30,
                    once: false,
                },
            ],
        );
        hooks.insert(
            "PostToolUse".to_string(),
            vec![SkillHookConfig {
                matcher: None,
                command: Some("echo post".to_string()),
                args: None,
                timeout_secs: 30,
                once: false,
            }],
        );

        let interface = make_interface_with_hooks(hooks);
        let defs = convert_skill_hooks(&interface);

        assert_eq!(defs.len(), 3);
    }

    #[test]
    fn test_convert_matcher_or() {
        let skill_matcher = SkillHookMatcher::Or {
            matchers: vec![
                SkillHookMatcher::Exact {
                    value: "Write".to_string(),
                },
                SkillHookMatcher::Exact {
                    value: "Edit".to_string(),
                },
            ],
        };

        let hook_matcher = convert_matcher(&skill_matcher);

        if let HookMatcher::Or { matchers } = hook_matcher {
            assert_eq!(matchers.len(), 2);
        } else {
            panic!("Expected Or matcher");
        }
    }

    #[test]
    fn test_convert_matcher_all() {
        let skill_matcher = SkillHookMatcher::All;
        let hook_matcher = convert_matcher(&skill_matcher);
        assert!(matches!(hook_matcher, HookMatcher::All));
    }

    #[test]
    fn test_convert_skill_hooks_skips_no_command() {
        let mut hooks = HashMap::new();
        hooks.insert(
            "PreToolUse".to_string(),
            vec![SkillHookConfig {
                matcher: None,
                command: None, // No command
                args: None,
                timeout_secs: 30,
                once: false,
            }],
        );

        let interface = make_interface_with_hooks(hooks);
        let defs = convert_skill_hooks(&interface);

        assert!(defs.is_empty());
    }

    #[test]
    fn test_convert_skill_hooks_skips_unknown_event() {
        let mut hooks = HashMap::new();
        hooks.insert(
            "UnknownEvent".to_string(),
            vec![SkillHookConfig {
                matcher: None,
                command: Some("echo".to_string()),
                args: None,
                timeout_secs: 30,
                once: false,
            }],
        );

        let interface = make_interface_with_hooks(hooks);
        let defs = convert_skill_hooks(&interface);

        assert!(defs.is_empty());
    }

    #[test]
    fn test_register_and_cleanup_skill_hooks() {
        let registry = HookRegistry::new();

        let mut hooks = HashMap::new();
        hooks.insert(
            "PreToolUse".to_string(),
            vec![SkillHookConfig {
                matcher: None,
                command: Some("echo test".to_string()),
                args: None,
                timeout_secs: 30,
                once: false,
            }],
        );

        let interface = make_interface_with_hooks(hooks);

        // Register
        let count = register_skill_hooks(&registry, &interface);
        assert_eq!(count, 1);

        // Verify registered
        let all = registry.all_hooks();
        assert_eq!(all.len(), 1);

        // Cleanup
        cleanup_skill_hooks(&registry, "test-skill");

        // Verify removed
        let all = registry.all_hooks();
        assert!(all.is_empty());
    }
}
