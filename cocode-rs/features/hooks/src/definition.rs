//! Hook definition types.
//!
//! A `HookDefinition` describes a single hook: when it fires (event type),
//! what it matches against (optional matcher), and what it does (handler).

use serde::{Deserialize, Serialize};

use crate::event::HookEventType;
use crate::matcher::HookMatcher;

/// Defines a single hook.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookDefinition {
    /// The name of this hook (for logging and identification).
    pub name: String,

    /// The event type that triggers this hook.
    pub event_type: HookEventType,

    /// Optional matcher to filter which invocations trigger this hook.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matcher: Option<HookMatcher>,

    /// The handler to execute when this hook fires.
    pub handler: HookHandler,

    /// Whether this hook is enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Timeout in seconds for hook execution.
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: i32,
}

fn default_enabled() -> bool {
    true
}

fn default_timeout_secs() -> i32 {
    30
}

/// The action performed by a hook.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HookHandler {
    /// Run an external command.
    Command {
        /// The command to execute.
        command: String,
        /// Arguments for the command.
        #[serde(default)]
        args: Vec<String>,
    },

    /// Inject a prompt template.
    Prompt {
        /// Template string. `$ARGUMENTS` is replaced with the JSON context.
        template: String,
    },

    /// Delegate to a sub-agent.
    Agent {
        /// Maximum number of turns the agent can run.
        #[serde(default = "default_max_turns")]
        max_turns: i32,
    },

    /// Send an HTTP webhook.
    Webhook {
        /// The URL to call.
        url: String,
    },

    /// An inline function handler (not serializable).
    #[serde(skip)]
    Inline,
}

fn default_max_turns() -> i32 {
    5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_definition_defaults() {
        let json = r#"{
            "name": "test-hook",
            "event_type": "pre_tool_use",
            "handler": { "type": "command", "command": "echo", "args": ["hello"] }
        }"#;
        let def: HookDefinition = serde_json::from_str(json).expect("parse");
        assert_eq!(def.name, "test-hook");
        assert!(def.enabled);
        assert_eq!(def.timeout_secs, 30);
        assert!(def.matcher.is_none());
    }

    #[test]
    fn test_handler_command_serde() {
        let handler = HookHandler::Command {
            command: "lint".to_string(),
            args: vec!["--fix".to_string()],
        };
        let json = serde_json::to_string(&handler).expect("serialize");
        assert!(json.contains("\"type\":\"command\""));

        let parsed: HookHandler = serde_json::from_str(&json).expect("deserialize");
        if let HookHandler::Command { command, args } = parsed {
            assert_eq!(command, "lint");
            assert_eq!(args, vec!["--fix"]);
        } else {
            panic!("Expected Command handler");
        }
    }

    #[test]
    fn test_handler_prompt_serde() {
        let handler = HookHandler::Prompt {
            template: "Review the changes: $ARGUMENTS".to_string(),
        };
        let json = serde_json::to_string(&handler).expect("serialize");
        let parsed: HookHandler = serde_json::from_str(&json).expect("deserialize");
        if let HookHandler::Prompt { template } = parsed {
            assert!(template.contains("$ARGUMENTS"));
        } else {
            panic!("Expected Prompt handler");
        }
    }

    #[test]
    fn test_handler_agent_default_turns() {
        let json = r#"{"type": "agent"}"#;
        let handler: HookHandler = serde_json::from_str(json).expect("deserialize");
        if let HookHandler::Agent { max_turns } = handler {
            assert_eq!(max_turns, 5);
        } else {
            panic!("Expected Agent handler");
        }
    }

    #[test]
    fn test_handler_webhook_serde() {
        let handler = HookHandler::Webhook {
            url: "https://example.com/hook".to_string(),
        };
        let json = serde_json::to_string(&handler).expect("serialize");
        let parsed: HookHandler = serde_json::from_str(&json).expect("deserialize");
        if let HookHandler::Webhook { url } = parsed {
            assert_eq!(url, "https://example.com/hook");
        } else {
            panic!("Expected Webhook handler");
        }
    }
}
