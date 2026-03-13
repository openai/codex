use std::collections::BTreeMap;
use std::time::Duration;
use std::time::Instant;

use codex_app_server_protocol::McpElicitationObjectType;
use codex_app_server_protocol::McpElicitationSchema;
use codex_app_server_protocol::McpServerElicitationRequest;
use codex_app_server_protocol::McpServerElicitationRequestParams;
use tracing::error;

use crate::analytics_client::AppInvocation;
use crate::analytics_client::InvocationType;
use crate::analytics_client::build_track_events_context;
use crate::arc_monitor::ArcMonitorOutcome;
use crate::arc_monitor::monitor_action;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::config::edit::ConfigEdit;
use crate::config::edit::ConfigEditsBuilder;
use crate::config::types::AppToolApproval;
use crate::connectors;
use crate::features::Feature;
use crate::guardian::GuardianApprovalRequest;
use crate::guardian::GuardianMcpAnnotations;
use crate::guardian::guardian_approval_request_to_json;
use crate::guardian::review_approval_request;
use crate::guardian::routes_approval_to_guardian;
use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
use crate::mcp_tool_approval_templates::RenderedMcpToolApprovalParam;
use crate::mcp_tool_approval_templates::render_mcp_tool_approval_template;
use crate::protocol::EventMsg;
use crate::protocol::McpInvocation;
use crate::protocol::McpToolCallBeginEvent;
use crate::protocol::McpToolCallEndEvent;
use crate::state_db;
use codex_protocol::mcp::CallToolResult;
use codex_protocol::openai_models::InputModality;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::request_user_input::RequestUserInputAnswer;
use codex_protocol::request_user_input::RequestUserInputArgs;
use codex_protocol::request_user_input::RequestUserInputQuestion;
use codex_protocol::request_user_input::RequestUserInputQuestionOption;
use codex_protocol::request_user_input::RequestUserInputResponse;
use codex_rmcp_client::ElicitationAction;
use codex_rmcp_client::ElicitationResponse;
use rmcp::model::ToolAnnotations;
use serde::Serialize;
use std::path::Path;
use std::sync::Arc;
use toml_edit::value;

/// Handles the specified tool call dispatches the appropriate
/// `McpToolCallBegin` and `McpToolCallEnd` events to the `Session`.
pub(crate) async fn handle_mcp_tool_call(
    sess: Arc<Session>,
    turn_context: &Arc<TurnContext>,
    call_id: String,
    server: String,
    tool_name: String,
    arguments: String,
) -> CallToolResult {
    // Parse the `arguments` as JSON. An empty string is OK, but invalid JSON
    // is not.
    let arguments_value = if arguments.trim().is_empty() {
        None
    } else {
        match serde_json::from_str::<serde_json::Value>(&arguments) {
            Ok(value) => Some(value),
            Err(e) => {
                error!("failed to parse tool call arguments: {e}");
                return CallToolResult::from_error_text(format!("err: {e}"));
            }
        }
    };

    let invocation = McpInvocation {
        server: server.clone(),
        tool: tool_name.clone(),
        arguments: arguments_value.clone(),
    };

    let metadata =
        lookup_mcp_tool_metadata(sess.as_ref(), turn_context.as_ref(), &server, &tool_name).await;
    let app_tool_policy = if server == CODEX_APPS_MCP_SERVER_NAME {
        connectors::app_tool_policy(
            &turn_context.config,
            metadata
                .as_ref()
                .and_then(|metadata| metadata.connector_id.as_deref()),
            &tool_name,
            metadata
                .as_ref()
                .and_then(|metadata| metadata.tool_title.as_deref()),
            metadata
                .as_ref()
                .and_then(|metadata| metadata.annotations.as_ref()),
        )
    } else {
        connectors::AppToolPolicy::default()
    };

    if server == CODEX_APPS_MCP_SERVER_NAME && !app_tool_policy.enabled {
        let result = notify_mcp_tool_call_skip(
            sess.as_ref(),
            turn_context.as_ref(),
            &call_id,
            invocation,
            "MCP tool call blocked by app configuration".to_string(),
            false,
        )
        .await;
        let status = if result.is_ok() { "ok" } else { "error" };
        turn_context
            .session_telemetry
            .counter("codex.mcp.call", 1, &[("status", status)]);
        return CallToolResult::from_result(result);
    }

    let tool_call_begin_event = EventMsg::McpToolCallBegin(McpToolCallBeginEvent {
        call_id: call_id.clone(),
        invocation: invocation.clone(),
    });
    notify_mcp_tool_call_event(sess.as_ref(), turn_context.as_ref(), tool_call_begin_event).await;

    if let Some(decision) = maybe_request_mcp_tool_approval(
        &sess,
        turn_context,
        &call_id,
        &invocation,
        metadata.as_ref(),
        app_tool_policy.approval,
    )
    .await
    {
        let result = match decision {
            McpToolApprovalDecision::Accept
            | McpToolApprovalDecision::AcceptForSession
            | McpToolApprovalDecision::AcceptAndRemember => {
                maybe_mark_thread_memory_mode_polluted(sess.as_ref(), turn_context.as_ref()).await;

                let start = Instant::now();
                let result = sess
                    .call_tool(&server, &tool_name, arguments_value.clone())
                    .await
                    .map_err(|e| format!("tool call error: {e:?}"));
                let result = sanitize_mcp_tool_result_for_model(
                    turn_context
                        .model_info
                        .input_modalities
                        .contains(&InputModality::Image),
                    result,
                );
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
                    turn_context.as_ref(),
                    tool_call_end_event.clone(),
                )
                .await;
                maybe_track_codex_app_used(
                    sess.as_ref(),
                    turn_context.as_ref(),
                    &server,
                    &tool_name,
                )
                .await;
                result
            }
            McpToolApprovalDecision::Decline => {
                let message = "user rejected MCP tool call".to_string();
                notify_mcp_tool_call_skip(
                    sess.as_ref(),
                    turn_context.as_ref(),
                    &call_id,
                    invocation,
                    message,
                    true,
                )
                .await
            }
            McpToolApprovalDecision::Cancel => {
                let message = "user cancelled MCP tool call".to_string();
                notify_mcp_tool_call_skip(
                    sess.as_ref(),
                    turn_context.as_ref(),
                    &call_id,
                    invocation,
                    message,
                    true,
                )
                .await
            }
            McpToolApprovalDecision::BlockedBySafetyMonitor(message) => {
                notify_mcp_tool_call_skip(
                    sess.as_ref(),
                    turn_context.as_ref(),
                    &call_id,
                    invocation,
                    message,
                    true,
                )
                .await
            }
        };

        let status = if result.is_ok() { "ok" } else { "error" };
        turn_context
            .session_telemetry
            .counter("codex.mcp.call", 1, &[("status", status)]);

        return CallToolResult::from_result(result);
    }

    maybe_mark_thread_memory_mode_polluted(sess.as_ref(), turn_context.as_ref()).await;

    let start = Instant::now();
    // Perform the tool call.
    let result = sess
        .call_tool(&server, &tool_name, arguments_value.clone())
        .await
        .map_err(|e| format!("tool call error: {e:?}"));
    let result = sanitize_mcp_tool_result_for_model(
        turn_context
            .model_info
            .input_modalities
            .contains(&InputModality::Image),
        result,
    );
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
        turn_context.as_ref(),
        tool_call_end_event.clone(),
    )
    .await;
    maybe_track_codex_app_used(sess.as_ref(), turn_context.as_ref(), &server, &tool_name).await;

    let status = if result.is_ok() { "ok" } else { "error" };
    turn_context
        .session_telemetry
        .counter("codex.mcp.call", 1, &[("status", status)]);

    CallToolResult::from_result(result)
}

async fn maybe_mark_thread_memory_mode_polluted(sess: &Session, turn_context: &TurnContext) {
    if !turn_context
        .config
        .memories
        .no_memories_if_mcp_or_web_search
    {
        return;
    }
    state_db::mark_thread_memory_mode_polluted(
        sess.services.state_db.as_deref(),
        sess.conversation_id,
        "mcp_tool_call",
    )
    .await;
}

fn sanitize_mcp_tool_result_for_model(
    supports_image_input: bool,
    result: Result<CallToolResult, String>,
) -> Result<CallToolResult, String> {
    if supports_image_input {
        return result;
    }

    result.map(|call_tool_result| CallToolResult {
        content: call_tool_result
            .content
            .iter()
            .map(|block| {
                if let Some(content_type) = block.get("type").and_then(serde_json::Value::as_str)
                    && content_type == "image"
                {
                    return serde_json::json!({
                        "type": "text",
                        "text": "<image content omitted because you do not support image input>",
                    });
                }

                block.clone()
            })
            .collect::<Vec<_>>(),
        structured_content: call_tool_result.structured_content,
        is_error: call_tool_result.is_error,
        meta: call_tool_result.meta,
    })
}

async fn notify_mcp_tool_call_event(sess: &Session, turn_context: &TurnContext, event: EventMsg) {
    sess.send_event(turn_context, event).await;
}

struct McpAppUsageMetadata {
    connector_id: Option<String>,
    app_name: Option<String>,
}

async fn maybe_track_codex_app_used(
    sess: &Session,
    turn_context: &TurnContext,
    server: &str,
    tool_name: &str,
) {
    if server != CODEX_APPS_MCP_SERVER_NAME {
        return;
    }
    let metadata = lookup_mcp_app_usage_metadata(sess, server, tool_name).await;
    let (connector_id, app_name) = metadata
        .map(|metadata| (metadata.connector_id, metadata.app_name))
        .unwrap_or((None, None));
    let invocation_type = if let Some(connector_id) = connector_id.as_deref() {
        let mentioned_connector_ids = sess.get_connector_selection().await;
        if mentioned_connector_ids.contains(connector_id) {
            InvocationType::Explicit
        } else {
            InvocationType::Implicit
        }
    } else {
        InvocationType::Implicit
    };

    let tracking = build_track_events_context(
        turn_context.model_info.slug.clone(),
        sess.conversation_id.to_string(),
        turn_context.sub_id.clone(),
    );
    sess.services.analytics_events_client.track_app_used(
        tracking,
        AppInvocation {
            connector_id,
            app_name,
            invocation_type: Some(invocation_type),
        },
    );
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum McpToolApprovalDecision {
    Accept,
    AcceptForSession,
    AcceptAndRemember,
    Decline,
    Cancel,
    BlockedBySafetyMonitor(String),
}

pub(crate) struct McpToolApprovalMetadata {
    annotations: Option<ToolAnnotations>,
    connector_id: Option<String>,
    connector_name: Option<String>,
    connector_description: Option<String>,
    tool_title: Option<String>,
    tool_description: Option<String>,
}

#[derive(Clone, Copy)]
struct McpToolApprovalPromptOptions {
    allow_session_remember: bool,
    allow_persistent_approval: bool,
}

struct McpToolApprovalElicitationRequest<'a> {
    server: &'a str,
    metadata: Option<&'a McpToolApprovalMetadata>,
    tool_params: Option<&'a serde_json::Value>,
    tool_params_display: Option<&'a [RenderedMcpToolApprovalParam]>,
    question: RequestUserInputQuestion,
    message_override: Option<&'a str>,
    prompt_options: McpToolApprovalPromptOptions,
}

pub(crate) const MCP_TOOL_APPROVAL_QUESTION_ID_PREFIX: &str = "mcp_tool_call_approval";
pub(crate) const MCP_TOOL_APPROVAL_ACCEPT: &str = "Allow";
pub(crate) const MCP_TOOL_APPROVAL_ACCEPT_FOR_SESSION: &str = "Allow for this session";
const MCP_TOOL_APPROVAL_ACCEPT_AND_REMEMBER: &str = "Allow and don't ask me again";
const MCP_TOOL_APPROVAL_CANCEL: &str = "Cancel";
const MCP_TOOL_APPROVAL_KIND_KEY: &str = "codex_approval_kind";
const MCP_TOOL_APPROVAL_KIND_MCP_TOOL_CALL: &str = "mcp_tool_call";
const MCP_TOOL_APPROVAL_PERSIST_KEY: &str = "persist";
const MCP_TOOL_APPROVAL_PERSIST_SESSION: &str = "session";
const MCP_TOOL_APPROVAL_PERSIST_ALWAYS: &str = "always";
const MCP_TOOL_APPROVAL_SOURCE_KEY: &str = "source";
const MCP_TOOL_APPROVAL_SOURCE_CONNECTOR: &str = "connector";
const MCP_TOOL_APPROVAL_CONNECTOR_ID_KEY: &str = "connector_id";
const MCP_TOOL_APPROVAL_CONNECTOR_NAME_KEY: &str = "connector_name";
const MCP_TOOL_APPROVAL_CONNECTOR_DESCRIPTION_KEY: &str = "connector_description";
const MCP_TOOL_APPROVAL_TOOL_TITLE_KEY: &str = "tool_title";
const MCP_TOOL_APPROVAL_TOOL_DESCRIPTION_KEY: &str = "tool_description";
const MCP_TOOL_APPROVAL_TOOL_PARAMS_KEY: &str = "tool_params";
const MCP_TOOL_APPROVAL_TOOL_PARAMS_DISPLAY_KEY: &str = "tool_params_display";

pub(crate) fn is_mcp_tool_approval_question_id(question_id: &str) -> bool {
    question_id
        .strip_prefix(MCP_TOOL_APPROVAL_QUESTION_ID_PREFIX)
        .is_some_and(|suffix| suffix.starts_with('_'))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct McpToolApprovalKey {
    server: String,
    connector_id: Option<String>,
    tool_name: String,
}

fn mcp_tool_approval_prompt_options(
    session_approval_key: Option<&McpToolApprovalKey>,
    persistent_approval_key: Option<&McpToolApprovalKey>,
    tool_call_mcp_elicitation_enabled: bool,
) -> McpToolApprovalPromptOptions {
    McpToolApprovalPromptOptions {
        allow_session_remember: session_approval_key.is_some(),
        allow_persistent_approval: tool_call_mcp_elicitation_enabled
            && persistent_approval_key.is_some(),
    }
}

async fn maybe_request_mcp_tool_approval(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    call_id: &str,
    invocation: &McpInvocation,
    metadata: Option<&McpToolApprovalMetadata>,
    approval_mode: AppToolApproval,
) -> Option<McpToolApprovalDecision> {
    let annotations = metadata.and_then(|metadata| metadata.annotations.as_ref());
    let approval_required = annotations.is_some_and(requires_mcp_tool_approval);
    let mut monitor_reason = None;

    if approval_mode == AppToolApproval::Approve {
        if !approval_required {
            return None;
        }

        match maybe_monitor_auto_approved_mcp_tool_call(sess, turn_context, invocation, metadata)
            .await
        {
            ArcMonitorOutcome::Ok => return None,
            ArcMonitorOutcome::AskUser(reason) => {
                monitor_reason = Some(reason);
            }
            ArcMonitorOutcome::SteerModel(reason) => {
                return Some(McpToolApprovalDecision::BlockedBySafetyMonitor(
                    arc_monitor_interrupt_message(&reason),
                ));
            }
        }
    }

    if approval_mode == AppToolApproval::Auto {
        if is_full_access_mode(turn_context) {
            return None;
        }
        if !approval_required {
            return None;
        }
    }

    let session_approval_key = session_mcp_tool_approval_key(invocation, metadata, approval_mode);
    let persistent_approval_key =
        persistent_mcp_tool_approval_key(invocation, metadata, approval_mode);
    if let Some(key) = session_approval_key.as_ref()
        && mcp_tool_approval_is_remembered(sess, key).await
    {
        return Some(McpToolApprovalDecision::Accept);
    }
    let tool_call_mcp_elicitation_enabled = turn_context
        .config
        .features
        .enabled(Feature::ToolCallMcpElicitation);

    if routes_approval_to_guardian(turn_context) {
        let decision = review_approval_request(
            sess,
            turn_context,
            build_guardian_mcp_tool_review_request(call_id, invocation, metadata),
            monitor_reason.clone(),
        )
        .await;
        let decision = mcp_tool_approval_decision_from_guardian(decision);
        apply_mcp_tool_approval_decision(
            sess,
            turn_context,
            &decision,
            session_approval_key,
            persistent_approval_key,
        )
        .await;
        return Some(decision);
    }

    let prompt_options = mcp_tool_approval_prompt_options(
        session_approval_key.as_ref(),
        persistent_approval_key.as_ref(),
        tool_call_mcp_elicitation_enabled,
    );
    let question_id = format!("{MCP_TOOL_APPROVAL_QUESTION_ID_PREFIX}_{call_id}");
    let rendered_template = render_mcp_tool_approval_template(
        &invocation.server,
        metadata.and_then(|metadata| metadata.connector_id.as_deref()),
        metadata.and_then(|metadata| metadata.connector_name.as_deref()),
        metadata.and_then(|metadata| metadata.tool_title.as_deref()),
        invocation.arguments.as_ref(),
    );
    let tool_params_display = rendered_template
        .as_ref()
        .map(|rendered_template| rendered_template.tool_params_display.clone())
        .or_else(|| build_mcp_tool_approval_display_params(invocation.arguments.as_ref()));
    let mut question = build_mcp_tool_approval_question(
        question_id.clone(),
        &invocation.server,
        &invocation.tool,
        metadata.and_then(|metadata| metadata.connector_name.as_deref()),
        prompt_options,
        rendered_template
            .as_ref()
            .map(|rendered_template| rendered_template.question.as_str()),
    );
    question.question =
        mcp_tool_approval_question_text(question.question, monitor_reason.as_deref());
    if tool_call_mcp_elicitation_enabled {
        let request_id = rmcp::model::RequestId::String(
            format!("{MCP_TOOL_APPROVAL_QUESTION_ID_PREFIX}_{call_id}").into(),
        );
        let params = build_mcp_tool_approval_elicitation_request(
            sess.as_ref(),
            turn_context.as_ref(),
            McpToolApprovalElicitationRequest {
                server: &invocation.server,
                metadata,
                tool_params: rendered_template
                    .as_ref()
                    .and_then(|rendered_template| rendered_template.tool_params.as_ref())
                    .or(invocation.arguments.as_ref()),
                tool_params_display: tool_params_display.as_deref(),
                question,
                message_override: rendered_template.as_ref().and_then(|rendered_template| {
                    monitor_reason
                        .is_none()
                        .then_some(rendered_template.elicitation_message.as_str())
                }),
                prompt_options,
            },
        );
        let decision = parse_mcp_tool_approval_elicitation_response(
            sess.request_mcp_server_elicitation(turn_context.as_ref(), request_id, params)
                .await,
            &question_id,
        );
        let decision = normalize_approval_decision_for_mode(decision, approval_mode);
        apply_mcp_tool_approval_decision(
            sess,
            turn_context,
            &decision,
            session_approval_key,
            persistent_approval_key,
        )
        .await;
        return Some(decision);
    }

    let args = RequestUserInputArgs {
        questions: vec![question],
    };
    let response = sess
        .request_user_input(turn_context.as_ref(), call_id.to_string(), args)
        .await;
    let decision = normalize_approval_decision_for_mode(
        parse_mcp_tool_approval_response(response, &question_id),
        approval_mode,
    );
    apply_mcp_tool_approval_decision(
        sess,
        turn_context,
        &decision,
        session_approval_key,
        persistent_approval_key,
    )
    .await;
    Some(decision)
}

async fn maybe_monitor_auto_approved_mcp_tool_call(
    sess: &Session,
    turn_context: &TurnContext,
    invocation: &McpInvocation,
    metadata: Option<&McpToolApprovalMetadata>,
) -> ArcMonitorOutcome {
    let action = prepare_arc_request_action(invocation, metadata);
    monitor_action(sess, turn_context, action).await
}

fn prepare_arc_request_action(
    invocation: &McpInvocation,
    metadata: Option<&McpToolApprovalMetadata>,
) -> serde_json::Value {
    let request = build_guardian_mcp_tool_review_request("arc-monitor", invocation, metadata);
    guardian_approval_request_to_json(&request)
}

fn session_mcp_tool_approval_key(
    invocation: &McpInvocation,
    metadata: Option<&McpToolApprovalMetadata>,
    approval_mode: AppToolApproval,
) -> Option<McpToolApprovalKey> {
    if approval_mode != AppToolApproval::Auto {
        return None;
    }

    let connector_id = metadata.and_then(|metadata| metadata.connector_id.clone());
    if invocation.server == CODEX_APPS_MCP_SERVER_NAME && connector_id.is_none() {
        return None;
    }

    Some(McpToolApprovalKey {
        server: invocation.server.clone(),
        connector_id,
        tool_name: invocation.tool.clone(),
    })
}

fn persistent_mcp_tool_approval_key(
    invocation: &McpInvocation,
    metadata: Option<&McpToolApprovalMetadata>,
    approval_mode: AppToolApproval,
) -> Option<McpToolApprovalKey> {
    if invocation.server != CODEX_APPS_MCP_SERVER_NAME {
        return None;
    }

    session_mcp_tool_approval_key(invocation, metadata, approval_mode)
        .filter(|key| key.connector_id.is_some())
}

pub(crate) fn build_guardian_mcp_tool_review_request(
    call_id: &str,
    invocation: &McpInvocation,
    metadata: Option<&McpToolApprovalMetadata>,
) -> GuardianApprovalRequest {
    GuardianApprovalRequest::McpToolCall {
        id: call_id.to_string(),
        server: invocation.server.clone(),
        tool_name: invocation.tool.clone(),
        arguments: invocation.arguments.clone(),
        connector_id: metadata.and_then(|metadata| metadata.connector_id.clone()),
        connector_name: metadata.and_then(|metadata| metadata.connector_name.clone()),
        connector_description: metadata.and_then(|metadata| metadata.connector_description.clone()),
        tool_title: metadata.and_then(|metadata| metadata.tool_title.clone()),
        tool_description: metadata.and_then(|metadata| metadata.tool_description.clone()),
        annotations: metadata
            .and_then(|metadata| metadata.annotations.as_ref())
            .map(|annotations| GuardianMcpAnnotations {
                destructive_hint: annotations.destructive_hint,
                open_world_hint: annotations.open_world_hint,
                read_only_hint: annotations.read_only_hint,
            }),
    }
}

fn mcp_tool_approval_decision_from_guardian(decision: ReviewDecision) -> McpToolApprovalDecision {
    match decision {
        ReviewDecision::Approved
        | ReviewDecision::ApprovedExecpolicyAmendment { .. }
        | ReviewDecision::NetworkPolicyAmendment { .. } => McpToolApprovalDecision::Accept,
        ReviewDecision::ApprovedForSession => McpToolApprovalDecision::AcceptForSession,
        ReviewDecision::Denied | ReviewDecision::Abort => McpToolApprovalDecision::Decline,
    }
}

fn is_full_access_mode(turn_context: &TurnContext) -> bool {
    matches!(turn_context.approval_policy.value(), AskForApproval::Never)
        && matches!(
            turn_context.sandbox_policy.get(),
            SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. }
        )
}

pub(crate) async fn lookup_mcp_tool_metadata(
    sess: &Session,
    turn_context: &TurnContext,
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

    let tool_info = tools
        .into_values()
        .find(|tool_info| tool_info.server_name == server && tool_info.tool.name == tool_name)?;
    let connector_description = if server == CODEX_APPS_MCP_SERVER_NAME {
        let connectors = match connectors::list_cached_accessible_connectors_from_mcp_tools(
            turn_context.config.as_ref(),
        )
        .await
        {
            Some(connectors) => Some(connectors),
            None => {
                connectors::list_accessible_connectors_from_mcp_tools(turn_context.config.as_ref())
                    .await
                    .ok()
            }
        };
        connectors.and_then(|connectors| {
            let connector_id = tool_info.connector_id.as_deref()?;
            connectors
                .into_iter()
                .find(|connector| connector.id == connector_id)
                .and_then(|connector| connector.description)
        })
    } else {
        None
    };

    Some(McpToolApprovalMetadata {
        annotations: tool_info.tool.annotations,
        connector_id: tool_info.connector_id,
        connector_name: tool_info.connector_name,
        connector_description,
        tool_title: tool_info.tool.title,
        tool_description: tool_info.tool.description.map(std::borrow::Cow::into_owned),
    })
}

async fn lookup_mcp_app_usage_metadata(
    sess: &Session,
    server: &str,
    tool_name: &str,
) -> Option<McpAppUsageMetadata> {
    let tools = sess
        .services
        .mcp_connection_manager
        .read()
        .await
        .list_all_tools()
        .await;

    tools.into_values().find_map(|tool_info| {
        if tool_info.server_name == server && tool_info.tool.name == tool_name {
            Some(McpAppUsageMetadata {
                connector_id: tool_info.connector_id,
                app_name: tool_info.connector_name,
            })
        } else {
            None
        }
    })
}

fn build_mcp_tool_approval_question(
    question_id: String,
    server: &str,
    tool_name: &str,
    connector_name: Option<&str>,
    prompt_options: McpToolApprovalPromptOptions,
    question_override: Option<&str>,
) -> RequestUserInputQuestion {
    let question = question_override
        .map(ToString::to_string)
        .unwrap_or_else(|| {
            build_mcp_tool_approval_fallback_message(server, tool_name, connector_name)
        });
    let question = format!("{}?", question.trim_end_matches('?'));

    let mut options = vec![RequestUserInputQuestionOption {
        label: MCP_TOOL_APPROVAL_ACCEPT.to_string(),
        description: "Run the tool and continue.".to_string(),
    }];
    if prompt_options.allow_session_remember {
        options.push(RequestUserInputQuestionOption {
            label: MCP_TOOL_APPROVAL_ACCEPT_FOR_SESSION.to_string(),
            description: "Run the tool and remember this choice for this session.".to_string(),
        });
    }
    if prompt_options.allow_persistent_approval {
        options.push(RequestUserInputQuestionOption {
            label: MCP_TOOL_APPROVAL_ACCEPT_AND_REMEMBER.to_string(),
            description: "Run the tool and remember this choice for future tool calls.".to_string(),
        });
    }
    options.push(RequestUserInputQuestionOption {
        label: MCP_TOOL_APPROVAL_CANCEL.to_string(),
        description: "Cancel this tool call.".to_string(),
    });

    RequestUserInputQuestion {
        id: question_id,
        header: "Approve app tool call?".to_string(),
        question,
        is_other: false,
        is_secret: false,
        options: Some(options),
    }
}

fn build_mcp_tool_approval_fallback_message(
    server: &str,
    tool_name: &str,
    connector_name: Option<&str>,
) -> String {
    let actor = connector_name
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| {
            if server == CODEX_APPS_MCP_SERVER_NAME {
                "this app".to_string()
            } else {
                format!("the {server} MCP server")
            }
        });
    format!("Allow {actor} to run tool \"{tool_name}\"?")
}

fn mcp_tool_approval_question_text(question: String, monitor_reason: Option<&str>) -> String {
    match monitor_reason.map(str::trim) {
        Some(reason) if !reason.is_empty() => {
            format!("Tool call needs your approval. Reason: {reason}")
        }
        _ => question,
    }
}

fn arc_monitor_interrupt_message(reason: &str) -> String {
    let reason = reason.trim();
    if reason.is_empty() {
        "Tool call was cancelled because of safety risks.".to_string()
    } else {
        format!("Tool call was cancelled because of safety risks: {reason}")
    }
}

fn build_mcp_tool_approval_elicitation_request(
    sess: &Session,
    turn_context: &TurnContext,
    request: McpToolApprovalElicitationRequest<'_>,
) -> McpServerElicitationRequestParams {
    let message = request
        .message_override
        .map(ToString::to_string)
        .unwrap_or_else(|| request.question.question.clone());

    McpServerElicitationRequestParams {
        thread_id: sess.conversation_id.to_string(),
        turn_id: Some(turn_context.sub_id.clone()),
        server_name: request.server.to_string(),
        request: McpServerElicitationRequest::Form {
            meta: build_mcp_tool_approval_elicitation_meta(
                request.server,
                request.metadata,
                request.tool_params,
                request.tool_params_display,
                request.prompt_options,
            ),
            message,
            requested_schema: McpElicitationSchema {
                schema_uri: None,
                type_: McpElicitationObjectType::Object,
                properties: BTreeMap::new(),
                required: None,
            },
        },
    }
}

fn build_mcp_tool_approval_elicitation_meta(
    server: &str,
    metadata: Option<&McpToolApprovalMetadata>,
    tool_params: Option<&serde_json::Value>,
    tool_params_display: Option<&[RenderedMcpToolApprovalParam]>,
    prompt_options: McpToolApprovalPromptOptions,
) -> Option<serde_json::Value> {
    let mut meta = serde_json::Map::new();
    meta.insert(
        MCP_TOOL_APPROVAL_KIND_KEY.to_string(),
        serde_json::Value::String(MCP_TOOL_APPROVAL_KIND_MCP_TOOL_CALL.to_string()),
    );
    match (
        prompt_options.allow_session_remember,
        prompt_options.allow_persistent_approval,
    ) {
        (true, true) => {
            meta.insert(
                MCP_TOOL_APPROVAL_PERSIST_KEY.to_string(),
                serde_json::json!([
                    MCP_TOOL_APPROVAL_PERSIST_SESSION,
                    MCP_TOOL_APPROVAL_PERSIST_ALWAYS,
                ]),
            );
        }
        (true, false) => {
            meta.insert(
                MCP_TOOL_APPROVAL_PERSIST_KEY.to_string(),
                serde_json::Value::String(MCP_TOOL_APPROVAL_PERSIST_SESSION.to_string()),
            );
        }
        (false, true) => {
            meta.insert(
                MCP_TOOL_APPROVAL_PERSIST_KEY.to_string(),
                serde_json::Value::String(MCP_TOOL_APPROVAL_PERSIST_ALWAYS.to_string()),
            );
        }
        (false, false) => {}
    }
    if let Some(metadata) = metadata {
        if let Some(tool_title) = metadata.tool_title.as_ref() {
            meta.insert(
                MCP_TOOL_APPROVAL_TOOL_TITLE_KEY.to_string(),
                serde_json::Value::String(tool_title.clone()),
            );
        }
        if let Some(tool_description) = metadata.tool_description.as_ref() {
            meta.insert(
                MCP_TOOL_APPROVAL_TOOL_DESCRIPTION_KEY.to_string(),
                serde_json::Value::String(tool_description.clone()),
            );
        }
        if server == CODEX_APPS_MCP_SERVER_NAME
            && (metadata.connector_id.is_some()
                || metadata.connector_name.is_some()
                || metadata.connector_description.is_some())
        {
            meta.insert(
                MCP_TOOL_APPROVAL_SOURCE_KEY.to_string(),
                serde_json::Value::String(MCP_TOOL_APPROVAL_SOURCE_CONNECTOR.to_string()),
            );
            if let Some(connector_id) = metadata.connector_id.as_deref() {
                meta.insert(
                    MCP_TOOL_APPROVAL_CONNECTOR_ID_KEY.to_string(),
                    serde_json::Value::String(connector_id.to_string()),
                );
            }
            if let Some(connector_name) = metadata.connector_name.as_ref() {
                meta.insert(
                    MCP_TOOL_APPROVAL_CONNECTOR_NAME_KEY.to_string(),
                    serde_json::Value::String(connector_name.clone()),
                );
            }
            if let Some(connector_description) = metadata.connector_description.as_ref() {
                meta.insert(
                    MCP_TOOL_APPROVAL_CONNECTOR_DESCRIPTION_KEY.to_string(),
                    serde_json::Value::String(connector_description.clone()),
                );
            }
        }
    }
    if let Some(tool_params) = tool_params {
        meta.insert(
            MCP_TOOL_APPROVAL_TOOL_PARAMS_KEY.to_string(),
            tool_params.clone(),
        );
    }
    if let Some(tool_params_display) = tool_params_display
        && let Ok(tool_params_display) = serde_json::to_value(tool_params_display)
    {
        meta.insert(
            MCP_TOOL_APPROVAL_TOOL_PARAMS_DISPLAY_KEY.to_string(),
            tool_params_display,
        );
    }
    (!meta.is_empty()).then_some(serde_json::Value::Object(meta))
}

fn build_mcp_tool_approval_display_params(
    tool_params: Option<&serde_json::Value>,
) -> Option<Vec<crate::mcp_tool_approval_templates::RenderedMcpToolApprovalParam>> {
    let tool_params = tool_params?.as_object()?;
    let mut display_params = tool_params
        .iter()
        .map(
            |(name, value)| crate::mcp_tool_approval_templates::RenderedMcpToolApprovalParam {
                name: name.clone(),
                value: value.clone(),
            },
        )
        .collect::<Vec<_>>();
    display_params.sort_by(|left, right| left.name.cmp(&right.name));
    Some(display_params)
}

fn parse_mcp_tool_approval_elicitation_response(
    response: Option<ElicitationResponse>,
    question_id: &str,
) -> McpToolApprovalDecision {
    let Some(response) = response else {
        return McpToolApprovalDecision::Cancel;
    };
    match response.action {
        ElicitationAction::Accept => {
            match response
                .meta
                .as_ref()
                .and_then(serde_json::Value::as_object)
                .and_then(|meta| meta.get(MCP_TOOL_APPROVAL_PERSIST_KEY))
                .and_then(serde_json::Value::as_str)
            {
                Some(MCP_TOOL_APPROVAL_PERSIST_SESSION) => {
                    return McpToolApprovalDecision::AcceptForSession;
                }
                Some(MCP_TOOL_APPROVAL_PERSIST_ALWAYS) => {
                    return McpToolApprovalDecision::AcceptAndRemember;
                }
                _ => {}
            }

            match parse_mcp_tool_approval_response(
                request_user_input_response_from_elicitation_content(response.content),
                question_id,
            ) {
                McpToolApprovalDecision::Cancel => McpToolApprovalDecision::Accept,
                decision => decision,
            }
        }
        ElicitationAction::Decline => McpToolApprovalDecision::Decline,
        ElicitationAction::Cancel => McpToolApprovalDecision::Cancel,
    }
}

fn request_user_input_response_from_elicitation_content(
    content: Option<serde_json::Value>,
) -> Option<RequestUserInputResponse> {
    let Some(content) = content else {
        return Some(RequestUserInputResponse {
            answers: std::collections::HashMap::new(),
        });
    };
    let content = content.as_object()?;
    let answers = content
        .iter()
        .filter_map(|(question_id, value)| {
            let answers = match value {
                serde_json::Value::String(answer) => vec![answer.clone()],
                serde_json::Value::Array(values) => values
                    .iter()
                    .filter_map(|value| value.as_str().map(ToString::to_string))
                    .collect(),
                _ => return None,
            };
            Some((question_id.clone(), RequestUserInputAnswer { answers }))
        })
        .collect();

    Some(RequestUserInputResponse { answers })
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
        .any(|answer| answer == MCP_TOOL_APPROVAL_ACCEPT_FOR_SESSION)
    {
        McpToolApprovalDecision::AcceptForSession
    } else if answers
        .iter()
        .any(|answer| answer == MCP_TOOL_APPROVAL_ACCEPT_AND_REMEMBER)
    {
        McpToolApprovalDecision::AcceptAndRemember
    } else if answers
        .iter()
        .any(|answer| answer == MCP_TOOL_APPROVAL_ACCEPT)
    {
        McpToolApprovalDecision::Accept
    } else {
        McpToolApprovalDecision::Cancel
    }
}

fn normalize_approval_decision_for_mode(
    decision: McpToolApprovalDecision,
    approval_mode: AppToolApproval,
) -> McpToolApprovalDecision {
    if approval_mode == AppToolApproval::Prompt
        && matches!(
            decision,
            McpToolApprovalDecision::AcceptForSession | McpToolApprovalDecision::AcceptAndRemember
        )
    {
        McpToolApprovalDecision::Accept
    } else {
        decision
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

async fn apply_mcp_tool_approval_decision(
    sess: &Session,
    turn_context: &TurnContext,
    decision: &McpToolApprovalDecision,
    session_approval_key: Option<McpToolApprovalKey>,
    persistent_approval_key: Option<McpToolApprovalKey>,
) {
    match decision {
        McpToolApprovalDecision::AcceptForSession => {
            if let Some(key) = session_approval_key {
                remember_mcp_tool_approval(sess, key).await;
            }
        }
        McpToolApprovalDecision::AcceptAndRemember => {
            if let Some(key) = persistent_approval_key {
                maybe_persist_mcp_tool_approval(sess, turn_context, key).await;
            } else if let Some(key) = session_approval_key {
                remember_mcp_tool_approval(sess, key).await;
            }
        }
        McpToolApprovalDecision::Accept
        | McpToolApprovalDecision::Decline
        | McpToolApprovalDecision::Cancel
        | McpToolApprovalDecision::BlockedBySafetyMonitor(_) => {}
    }
}

async fn maybe_persist_mcp_tool_approval(
    sess: &Session,
    turn_context: &TurnContext,
    key: McpToolApprovalKey,
) {
    let Some(connector_id) = key.connector_id.clone() else {
        remember_mcp_tool_approval(sess, key).await;
        return;
    };
    let tool_name = key.tool_name.clone();

    if let Err(err) =
        persist_codex_app_tool_approval(&turn_context.config.codex_home, &connector_id, &tool_name)
            .await
    {
        error!(
            error = %err,
            connector_id,
            tool_name,
            "failed to persist codex app tool approval"
        );
        remember_mcp_tool_approval(sess, key).await;
        return;
    }

    sess.reload_user_config_layer().await;
    remember_mcp_tool_approval(sess, key).await;
}

async fn persist_codex_app_tool_approval(
    codex_home: &Path,
    connector_id: &str,
    tool_name: &str,
) -> anyhow::Result<()> {
    ConfigEditsBuilder::new(codex_home)
        .with_edits([ConfigEdit::SetPath {
            segments: vec![
                "apps".to_string(),
                connector_id.to_string(),
                "tools".to_string(),
                tool_name.to_string(),
                "approval_mode".to_string(),
            ],
            value: value("approve"),
        }])
        .apply()
        .await
}

fn requires_mcp_tool_approval(annotations: &ToolAnnotations) -> bool {
    if annotations.destructive_hint == Some(true) {
        return true;
    }

    annotations.read_only_hint == Some(false) && annotations.open_world_hint == Some(true)
}

async fn notify_mcp_tool_call_skip(
    sess: &Session,
    turn_context: &TurnContext,
    call_id: &str,
    invocation: McpInvocation,
    message: String,
    already_started: bool,
) -> Result<CallToolResult, String> {
    if !already_started {
        let tool_call_begin_event = EventMsg::McpToolCallBegin(McpToolCallBeginEvent {
            call_id: call_id.to_string(),
            invocation: invocation.clone(),
        });
        notify_mcp_tool_call_event(sess, turn_context, tool_call_begin_event).await;
    }

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
#[path = "mcp_tool_call_tests.rs"]
mod tests;
