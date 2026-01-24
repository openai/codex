//! Core hook types aligned with Claude Code.

use std::fmt::Debug;
use std::sync::Arc;

use futures::future::BoxFuture;
use serde::Deserialize;
use serde::Serialize;
use tokio_util::sync::CancellationToken;

use crate::error::HookError;
use crate::input::HookInput;
use crate::output::HookOutput;

/// Hook event types aligned with Claude Code (12 events).
///
/// Each event type has an optional matcher field that determines which hooks
/// execute for a given event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum HookEventType {
    /// Before tool execution. Matcher: tool_name.
    PreToolUse,
    /// After successful tool execution. Matcher: tool_name.
    PostToolUse,
    /// After failed tool execution. Matcher: tool_name.
    PostToolUseFailure,
    /// Session begins. Matcher: source.
    SessionStart,
    /// Session ends. Matcher: reason.
    SessionEnd,
    /// User interruption (Ctrl+C). No matcher - all hooks run.
    Stop,
    /// Subagent spawns. Matcher: agent_type.
    SubagentStart,
    /// Subagent ends. No matcher - all hooks run.
    SubagentStop,
    /// User sends message. No matcher - all hooks run.
    UserPromptSubmit,
    /// System notification. Matcher: notification_type.
    Notification,
    /// Before context compaction. Matcher: trigger.
    PreCompact,
    /// Before permission prompt. Matcher: tool_name.
    PermissionRequest,
}

impl HookEventType {
    /// Get the field name used for matching.
    ///
    /// Returns `None` for events where all hooks run regardless of matcher.
    pub fn matcher_field(&self) -> Option<&'static str> {
        match self {
            Self::PreToolUse
            | Self::PostToolUse
            | Self::PostToolUseFailure
            | Self::PermissionRequest => Some("tool_name"),
            Self::SessionStart => Some("source"),
            Self::PreCompact => Some("trigger"),
            Self::Notification => Some("notification_type"),
            Self::SessionEnd => Some("reason"),
            Self::SubagentStart => Some("agent_type"),
            // No matcher - all hooks run for these events
            Self::Stop | Self::SubagentStop | Self::UserPromptSubmit => None,
        }
    }

    /// Get the string name of this event type.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PreToolUse => "PreToolUse",
            Self::PostToolUse => "PostToolUse",
            Self::PostToolUseFailure => "PostToolUseFailure",
            Self::SessionStart => "SessionStart",
            Self::SessionEnd => "SessionEnd",
            Self::Stop => "Stop",
            Self::SubagentStart => "SubagentStart",
            Self::SubagentStop => "SubagentStop",
            Self::UserPromptSubmit => "UserPromptSubmit",
            Self::Notification => "Notification",
            Self::PreCompact => "PreCompact",
            Self::PermissionRequest => "PermissionRequest",
        }
    }
}

impl std::fmt::Display for HookEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Hook type configuration (MVP: Command + Callback only).
#[derive(Clone)]
pub enum HookType {
    /// Shell command execution.
    ///
    /// - Input is passed as JSON via stdin
    /// - Exit code 0 = success, 2 = blocking, 1/3+ = non-blocking error
    /// - Environment: CLAUDE_PROJECT_DIR, CLAUDE_ENV_FILE (SessionStart only)
    Command {
        /// The shell command to execute.
        command: String,
        /// Timeout in seconds (default: 60).
        timeout_secs: u32,
        /// Optional status message for progress display.
        status_message: Option<String>,
    },
    /// Native Rust callback (trait-based).
    ///
    /// Timeout is in milliseconds (different from command hooks).
    Callback {
        /// The callback implementation.
        callback: Arc<dyn HookCallback>,
        /// Timeout in milliseconds (default: 60000).
        timeout_ms: Option<u64>,
    },
    // DEFERRED: Prompt (LLM-based), Agent (agentic verifier), Function (message analysis)
}

impl Debug for HookType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Command {
                command,
                timeout_secs,
                status_message,
            } => f
                .debug_struct("Command")
                .field("command", command)
                .field("timeout_secs", timeout_secs)
                .field("status_message", status_message)
                .finish(),
            Self::Callback { timeout_ms, .. } => f
                .debug_struct("Callback")
                .field("timeout_ms", timeout_ms)
                .finish_non_exhaustive(),
        }
    }
}

/// Trait for native hook callbacks.
///
/// Aligned with Claude Code callback signature:
/// ```typescript
/// callback: (hookInput, toolUseID, signal, hookIndex) => Promise<HookOutput>
/// ```
///
/// # Example
///
/// ```rust,ignore
/// use codex_hooks::{HookCallback, HookInput, HookOutput, HookError};
/// use futures::future::BoxFuture;
/// use tokio_util::sync::CancellationToken;
///
/// struct MyCallback;
///
/// impl HookCallback for MyCallback {
///     fn execute(
///         &self,
///         input: HookInput,
///         tool_use_id: Option<String>,
///         cancel: CancellationToken,
///         hook_index: i32,
///     ) -> BoxFuture<'static, Result<HookOutput, HookError>> {
///         Box::pin(async move {
///             println!("Hook triggered: {:?}", input.hook_event_name);
///             Ok(HookOutput::default())
///         })
///     }
/// }
/// ```
pub trait HookCallback: Send + Sync + Debug {
    /// Execute the callback with the given hook input.
    ///
    /// # Arguments
    ///
    /// * `input` - The complete hook input including event data and context
    /// * `tool_use_id` - Optional tool use ID for tool-related events
    /// * `cancel` - Cancellation token for timeout/abort handling
    /// * `hook_index` - Index of this hook in the execution order
    fn execute(
        &self,
        input: HookInput,
        tool_use_id: Option<String>,
        cancel: CancellationToken,
        hook_index: i32,
    ) -> BoxFuture<'static, Result<HookOutput, HookError>>;

    /// Get the deduplication key for this callback.
    ///
    /// Returns `None` for callbacks, as Claude Code never deduplicates callbacks.
    /// Command hooks use the command string for deduplication.
    fn dedupe_key(&self) -> Option<String> {
        None
    }
}

/// Matcher configuration for hook filtering.
#[derive(Debug, Clone)]
pub struct HookMatcher {
    /// Pattern to match against the event's matcher field.
    ///
    /// - Empty or "*" → match all
    /// - Exact value → exact match
    /// - "A|B|C" → match any of A, B, or C
    /// - Otherwise → treated as regex
    pub matcher: String,

    /// Hooks to execute when the pattern matches.
    pub hooks: Vec<HookConfig>,
}

/// Complete hook configuration for one hook instance.
#[derive(Clone)]
pub struct HookConfig {
    /// The hook type (Command or Callback).
    pub hook_type: HookType,

    /// Optional success callback, called after successful hook execution.
    /// Aligned with Claude Code's `onHookSuccess` callback.
    pub on_success: Option<Arc<dyn HookSuccessCallback>>,
}

impl Debug for HookConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HookConfig")
            .field("hook_type", &self.hook_type)
            .field("on_success", &self.on_success.is_some())
            .finish()
    }
}

/// Called after successful hook execution.
///
/// Aligned with Claude Code's `onHookSuccess` callback pattern.
pub trait HookSuccessCallback: Send + Sync {
    /// Called when a hook executes successfully.
    ///
    /// # Arguments
    ///
    /// * `hook` - The hook configuration that executed
    /// * `result` - The result of the hook execution
    fn on_success(&self, hook: &HookConfig, result: &crate::output::HookResult);
}

impl Debug for dyn HookSuccessCallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HookSuccessCallback")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_event_type_matcher_fields() {
        assert_eq!(HookEventType::PreToolUse.matcher_field(), Some("tool_name"));
        assert_eq!(
            HookEventType::PostToolUse.matcher_field(),
            Some("tool_name")
        );
        assert_eq!(HookEventType::SessionStart.matcher_field(), Some("source"));
        assert_eq!(HookEventType::PreCompact.matcher_field(), Some("trigger"));
        assert_eq!(HookEventType::Stop.matcher_field(), None);
        assert_eq!(HookEventType::UserPromptSubmit.matcher_field(), None);
    }

    #[test]
    fn test_hook_event_type_display() {
        assert_eq!(HookEventType::PreToolUse.to_string(), "PreToolUse");
        assert_eq!(HookEventType::SessionStart.to_string(), "SessionStart");
    }

    #[test]
    fn test_hook_type_debug() {
        let cmd = HookType::Command {
            command: "echo test".to_string(),
            timeout_secs: 30,
            status_message: Some("Testing".to_string()),
        };
        let debug_str = format!("{cmd:?}");
        assert!(debug_str.contains("Command"));
        assert!(debug_str.contains("echo test"));
    }
}
