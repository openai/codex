//! Hook execution context.
//!
//! Provides all information available to a hook at execution time.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::event::HookEventType;

/// Context passed to hooks during execution.
///
/// Contains information about the event that triggered the hook and the
/// current session environment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookContext {
    /// The event type that triggered this hook.
    pub event_type: HookEventType,

    /// The tool name (if the event is tool-related).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,

    /// The tool input JSON (if the event is tool-related).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_input: Option<Value>,

    /// The current session identifier.
    pub session_id: String,

    /// The working directory for the session.
    pub working_dir: PathBuf,
}

impl HookContext {
    /// Creates a new `HookContext` with the required fields.
    pub fn new(event_type: HookEventType, session_id: String, working_dir: PathBuf) -> Self {
        Self {
            event_type,
            tool_name: None,
            tool_input: None,
            session_id,
            working_dir,
        }
    }

    /// Sets the tool name and returns `self` for chaining.
    pub fn with_tool_name(mut self, name: impl Into<String>) -> Self {
        self.tool_name = Some(name.into());
        self
    }

    /// Sets the tool input and returns `self` for chaining.
    pub fn with_tool_input(mut self, input: Value) -> Self {
        self.tool_input = Some(input);
        self
    }

    /// Sets both tool name and input, returning `self` for chaining.
    pub fn with_tool(self, name: impl Into<String>, input: Value) -> Self {
        self.with_tool_name(name).with_tool_input(input)
    }

    /// Sets the session ID and returns `self` for chaining.
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = session_id.into();
        self
    }

    /// Sets the working directory and returns `self` for chaining.
    pub fn with_working_dir(mut self, working_dir: PathBuf) -> Self {
        self.working_dir = working_dir;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let ctx = HookContext::new(
            HookEventType::PreToolUse,
            "sess-1".to_string(),
            PathBuf::from("/tmp"),
        );
        assert_eq!(ctx.event_type, HookEventType::PreToolUse);
        assert_eq!(ctx.session_id, "sess-1");
        assert_eq!(ctx.working_dir, PathBuf::from("/tmp"));
        assert!(ctx.tool_name.is_none());
        assert!(ctx.tool_input.is_none());
    }

    #[test]
    fn test_builder_pattern() {
        let ctx = HookContext::new(
            HookEventType::PostToolUse,
            "sess-2".to_string(),
            PathBuf::from("/home"),
        )
        .with_tool("read_file", serde_json::json!({"path": "/etc/hosts"}));

        assert_eq!(ctx.tool_name.as_deref(), Some("read_file"));
        assert!(ctx.tool_input.is_some());
    }

    #[test]
    fn test_with_session_id() {
        let ctx = HookContext::new(
            HookEventType::SessionStart,
            "old".to_string(),
            PathBuf::from("/tmp"),
        )
        .with_session_id("new-session");

        assert_eq!(ctx.session_id, "new-session");
    }

    #[test]
    fn test_serde_roundtrip() {
        let ctx = HookContext::new(
            HookEventType::PreToolUse,
            "sess-1".to_string(),
            PathBuf::from("/tmp"),
        )
        .with_tool_name("bash");

        let json = serde_json::to_string(&ctx).expect("serialize");
        let parsed: HookContext = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.event_type, ctx.event_type);
        assert_eq!(parsed.tool_name, ctx.tool_name);
        assert_eq!(parsed.session_id, ctx.session_id);
    }
}
