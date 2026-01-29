//! Configuration loading for hooks.
//!
//! Loads hook definitions from TOML files.

use std::path::Path;

use serde::Deserialize;
use tracing::debug;

use crate::definition::{HookDefinition, HookHandler};
use crate::event::HookEventType;
use crate::matcher::HookMatcher;

/// Top-level TOML structure for hook configuration.
#[derive(Debug, Deserialize)]
struct HooksToml {
    #[serde(default)]
    hooks: Vec<HookTomlEntry>,
}

/// A single hook entry in TOML format.
#[derive(Debug, Deserialize)]
struct HookTomlEntry {
    name: String,
    event: HookEventType,
    #[serde(default)]
    matcher: Option<HookMatcherToml>,
    handler: HookHandlerToml,
    #[serde(default = "default_enabled")]
    enabled: bool,
    #[serde(default = "default_timeout")]
    timeout_secs: i32,
}

fn default_enabled() -> bool {
    true
}

fn default_timeout() -> i32 {
    30
}

/// TOML representation of a hook matcher.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum HookMatcherToml {
    Exact { value: String },
    Wildcard { pattern: String },
    Regex { pattern: String },
    All,
    Or { matchers: Vec<HookMatcherToml> },
}

/// TOML representation of a hook handler.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum HookHandlerToml {
    Command {
        command: String,
        #[serde(default)]
        args: Vec<String>,
    },
    Prompt {
        template: String,
    },
    Agent {
        #[serde(default = "default_max_turns")]
        max_turns: i32,
    },
    Webhook {
        url: String,
    },
}

fn default_max_turns() -> i32 {
    5
}

impl From<HookMatcherToml> for HookMatcher {
    fn from(toml: HookMatcherToml) -> Self {
        match toml {
            HookMatcherToml::Exact { value } => HookMatcher::Exact { value },
            HookMatcherToml::Wildcard { pattern } => HookMatcher::Wildcard { pattern },
            HookMatcherToml::Regex { pattern } => HookMatcher::Regex { pattern },
            HookMatcherToml::All => HookMatcher::All,
            HookMatcherToml::Or { matchers } => HookMatcher::Or {
                matchers: matchers.into_iter().map(Into::into).collect(),
            },
        }
    }
}

impl From<HookHandlerToml> for HookHandler {
    fn from(toml: HookHandlerToml) -> Self {
        match toml {
            HookHandlerToml::Command { command, args } => HookHandler::Command { command, args },
            HookHandlerToml::Prompt { template } => HookHandler::Prompt { template },
            HookHandlerToml::Agent { max_turns } => HookHandler::Agent { max_turns },
            HookHandlerToml::Webhook { url } => HookHandler::Webhook { url },
        }
    }
}

impl From<HookTomlEntry> for HookDefinition {
    fn from(entry: HookTomlEntry) -> Self {
        HookDefinition {
            name: entry.name,
            event_type: entry.event,
            matcher: entry.matcher.map(Into::into),
            handler: entry.handler.into(),
            enabled: entry.enabled,
            timeout_secs: entry.timeout_secs,
        }
    }
}

/// Loads hook definitions from a TOML file.
///
/// The file should have the following structure:
///
/// ```toml
/// [[hooks]]
/// name = "lint-check"
/// event = "pre_tool_use"
/// timeout_secs = 10
///
/// [hooks.matcher]
/// type = "exact"
/// value = "bash"
///
/// [hooks.handler]
/// type = "command"
/// command = "lint"
/// args = ["--check"]
/// ```
pub fn load_hooks_from_toml(path: &Path) -> Result<Vec<HookDefinition>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read hooks file '{}': {e}", path.display()))?;

    let hooks_toml: HooksToml = toml::from_str(&content)
        .map_err(|e| format!("failed to parse hooks TOML '{}': {e}", path.display()))?;

    let definitions: Vec<HookDefinition> = hooks_toml.hooks.into_iter().map(Into::into).collect();

    // Validate all matchers
    for def in &definitions {
        if let Some(matcher) = &def.matcher {
            matcher
                .validate()
                .map_err(|e| format!("invalid matcher in hook '{}': {e}", def.name))?;
        }
    }

    debug!(
        path = %path.display(),
        count = definitions.len(),
        "Loaded hooks from TOML"
    );

    Ok(definitions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_hooks_from_toml() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("hooks.toml");
        std::fs::write(
            &file,
            r#"
[[hooks]]
name = "lint-check"
event = "pre_tool_use"
timeout_secs = 10

[hooks.matcher]
type = "exact"
value = "bash"

[hooks.handler]
type = "command"
command = "lint"
args = ["--check"]

[[hooks]]
name = "notify-session"
event = "session_start"

[hooks.handler]
type = "prompt"
template = "Session started: $ARGUMENTS"
"#,
        )
        .expect("write");

        let hooks = load_hooks_from_toml(&file).expect("load");
        assert_eq!(hooks.len(), 2);

        assert_eq!(hooks[0].name, "lint-check");
        assert_eq!(hooks[0].event_type, HookEventType::PreToolUse);
        assert_eq!(hooks[0].timeout_secs, 10);
        assert!(hooks[0].matcher.is_some());
        assert!(hooks[0].enabled);

        assert_eq!(hooks[1].name, "notify-session");
        assert_eq!(hooks[1].event_type, HookEventType::SessionStart);
        assert_eq!(hooks[1].timeout_secs, 30); // default
        assert!(hooks[1].matcher.is_none());
    }

    #[test]
    fn test_load_empty_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("empty.toml");
        std::fs::write(&file, "").expect("write");

        let hooks = load_hooks_from_toml(&file).expect("load");
        assert!(hooks.is_empty());
    }

    #[test]
    fn test_load_nonexistent_file() {
        let result = load_hooks_from_toml(Path::new("/nonexistent/hooks.toml"));
        assert!(result.is_err());
    }

    #[test]
    fn test_load_invalid_toml() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("bad.toml");
        std::fs::write(&file, "this is not valid toml {{{").expect("write");

        let result = load_hooks_from_toml(&file);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_invalid_regex_matcher() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("bad_regex.toml");
        std::fs::write(
            &file,
            r#"
[[hooks]]
name = "bad-regex"
event = "pre_tool_use"

[hooks.matcher]
type = "regex"
pattern = "[invalid"

[hooks.handler]
type = "command"
command = "echo"
"#,
        )
        .expect("write");

        let result = load_hooks_from_toml(&file);
        assert!(result.is_err());
        assert!(result.err().expect("error").contains("invalid matcher"));
    }

    #[test]
    fn test_load_all_handler_types() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("all.toml");
        std::fs::write(
            &file,
            r#"
[[hooks]]
name = "cmd"
event = "pre_tool_use"
[hooks.handler]
type = "command"
command = "echo"

[[hooks]]
name = "prompt"
event = "session_start"
[hooks.handler]
type = "prompt"
template = "hello"

[[hooks]]
name = "agent"
event = "stop"
[hooks.handler]
type = "agent"

[[hooks]]
name = "webhook"
event = "session_end"
[hooks.handler]
type = "webhook"
url = "https://example.com"
"#,
        )
        .expect("write");

        let hooks = load_hooks_from_toml(&file).expect("load");
        assert_eq!(hooks.len(), 4);
        assert!(matches!(hooks[0].handler, HookHandler::Command { .. }));
        assert!(matches!(hooks[1].handler, HookHandler::Prompt { .. }));
        assert!(matches!(hooks[2].handler, HookHandler::Agent { .. }));
        assert!(matches!(hooks[3].handler, HookHandler::Webhook { .. }));
    }

    #[test]
    fn test_load_or_matcher() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("or.toml");
        std::fs::write(
            &file,
            r#"
[[hooks]]
name = "multi-tool"
event = "pre_tool_use"

[hooks.matcher]
type = "or"

[[hooks.matcher.matchers]]
type = "exact"
value = "bash"

[[hooks.matcher.matchers]]
type = "wildcard"
pattern = "read_*"

[hooks.handler]
type = "command"
command = "check"
"#,
        )
        .expect("write");

        let hooks = load_hooks_from_toml(&file).expect("load");
        assert_eq!(hooks.len(), 1);
        let matcher = hooks[0].matcher.as_ref().expect("matcher");
        assert!(matcher.matches("bash"));
        assert!(matcher.matches("read_file"));
        assert!(!matcher.matches("write_file"));
    }
}
