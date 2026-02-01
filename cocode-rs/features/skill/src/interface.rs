//! Skill interface definition (`SKILL.toml` schema).
//!
//! Each skill directory contains a `SKILL.toml` file that describes the
//! skill's metadata and prompt content. This module defines the
//! deserialization target for that file.

use std::collections::HashMap;

use serde::Deserialize;
use serde::Serialize;

/// Metadata and content of a skill, as defined in `SKILL.toml`.
///
/// A skill must have a `name` and `description`. The prompt content can be
/// provided either inline (`prompt_inline`) or by referencing an external
/// file (`prompt_file`). If both are specified, `prompt_file` takes
/// precedence.
///
/// # Example SKILL.toml
///
/// ```toml
/// name = "commit"
/// description = "Generate a commit message from staged changes"
/// prompt_file = "prompt.md"
/// allowed_tools = ["Bash", "Read"]
///
/// # Optional hooks that run when this skill is active
/// [hooks.PreToolUse]
/// matcher = { type = "or", matchers = [{ type = "exact", value = "Write" }, { type = "exact", value = "Edit" }] }
/// command = "npm run lint"
/// once = true
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInterface {
    /// Unique skill name (used as slash-command identifier).
    pub name: String,

    /// Human-readable description.
    pub description: String,

    /// Path to an external file containing the prompt text.
    /// Relative to the skill directory.
    #[serde(default)]
    pub prompt_file: Option<String>,

    /// Inline prompt text (used when `prompt_file` is not set).
    #[serde(default)]
    pub prompt_inline: Option<String>,

    /// Tools the skill is allowed to invoke.
    #[serde(default)]
    pub allowed_tools: Option<Vec<String>>,

    /// Hooks that are registered when this skill starts and removed when it ends.
    ///
    /// The key is the event type (e.g., "PreToolUse", "PostToolUse"), and the
    /// value is a list of hook configurations for that event.
    ///
    /// These hooks are scoped to the skill and automatically cleaned up when
    /// the skill finishes executing.
    #[serde(default)]
    pub hooks: Option<HashMap<String, Vec<SkillHookConfig>>>,
}

/// Hook configuration within a skill's SKILL.toml.
///
/// This is a simplified hook definition that maps to [`HookDefinition`]
/// when the skill is loaded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillHookConfig {
    /// Optional matcher to filter which tool calls trigger this hook.
    #[serde(default)]
    pub matcher: Option<SkillHookMatcher>,

    /// Command to execute. The command receives hook context as JSON on stdin.
    #[serde(default)]
    pub command: Option<String>,

    /// Arguments for the command.
    #[serde(default)]
    pub args: Option<Vec<String>>,

    /// Timeout in seconds (default: 30).
    #[serde(default = "default_timeout")]
    pub timeout_secs: i32,

    /// If true, the hook is removed after its first successful execution.
    #[serde(default)]
    pub once: bool,
}

fn default_timeout() -> i32 {
    30
}

/// Matcher configuration for skill hooks.
///
/// Simplified version of HookMatcher for SKILL.toml.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SkillHookMatcher {
    /// Exact string match.
    Exact { value: String },
    /// Wildcard pattern with `*` and `?`.
    Wildcard { pattern: String },
    /// Regular expression.
    Regex { pattern: String },
    /// Match any of the given matchers.
    Or { matchers: Vec<SkillHookMatcher> },
    /// Match all values.
    All,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_full() {
        let toml_str = r#"
name = "commit"
description = "Generate a commit message"
prompt_file = "prompt.md"
allowed_tools = ["Bash", "Read"]
"#;
        let iface: SkillInterface = toml::from_str(toml_str).expect("parse SKILL.toml");
        assert_eq!(iface.name, "commit");
        assert_eq!(iface.description, "Generate a commit message");
        assert_eq!(iface.prompt_file, Some("prompt.md".to_string()));
        assert!(iface.prompt_inline.is_none());
        assert_eq!(
            iface.allowed_tools,
            Some(vec!["Bash".to_string(), "Read".to_string()])
        );
    }

    #[test]
    fn test_deserialize_inline_prompt() {
        let toml_str = r#"
name = "review"
description = "Review code"
prompt_inline = "Please review the following code changes."
"#;
        let iface: SkillInterface = toml::from_str(toml_str).expect("parse SKILL.toml");
        assert_eq!(iface.name, "review");
        assert_eq!(
            iface.prompt_inline,
            Some("Please review the following code changes.".to_string())
        );
        assert!(iface.prompt_file.is_none());
        assert!(iface.allowed_tools.is_none());
    }

    #[test]
    fn test_deserialize_minimal() {
        let toml_str = r#"
name = "test"
description = "A test skill"
"#;
        let iface: SkillInterface = toml::from_str(toml_str).expect("parse SKILL.toml");
        assert_eq!(iface.name, "test");
        assert!(iface.prompt_file.is_none());
        assert!(iface.prompt_inline.is_none());
        assert!(iface.allowed_tools.is_none());
    }

    #[test]
    fn test_serialize_roundtrip() {
        let iface = SkillInterface {
            name: "roundtrip".to_string(),
            description: "Roundtrip test".to_string(),
            prompt_file: None,
            prompt_inline: Some("Do things".to_string()),
            allowed_tools: Some(vec!["Bash".to_string()]),
            hooks: None,
        };
        let serialized = toml::to_string(&iface).expect("serialize");
        let deserialized: SkillInterface = toml::from_str(&serialized).expect("deserialize");
        assert_eq!(deserialized.name, "roundtrip");
        assert_eq!(deserialized.prompt_inline, Some("Do things".to_string()));
        assert_eq!(deserialized.allowed_tools, Some(vec!["Bash".to_string()]));
    }

    #[test]
    fn test_deserialize_with_hooks() {
        let toml_str = r#"
name = "lint-check"
description = "Skill with hooks"
prompt_inline = "Do the thing"

[[hooks.PreToolUse]]
command = "npm run lint"
timeout_secs = 10
once = true

[hooks.PreToolUse.matcher]
type = "exact"
value = "Write"
"#;
        let iface: SkillInterface = toml::from_str(toml_str).expect("parse SKILL.toml");
        assert_eq!(iface.name, "lint-check");
        assert!(iface.hooks.is_some());

        let hooks = iface.hooks.unwrap();
        let pre_hooks = hooks.get("PreToolUse").expect("PreToolUse hooks");
        assert_eq!(pre_hooks.len(), 1);
        assert_eq!(pre_hooks[0].command, Some("npm run lint".to_string()));
        assert_eq!(pre_hooks[0].timeout_secs, 10);
        assert!(pre_hooks[0].once);
    }

    #[test]
    fn test_deserialize_hook_or_matcher() {
        let toml_str = r#"
name = "multi-matcher"
description = "Skill with OR matcher"
prompt_inline = "test"

[[hooks.PreToolUse]]
command = "check"

[hooks.PreToolUse.matcher]
type = "or"

[[hooks.PreToolUse.matcher.matchers]]
type = "exact"
value = "Write"

[[hooks.PreToolUse.matcher.matchers]]
type = "exact"
value = "Edit"
"#;
        let iface: SkillInterface = toml::from_str(toml_str).expect("parse SKILL.toml");
        let hooks = iface.hooks.expect("hooks");
        let pre_hooks = hooks.get("PreToolUse").expect("PreToolUse");

        if let Some(SkillHookMatcher::Or { matchers }) = &pre_hooks[0].matcher {
            assert_eq!(matchers.len(), 2);
        } else {
            panic!("Expected Or matcher");
        }
    }

    #[test]
    fn test_skill_hook_config_defaults() {
        let toml_str = r#"
name = "defaults"
description = "Test defaults"
prompt_inline = "test"

[[hooks.PostToolUse]]
command = "echo done"
"#;
        let iface: SkillInterface = toml::from_str(toml_str).expect("parse");
        let hooks = iface.hooks.expect("hooks");
        let post_hooks = hooks.get("PostToolUse").expect("PostToolUse");

        assert_eq!(post_hooks[0].timeout_secs, 30); // default
        assert!(!post_hooks[0].once); // default false
        assert!(post_hooks[0].matcher.is_none()); // no matcher
        assert!(post_hooks[0].args.is_none()); // no args
    }
}
