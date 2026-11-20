//! Extracts confirmed User Messaging text after input-rewriting hooks.
//! Tracks nested sends through cancellation and captures evidence before post-tool hooks.

use crate::function_tool::FunctionCallError;
use crate::guardian::GUARDIAN_MAX_ROOT_MESSAGE_TOKENS;
use crate::guardian::guardian_truncate_text;
use crate::session::session::Session;
use crate::tools::context::ToolCallSource;
use crate::tools::context::ToolCallState;
use crate::tools::registry::AnyToolResult;
use codex_guardian_context::ContextBudgeting;
use codex_history::RetainedUserMessage;
use codex_mcp::PreparedMcpCall;
use codex_protocol::mcp::CallToolResult;
use codex_tools::ToolName;
use serde_json::Value;
use std::sync::Arc;
use tokio_util::task::task_tracker::TaskTrackerToken;

pub(super) fn admit_code_mode_send(
    session: &Session,
    source: &ToolCallSource,
    tool_name: &ToolName,
) -> Result<Option<TaskTrackerToken>, FunctionCallError> {
    // All User Messaging aliases end in `send_message`; confirmation checks the exact identity.
    if !matches!(source, ToolCallSource::CodeMode { .. })
        || !tool_name.name.ends_with("send_message")
    {
        return Ok(None);
    }
    session.track_code_mode_message().map(Some).ok_or_else(|| {
        FunctionCallError::RespondToModel("code mode nested tool call cancelled".to_owned())
    })
}

/// Captures confirmed delivery before any further await, including post-tool hooks.
pub(super) fn capture_delivery(
    result: &AnyToolResult,
    call_state: Option<&ToolCallState>,
    budgeting: ContextBudgeting,
) {
    if let Some(call_state) = call_state
        && let Some(delivery) = result.delivered_assistant_message(budgeting)
    {
        let _ = call_state.delivered_assistant_message.set(delivery);
    }
}

/// Starts persistence at the successful MCP response boundary, before result callbacks.
pub(crate) fn record_confirmed_code_mode_send(
    session: &Arc<Session>,
    turn_id: &str,
    call_id: &str,
    source: &ToolCallSource,
    prepared_call: &PreparedMcpCall,
    tool_input: &Value,
    result: &CallToolResult,
) {
    if !matches!(source, ToolCallSource::CodeMode { .. })
        || result.is_error == Some(true)
        || !prepared_call.is_host_owned_apps()
        || !matches!(
            prepared_call.tool_info().tool.name.as_ref(),
            "user_message_send_message"
                | "user_messaging_send_message"
                | "user_message.send_message"
                | "user_messaging.send_message"
        )
    {
        return;
    }
    let Some(text) = tool_input.get("text").and_then(Value::as_str) else {
        return;
    };
    if text.trim().is_empty() {
        return;
    }
    let message = RetainedUserMessage {
        origin: codex_history::UserInputOrigin::User,
        turn_id: turn_id.to_owned(),
        message_id: Some(call_id.to_owned()),
        text: text.to_owned(),
        complete: true,
        phase: None,
    };
    let _ = session.record_delivered_assistant_message(message);
}

impl AnyToolResult {
    fn delivered_assistant_message(&self, budgeting: ContextBudgeting) -> Option<String> {
        if !self.result.success_for_logging() {
            return None;
        }
        let payload = self.post_tool_use_payload.as_ref()?;
        // MCP hook names normalize prefixed/unprefixed and flat/namespaced calls.
        // Both connector spellings also have a form for catalogs without connector metadata.
        if !matches!(
            payload.tool_name.name(),
            "mcp__codex_apps__user_messaging__send_message"
                | "mcp__codex_apps__user_messaging_send_message"
                | "mcp__codex_apps__user_message__send_message"
                | "mcp__codex_apps__user_message_send_message"
        ) {
            return None;
        }
        let text = payload.tool_input.get("text")?.as_str()?;
        if text.trim().is_empty() {
            return None;
        }
        Some(match budgeting {
            ContextBudgeting::Fixed => {
                guardian_truncate_text(text, GUARDIAN_MAX_ROOT_MESSAGE_TOKENS).0
            }
            // History can account for the complete record once it has the call's IDs.
            ContextBudgeting::Elastic => text.to_owned(),
        })
    }
}

#[cfg(test)]
#[path = "user_messaging_tests.rs"]
mod tests;
