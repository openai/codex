use std::time::Duration;
use std::time::Instant;

use tracing::error;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
use crate::protocol::EventMsg;
use crate::protocol::McpInvocation;
use crate::protocol::McpToolCallBeginEvent;
use crate::protocol::McpToolCallEndEvent;
use codex_protocol::mcp::CallToolResult;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::request_user_input::RequestUserInputArgs;
use codex_protocol::request_user_input::RequestUserInputQuestion;
use codex_protocol::request_user_input::RequestUserInputQuestionOption;
use codex_protocol::request_user_input::RequestUserInputResponse;
use rmcp::model::ToolAnnotations;
use serde::Serialize;
use std::sync::Arc;

/// Handles the specified tool call dispatches the appropriate
/// `McpToolCallBegin` and `McpToolCallEnd` events to the `Session`.
pub(crate) async fn handle_mcp_tool_call(
    sess: Arc<Session>,
    turn_context: &TurnContext,
    call_id: String,
    server: String,
    tool_name: String,
    arguments: String,
) -> ResponseInputItem {
    // Parse the `arguments` as JSON. An empty string is OK, but invalid JSON
    // is not.
    let arguments_value = if arguments.trim().is_empty() {
        None
    } else {
        match serde_json::from_str::<serde_json::Value>(&arguments) {
            Ok(value) => Some(value),
            Err(e) => {
                error!("failed to parse tool call arguments: {e}");
                return ResponseInputItem::FunctionCallOutput {
                    call_id: call_id.clone(),
                    output: FunctionCallOutputPayload {
                        body: FunctionCallOutputBody::Text(format!("err: {e}")),
                        success: Some(false),
                    },
                };
            }
        }
    };

    let invocation = McpInvocation {
        server: server.clone(),
        tool: tool_name.clone(),
        arguments: arguments_value.clone(),
    };

    if let Some(decision) =
        maybe_request_mcp_tool_approval(sess.as_ref(), turn_context, &call_id, &server, &tool_name)
            .await
    {
        let result = match decision {
            McpToolApprovalDecision::Accept | McpToolApprovalDecision::AcceptAndRemember => {
                let tool_call_begin_event = EventMsg::McpToolCallBegin(McpToolCallBeginEvent {
                    call_id: call_id.clone(),
                    invocation: invocation.clone(),
                });
                notify_mcp_tool_call_event(sess.as_ref(), turn_context, tool_call_begin_event)
                    .await;

                let start = Instant::now();
                let result: Result<CallToolResult, String> = sess
                    .call_tool(&server, &tool_name, arguments_value.clone())
                    .await
                    .map_err(|e| format!("tool call error: {e:?}"));
                if let Err(e) = &result {
                    tracing::warn!("MCP tool call error: {e:?}");
                }
                let tool_call_end_event = EventMsg::McpToolCallEnd(McpToolCallEndEvent {
                    call_id: call_id.clone(),
                    invocation,
                    duration: start.elapsed(),
                    result: result.clone(),
                });
                notify_mcp_tool_call_event(
                    sess.as_ref(),
                    turn_context,
                    tool_call_end_event.clone(),
                )
                .await;
                result
            }
            McpToolApprovalDecision::Decline => {
                let message = "user rejected MCP tool call".to_string();
                notify_mcp_tool_call_skip(
                    sess.as_ref(),
                    turn_context,
                    &call_id,
                    invocation,
                    message,
                )
                .await
            }
            McpToolApprovalDecision::Cancel => {
                let message = "user cancelled MCP tool call".to_string();
                notify_mcp_tool_call_skip(
                    sess.as_ref(),
                    turn_context,
                    &call_id,
                    invocation,
                    message,
                )
                .await
            }
        };

        let status = if result.is_ok() { "ok" } else { "error" };
        turn_context
            .otel_manager
            .counter("codex.mcp.call", 1, &[("status", status)]);

        return ResponseInputItem::McpToolCallOutput { call_id, result };
    }

    let tool_call_begin_event = EventMsg::McpToolCallBegin(McpToolCallBeginEvent {
        call_id: call_id.clone(),
        invocation: invocation.clone(),
    });
    notify_mcp_tool_call_event(sess.as_ref(), turn_context, tool_call_begin_event).await;

    let start = Instant::now();
    // Perform the tool call.
    let result: Result<CallToolResult, String> = sess
        .call_tool(&server, &tool_name, arguments_value.clone())
        .await
        .map_err(|e| format!("tool call error: {e:?}"));
    if let Err(e) = &result {
        tracing::warn!("MCP tool call error: {e:?}");
    }
    let tool_call_end_event = EventMsg::McpToolCallEnd(McpToolCallEndEvent {
        call_id: call_id.clone(),
        invocation,
        duration: start.elapsed(),
        result: result.clone(),
    });

    notify_mcp_tool_call_event(sess.as_ref(), turn_context, tool_call_end_event.clone()).await;

    let status = if result.is_ok() { "ok" } else { "error" };
    turn_context
        .otel_manager
        .counter("codex.mcp.call", 1, &[("status", status)]);

    ResponseInputItem::McpToolCallOutput { call_id, result }
}

async fn notify_mcp_tool_call_event(sess: &Session, turn_context: &TurnContext, event: EventMsg) {
    sess.send_event(turn_context, event).await;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum McpToolApprovalDecision {
    Accept,
    AcceptAndRemember,
    Decline,
    Cancel,
}

struct McpToolApprovalMetadata {
    annotations: ToolAnnotations,
    connector_id: Option<String>,
    connector_name: Option<String>,
    tool_title: Option<String>,
}

const MCP_TOOL_APPROVAL_QUESTION_ID_PREFIX: &str = "mcp_tool_call_approval";
const MCP_TOOL_APPROVAL_ACCEPT: &str = "Approve Once";
const MCP_TOOL_APPROVAL_ACCEPT_AND_REMEMBER: &str = "Approve this Session";
const MCP_TOOL_APPROVAL_DECLINE: &str = "Deny";
const MCP_TOOL_APPROVAL_CANCEL: &str = "Cancel";

#[derive(Debug, Serialize)]
struct McpToolApprovalKey {
    server: String,
    connector_id: String,
    tool_name: String,
}

async fn maybe_request_mcp_tool_approval(
    sess: &Session,
    turn_context: &TurnContext,
    call_id: &str,
    server: &str,
    tool_name: &str,
) -> Option<McpToolApprovalDecision> {
    if is_full_access_mode(turn_context) {
        return None;
    }
    if server != CODEX_APPS_MCP_SERVER_NAME {
        return None;
    }

    let metadata = lookup_mcp_tool_metadata(sess, server, tool_name).await?;
    if !requires_mcp_tool_approval(&metadata.annotations) {
        return None;
    }
    let approval_key = metadata
        .connector_id
        .as_deref()
        .map(|connector_id| McpToolApprovalKey {
            server: server.to_string(),
            connector_id: connector_id.to_string(),
            tool_name: tool_name.to_string(),
        });
    if let Some(key) = approval_key.as_ref()
        && mcp_tool_approval_is_remembered(sess, key).await
    {
        return Some(McpToolApprovalDecision::Accept);
    }

    let question_id = format!("{MCP_TOOL_APPROVAL_QUESTION_ID_PREFIX}_{call_id}");
    let question = build_mcp_tool_approval_question(
        question_id.clone(),
        tool_name,
        metadata.tool_title.as_deref(),
        metadata.connector_name.as_deref(),
        &metadata.annotations,
        approval_key.is_some(),
    );
    let args = RequestUserInputArgs {
        questions: vec![question],
    };
    let response = sess
        .request_user_input(turn_context, call_id.to_string(), args)
        .await;
    let decision = parse_mcp_tool_approval_response(response, &question_id);
    if matches!(decision, McpToolApprovalDecision::AcceptAndRemember)
        && let Some(key) = approval_key
    {
        remember_mcp_tool_approval(sess, key).await;
    }
    Some(decision)
}

fn is_full_access_mode(turn_context: &TurnContext) -> bool {
    matches!(turn_context.approval_policy, AskForApproval::Never)
        && matches!(
            turn_context.sandbox_policy,
            SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. }
        )
}

async fn lookup_mcp_tool_metadata(
    sess: &Session,
    server: &str,
    tool_name: &str,
) -> Option<McpToolApprovalMetadata> {
    let tools = sess
        .services
        .mcp_connection_manager
        .read()
        .await
        .list_all_tools()
        .await;

    tools.into_values().find_map(|tool_info| {
        if tool_info.server_name == server && tool_info.tool_name == tool_name {
            tool_info
                .tool
                .annotations
                .map(|annotations| McpToolApprovalMetadata {
                    annotations,
                    connector_id: tool_info.connector_id,
                    connector_name: tool_info.connector_name,
                    tool_title: tool_info.tool.title,
                })
        } else {
            None
        }
    })
}

fn build_mcp_tool_approval_question(
    question_id: String,
    tool_name: &str,
    tool_title: Option<&str>,
    connector_name: Option<&str>,
    annotations: &ToolAnnotations,
    allow_remember_option: bool,
) -> RequestUserInputQuestion {
    let destructive = annotations.destructive_hint == Some(true);
    let open_world = annotations.open_world_hint == Some(true);
    let reason = match (destructive, open_world) {
        (true, true) => "may modify data and access external systems",
        (true, false) => "may modify or delete data",
        (false, true) => "may access external systems",
        (false, false) => "may have side effects",
    };

    let tool_label = tool_title.unwrap_or(tool_name);
    let app_label = connector_name
        .map(|name| format!("The {name} app"))
        .unwrap_or_else(|| "This app".to_string());
    let question = format!(
        "{app_label} wants to run the tool \"{tool_label}\", which {reason}. Allow this action?"
    );

    let mut options = vec![RequestUserInputQuestionOption {
        label: MCP_TOOL_APPROVAL_ACCEPT.to_string(),
        description: "Run the tool and continue.".to_string(),
    }];
    if allow_remember_option {
        options.push(RequestUserInputQuestionOption {
            label: MCP_TOOL_APPROVAL_ACCEPT_AND_REMEMBER.to_string(),
            description: "Run the tool and remember this choice for this session.".to_string(),
        });
    }
    options.extend([
        RequestUserInputQuestionOption {
            label: MCP_TOOL_APPROVAL_DECLINE.to_string(),
            description: "Decline this tool call and continue.".to_string(),
        },
        RequestUserInputQuestionOption {
            label: MCP_TOOL_APPROVAL_CANCEL.to_string(),
            description: "Cancel this tool call".to_string(),
        },
    ]);

    RequestUserInputQuestion {
        id: question_id,
        header: "Approve app tool call?".to_string(),
        question,
        is_other: false,
        is_secret: false,
        options: Some(options),
    }
}

fn parse_mcp_tool_approval_response(
    response: Option<RequestUserInputResponse>,
    question_id: &str,
) -> McpToolApprovalDecision {
    let Some(response) = response else {
        return McpToolApprovalDecision::Cancel;
    };
    let answers = response
        .answers
        .get(question_id)
        .map(|answer| answer.answers.as_slice());
    let Some(answers) = answers else {
        return McpToolApprovalDecision::Cancel;
    };
    if answers
        .iter()
        .any(|answer| answer == MCP_TOOL_APPROVAL_ACCEPT_AND_REMEMBER)
    {
        McpToolApprovalDecision::AcceptAndRemember
    } else if answers
        .iter()
        .any(|answer| answer == MCP_TOOL_APPROVAL_ACCEPT)
    {
        McpToolApprovalDecision::Accept
    } else if answers
        .iter()
        .any(|answer| answer == MCP_TOOL_APPROVAL_CANCEL)
    {
        McpToolApprovalDecision::Cancel
    } else {
        McpToolApprovalDecision::Decline
    }
}

async fn mcp_tool_approval_is_remembered(sess: &Session, key: &McpToolApprovalKey) -> bool {
    let store = sess.services.tool_approvals.lock().await;
    matches!(store.get(key), Some(ReviewDecision::ApprovedForSession))
}

async fn remember_mcp_tool_approval(sess: &Session, key: McpToolApprovalKey) {
    let mut store = sess.services.tool_approvals.lock().await;
    store.put(key, ReviewDecision::ApprovedForSession);
}

fn requires_mcp_tool_approval(annotations: &ToolAnnotations) -> bool {
    annotations.read_only_hint == Some(false)
        && (annotations.destructive_hint == Some(true) || annotations.open_world_hint == Some(true))
}

async fn notify_mcp_tool_call_skip(
    sess: &Session,
    turn_context: &TurnContext,
    call_id: &str,
    invocation: McpInvocation,
    message: String,
) -> Result<CallToolResult, String> {
    let tool_call_begin_event = EventMsg::McpToolCallBegin(McpToolCallBeginEvent {
        call_id: call_id.to_string(),
        invocation: invocation.clone(),
    });
    notify_mcp_tool_call_event(sess, turn_context, tool_call_begin_event).await;

    let tool_call_end_event = EventMsg::McpToolCallEnd(McpToolCallEndEvent {
        call_id: call_id.to_string(),
        invocation,
        duration: Duration::ZERO,
        result: Err(message.clone()),
    });
    notify_mcp_tool_call_event(sess, turn_context, tool_call_end_event).await;
    Err(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn annotations(
        read_only: Option<bool>,
        destructive: Option<bool>,
        open_world: Option<bool>,
    ) -> ToolAnnotations {
        ToolAnnotations {
            destructive_hint: destructive,
            idempotent_hint: None,
            open_world_hint: open_world,
            read_only_hint: read_only,
            title: None,
        }
    }

    #[test]
    fn approval_required_when_read_only_false_and_destructive() {
        let annotations = annotations(Some(false), Some(true), None);
        assert_eq!(requires_mcp_tool_approval(&annotations), true);
    }

    #[test]
    fn approval_required_when_read_only_false_and_open_world() {
        let annotations = annotations(Some(false), None, Some(true));
        assert_eq!(requires_mcp_tool_approval(&annotations), true);
    }

    #[test]
    fn approval_not_required_when_read_only_true() {
        let annotations = annotations(Some(true), Some(true), Some(true));
        assert_eq!(requires_mcp_tool_approval(&annotations), false);
    }
}
