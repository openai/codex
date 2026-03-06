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
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::config::types::AppToolApproval;
use crate::connectors;
use crate::features::Feature;
use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
use crate::protocol::EventMsg;
use crate::protocol::McpInvocation;
use crate::protocol::McpToolCallBeginEvent;
use crate::protocol::McpToolCallEndEvent;
use crate::state_db;
use codex_protocol::mcp::CallToolResult;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
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

    let metadata = lookup_mcp_tool_metadata(sess.as_ref(), turn_context, &server, &tool_name).await;
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
            turn_context,
            &call_id,
            invocation,
            "MCP tool call blocked by app configuration".to_string(),
        )
        .await;
        let status = if result.is_ok() { "ok" } else { "error" };
        turn_context
            .otel_manager
            .counter("codex.mcp.call", 1, &[("status", status)]);
        return ResponseInputItem::McpToolCallOutput { call_id, result };
    }

    if let Some(decision) = maybe_request_mcp_tool_approval(
        sess.as_ref(),
        turn_context,
        &call_id,
        &invocation,
        metadata.as_ref(),
        app_tool_policy.approval,
    )
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
                maybe_mark_thread_memory_mode_polluted(sess.as_ref(), turn_context).await;

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
                    turn_context,
                    tool_call_end_event.clone(),
                )
                .await;
                maybe_track_codex_app_used(sess.as_ref(), turn_context, &server, &tool_name).await;
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
    maybe_mark_thread_memory_mode_polluted(sess.as_ref(), turn_context).await;

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

    notify_mcp_tool_call_event(sess.as_ref(), turn_context, tool_call_end_event.clone()).await;
    maybe_track_codex_app_used(sess.as_ref(), turn_context, &server, &tool_name).await;

    let status = if result.is_ok() { "ok" } else { "error" };
    turn_context
        .otel_manager
        .counter("codex.mcp.call", 1, &[("status", status)]);

    ResponseInputItem::McpToolCallOutput { call_id, result }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum McpToolApprovalDecision {
    Accept,
    AcceptAndRemember,
    Decline,
    Cancel,
}

struct McpToolApprovalMetadata {
    annotations: Option<ToolAnnotations>,
    connector_id: Option<String>,
    connector_name: Option<String>,
    connector_description: Option<String>,
    tool_title: Option<String>,
    tool_description: Option<String>,
}

const MCP_TOOL_APPROVAL_QUESTION_ID_PREFIX: &str = "mcp_tool_call_approval";
const MCP_TOOL_APPROVAL_ACCEPT: &str = "Approve Once";
const MCP_TOOL_APPROVAL_ACCEPT_AND_REMEMBER: &str = "Approve this Session";
const MCP_TOOL_APPROVAL_DECLINE: &str = "Deny";
const MCP_TOOL_APPROVAL_CANCEL: &str = "Cancel";
const MCP_TOOL_APPROVAL_KIND_KEY: &str = "codex_approval_kind";
const MCP_TOOL_APPROVAL_KIND_MCP_TOOL_CALL: &str = "mcp_tool_call";
const MCP_TOOL_APPROVAL_PERSIST_KEY: &str = "persist";
const MCP_TOOL_APPROVAL_PERSIST_SESSION: &str = "session";
const MCP_TOOL_APPROVAL_SOURCE_KEY: &str = "source";
const MCP_TOOL_APPROVAL_SOURCE_CONNECTOR: &str = "connector";
const MCP_TOOL_APPROVAL_CONNECTOR_ID_KEY: &str = "connector_id";
const MCP_TOOL_APPROVAL_CONNECTOR_NAME_KEY: &str = "connector_name";
const MCP_TOOL_APPROVAL_CONNECTOR_DESCRIPTION_KEY: &str = "connector_description";
const MCP_TOOL_APPROVAL_TOOL_TITLE_KEY: &str = "tool_title";
const MCP_TOOL_APPROVAL_TOOL_DESCRIPTION_KEY: &str = "tool_description";
const MCP_TOOL_APPROVAL_TOOL_PARAMS_KEY: &str = "tool_params";

#[derive(Debug, Serialize)]
struct McpToolApprovalKey {
    server: String,
    connector_id: Option<String>,
    tool_name: String,
}

async fn maybe_request_mcp_tool_approval(
    sess: &Session,
    turn_context: &TurnContext,
    call_id: &str,
    invocation: &McpInvocation,
    metadata: Option<&McpToolApprovalMetadata>,
    approval_mode: AppToolApproval,
) -> Option<McpToolApprovalDecision> {
    if approval_mode == AppToolApproval::Approve {
        return None;
    }
    let annotations = metadata.and_then(|metadata| metadata.annotations.as_ref());
    if approval_mode == AppToolApproval::Auto {
        if is_full_access_mode(turn_context) {
            return None;
        }
        if !annotations.is_some_and(requires_mcp_tool_approval) {
            return None;
        }
    }

    let approval_key = if approval_mode == AppToolApproval::Auto {
        let connector_id = metadata.and_then(|metadata| metadata.connector_id.clone());
        if invocation.server == CODEX_APPS_MCP_SERVER_NAME && connector_id.is_none() {
            None
        } else {
            Some(McpToolApprovalKey {
                server: invocation.server.clone(),
                connector_id,
                tool_name: invocation.tool.clone(),
            })
        }
    } else {
        None
    };
    if let Some(key) = approval_key.as_ref()
        && mcp_tool_approval_is_remembered(sess, key).await
    {
        return Some(McpToolApprovalDecision::Accept);
    }

    let question_id = format!("{MCP_TOOL_APPROVAL_QUESTION_ID_PREFIX}_{call_id}");
    let question = build_mcp_tool_approval_question(
        question_id.clone(),
        &invocation.server,
        &invocation.tool,
        metadata.and_then(|metadata| metadata.tool_title.as_deref()),
        metadata.and_then(|metadata| metadata.connector_name.as_deref()),
        annotations,
        approval_key.is_some(),
    );
    if turn_context
        .config
        .features
        .enabled(Feature::ToolCallMcpElicitation)
    {
        let request_id = rmcp::model::RequestId::String(
            format!("{MCP_TOOL_APPROVAL_QUESTION_ID_PREFIX}_{call_id}").into(),
        );
        let params = build_mcp_tool_approval_elicitation_request(
            sess,
            turn_context,
            &invocation.server,
            metadata,
            invocation.arguments.as_ref(),
            question.clone(),
            approval_key.is_some(),
        );
        let decision = parse_mcp_tool_approval_elicitation_response(
            sess.request_mcp_server_elicitation(turn_context, request_id, params)
                .await,
            &question_id,
        );
        let decision = normalize_approval_decision_for_mode(decision, approval_mode);
        if matches!(decision, McpToolApprovalDecision::AcceptAndRemember)
            && let Some(key) = approval_key
        {
            remember_mcp_tool_approval(sess, key).await;
        }
        return Some(decision);
    }

    let args = RequestUserInputArgs {
        questions: vec![question],
    };
    let response = sess
        .request_user_input(turn_context, call_id.to_string(), args)
        .await;
    let decision = normalize_approval_decision_for_mode(
        parse_mcp_tool_approval_response(response, &question_id),
        approval_mode,
    );
    if matches!(decision, McpToolApprovalDecision::AcceptAndRemember)
        && let Some(key) = approval_key
    {
        remember_mcp_tool_approval(sess, key).await;
    }
    Some(decision)
}

fn is_full_access_mode(turn_context: &TurnContext) -> bool {
    matches!(turn_context.approval_policy.value(), AskForApproval::Never)
        && matches!(
            turn_context.sandbox_policy.get(),
            SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. }
        )
}

async fn lookup_mcp_tool_metadata(
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
        .find(|tool_info| tool_info.server_name == server && tool_info.tool_name == tool_name)?;
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
        if tool_info.server_name == server && tool_info.tool_name == tool_name {
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
    tool_title: Option<&str>,
    connector_name: Option<&str>,
    annotations: Option<&ToolAnnotations>,
    allow_remember_option: bool,
) -> RequestUserInputQuestion {
    let destructive =
        annotations.and_then(|annotations| annotations.destructive_hint) == Some(true);
    let open_world = annotations.and_then(|annotations| annotations.open_world_hint) == Some(true);
    let reason = match (destructive, open_world) {
        (true, true) => "may modify data and access external systems",
        (true, false) => "may modify or delete data",
        (false, true) => "may access external systems",
        (false, false) => "may have side effects",
    };

    let tool_label = tool_title.unwrap_or(tool_name);
    let app_label = connector_name
        .map(|name| format!("The {name} app"))
        .unwrap_or_else(|| {
            if server == CODEX_APPS_MCP_SERVER_NAME {
                "This app".to_string()
            } else {
                format!("The {server} MCP server")
            }
        });
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

fn build_mcp_tool_approval_elicitation_request(
    sess: &Session,
    turn_context: &TurnContext,
    server: &str,
    metadata: Option<&McpToolApprovalMetadata>,
    tool_params: Option<&serde_json::Value>,
    question: RequestUserInputQuestion,
    allow_session_persist: bool,
) -> McpServerElicitationRequestParams {
    let message = if question.header.trim().is_empty() {
        question.question
    } else {
        let header = question.header;
        let prompt = question.question;
        format!("{header}\n\n{prompt}")
    };

    McpServerElicitationRequestParams {
        thread_id: sess.conversation_id.to_string(),
        turn_id: Some(turn_context.sub_id.clone()),
        server_name: server.to_string(),
        request: McpServerElicitationRequest::Form {
            meta: build_mcp_tool_approval_elicitation_meta(
                server,
                metadata,
                tool_params,
                allow_session_persist,
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
    allow_session_persist: bool,
) -> Option<serde_json::Value> {
    let mut meta = serde_json::Map::new();
    meta.insert(
        MCP_TOOL_APPROVAL_KIND_KEY.to_string(),
        serde_json::Value::String(MCP_TOOL_APPROVAL_KIND_MCP_TOOL_CALL.to_string()),
    );
    if allow_session_persist {
        meta.insert(
            MCP_TOOL_APPROVAL_PERSIST_KEY.to_string(),
            serde_json::Value::String(MCP_TOOL_APPROVAL_PERSIST_SESSION.to_string()),
        );
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
    (!meta.is_empty()).then_some(serde_json::Value::Object(meta))
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
            if response
                .meta
                .as_ref()
                .and_then(serde_json::Value::as_object)
                .and_then(|meta| meta.get(MCP_TOOL_APPROVAL_PERSIST_KEY))
                .and_then(serde_json::Value::as_str)
                == Some(MCP_TOOL_APPROVAL_PERSIST_SESSION)
            {
                return McpToolApprovalDecision::AcceptAndRemember;
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

fn normalize_approval_decision_for_mode(
    decision: McpToolApprovalDecision,
    approval_mode: AppToolApproval,
) -> McpToolApprovalDecision {
    if approval_mode == AppToolApproval::Prompt
        && decision == McpToolApprovalDecision::AcceptAndRemember
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

    fn approval_metadata(
        connector_id: Option<&str>,
        connector_name: Option<&str>,
        connector_description: Option<&str>,
        tool_title: Option<&str>,
        tool_description: Option<&str>,
    ) -> McpToolApprovalMetadata {
        McpToolApprovalMetadata {
            annotations: None,
            connector_id: connector_id.map(str::to_string),
            connector_name: connector_name.map(str::to_string),
            connector_description: connector_description.map(str::to_string),
            tool_title: tool_title.map(str::to_string),
            tool_description: tool_description.map(str::to_string),
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
    fn approval_required_when_destructive_even_if_read_only_true() {
        let annotations = annotations(Some(true), Some(true), Some(true));
        assert_eq!(requires_mcp_tool_approval(&annotations), true);
    }

    #[test]
    fn prompt_mode_does_not_allow_session_remember() {
        assert_eq!(
            normalize_approval_decision_for_mode(
                McpToolApprovalDecision::AcceptAndRemember,
                AppToolApproval::Prompt,
            ),
            McpToolApprovalDecision::Accept
        );
    }

    #[test]
    fn custom_mcp_tool_question_mentions_server_name() {
        let question = build_mcp_tool_approval_question(
            "q".to_string(),
            "custom_server",
            "run_action",
            Some("Run Action"),
            None,
            Some(&annotations(Some(false), Some(true), None)),
            true,
        );

        assert_eq!(question.header, "Approve app tool call?");
        assert_eq!(
            question.question,
            "The custom_server MCP server wants to run the tool \"Run Action\", which may modify or delete data. Allow this action?"
        );
        assert!(
            question
                .options
                .expect("options")
                .into_iter()
                .map(|option| option.label)
                .any(|label| label == MCP_TOOL_APPROVAL_ACCEPT_AND_REMEMBER)
        );
    }

    #[test]
    fn codex_apps_tool_question_keeps_legacy_app_label() {
        let question = build_mcp_tool_approval_question(
            "q".to_string(),
            CODEX_APPS_MCP_SERVER_NAME,
            "run_action",
            Some("Run Action"),
            None,
            Some(&annotations(Some(false), Some(true), None)),
            true,
        );

        assert!(
            question
                .question
                .starts_with("This app wants to run the tool \"Run Action\"")
        );
    }

    #[test]
    fn sanitize_mcp_tool_result_for_model_rewrites_image_content() {
        let result = Ok(CallToolResult {
            content: vec![
                serde_json::json!({
                    "type": "image",
                    "data": "Zm9v",
                    "mimeType": "image/png",
                }),
                serde_json::json!({
                    "type": "text",
                    "text": "hello",
                }),
            ],
            structured_content: None,
            is_error: Some(false),
            meta: None,
        });

        let got = sanitize_mcp_tool_result_for_model(false, result).expect("sanitized result");

        assert_eq!(
            got.content,
            vec![
                serde_json::json!({
                    "type": "text",
                    "text": "<image content omitted because you do not support image input>",
                }),
                serde_json::json!({
                    "type": "text",
                    "text": "hello",
                }),
            ]
        );
    }

    #[test]
    fn sanitize_mcp_tool_result_for_model_preserves_image_when_supported() {
        let original = CallToolResult {
            content: vec![serde_json::json!({
                "type": "image",
                "data": "Zm9v",
                "mimeType": "image/png",
            })],
            structured_content: Some(serde_json::json!({"x": 1})),
            is_error: Some(false),
            meta: Some(serde_json::json!({"k": "v"})),
        };

        let got = sanitize_mcp_tool_result_for_model(true, Ok(original.clone()))
            .expect("unsanitized result");

        assert_eq!(got, original);
    }

    #[test]
    fn accepted_elicitation_content_converts_to_request_user_input_response() {
        let response =
            request_user_input_response_from_elicitation_content(Some(serde_json::json!(
                {
                    "approval": MCP_TOOL_APPROVAL_ACCEPT_AND_REMEMBER,
                }
            )));

        assert_eq!(
            response,
            Some(RequestUserInputResponse {
                answers: std::collections::HashMap::from([(
                    "approval".to_string(),
                    RequestUserInputAnswer {
                        answers: vec![MCP_TOOL_APPROVAL_ACCEPT_AND_REMEMBER.to_string()],
                    },
                )]),
            })
        );
    }

    #[test]
    fn approval_elicitation_meta_marks_tool_approvals() {
        assert_eq!(
            build_mcp_tool_approval_elicitation_meta("custom_server", None, None, false),
            Some(serde_json::json!({
                MCP_TOOL_APPROVAL_KIND_KEY: MCP_TOOL_APPROVAL_KIND_MCP_TOOL_CALL,
            }))
        );
    }

    #[test]
    fn approval_elicitation_meta_keeps_session_persist_behavior() {
        assert_eq!(
            build_mcp_tool_approval_elicitation_meta(
                "custom_server",
                Some(&approval_metadata(
                    None,
                    None,
                    None,
                    Some("Run Action"),
                    Some("Runs the selected action."),
                )),
                Some(&serde_json::json!({"id": 1})),
                true,
            ),
            Some(serde_json::json!({
                MCP_TOOL_APPROVAL_KIND_KEY: MCP_TOOL_APPROVAL_KIND_MCP_TOOL_CALL,
                MCP_TOOL_APPROVAL_PERSIST_KEY: MCP_TOOL_APPROVAL_PERSIST_SESSION,
                MCP_TOOL_APPROVAL_TOOL_TITLE_KEY: "Run Action",
                MCP_TOOL_APPROVAL_TOOL_DESCRIPTION_KEY: "Runs the selected action.",
                MCP_TOOL_APPROVAL_TOOL_PARAMS_KEY: {
                    "id": 1,
                },
            }))
        );
    }

    #[test]
    fn approval_elicitation_meta_includes_connector_source_for_codex_apps() {
        assert_eq!(
            build_mcp_tool_approval_elicitation_meta(
                CODEX_APPS_MCP_SERVER_NAME,
                Some(&approval_metadata(
                    Some("calendar"),
                    Some("Calendar"),
                    Some("Manage events and schedules."),
                    Some("Run Action"),
                    Some("Runs the selected action."),
                )),
                Some(&serde_json::json!({
                    "calendar_id": "primary",
                })),
                false,
            ),
            Some(serde_json::json!({
                MCP_TOOL_APPROVAL_KIND_KEY: MCP_TOOL_APPROVAL_KIND_MCP_TOOL_CALL,
                MCP_TOOL_APPROVAL_SOURCE_KEY: MCP_TOOL_APPROVAL_SOURCE_CONNECTOR,
                MCP_TOOL_APPROVAL_CONNECTOR_ID_KEY: "calendar",
                MCP_TOOL_APPROVAL_CONNECTOR_NAME_KEY: "Calendar",
                MCP_TOOL_APPROVAL_CONNECTOR_DESCRIPTION_KEY: "Manage events and schedules.",
                MCP_TOOL_APPROVAL_TOOL_TITLE_KEY: "Run Action",
                MCP_TOOL_APPROVAL_TOOL_DESCRIPTION_KEY: "Runs the selected action.",
                MCP_TOOL_APPROVAL_TOOL_PARAMS_KEY: {
                    "calendar_id": "primary",
                },
            }))
        );
    }

    #[test]
    fn approval_elicitation_meta_merges_session_persist_with_connector_source() {
        assert_eq!(
            build_mcp_tool_approval_elicitation_meta(
                CODEX_APPS_MCP_SERVER_NAME,
                Some(&approval_metadata(
                    Some("calendar"),
                    Some("Calendar"),
                    Some("Manage events and schedules."),
                    Some("Run Action"),
                    Some("Runs the selected action."),
                )),
                Some(&serde_json::json!({
                    "calendar_id": "primary",
                })),
                true,
            ),
            Some(serde_json::json!({
                MCP_TOOL_APPROVAL_KIND_KEY: MCP_TOOL_APPROVAL_KIND_MCP_TOOL_CALL,
                MCP_TOOL_APPROVAL_PERSIST_KEY: MCP_TOOL_APPROVAL_PERSIST_SESSION,
                MCP_TOOL_APPROVAL_SOURCE_KEY: MCP_TOOL_APPROVAL_SOURCE_CONNECTOR,
                MCP_TOOL_APPROVAL_CONNECTOR_ID_KEY: "calendar",
                MCP_TOOL_APPROVAL_CONNECTOR_NAME_KEY: "Calendar",
                MCP_TOOL_APPROVAL_CONNECTOR_DESCRIPTION_KEY: "Manage events and schedules.",
                MCP_TOOL_APPROVAL_TOOL_TITLE_KEY: "Run Action",
                MCP_TOOL_APPROVAL_TOOL_DESCRIPTION_KEY: "Runs the selected action.",
                MCP_TOOL_APPROVAL_TOOL_PARAMS_KEY: {
                    "calendar_id": "primary",
                },
            }))
        );
    }

    #[test]
    fn declined_elicitation_response_stays_decline() {
        let response = parse_mcp_tool_approval_elicitation_response(
            Some(ElicitationResponse {
                action: ElicitationAction::Decline,
                content: Some(serde_json::json!({
                    "approval": MCP_TOOL_APPROVAL_ACCEPT,
                })),
                meta: None,
            }),
            "approval",
        );

        assert_eq!(response, McpToolApprovalDecision::Decline);
    }

    #[test]
    fn accepted_elicitation_response_uses_session_persist_meta() {
        let response = parse_mcp_tool_approval_elicitation_response(
            Some(ElicitationResponse {
                action: ElicitationAction::Accept,
                content: None,
                meta: Some(serde_json::json!({
                    MCP_TOOL_APPROVAL_PERSIST_KEY: MCP_TOOL_APPROVAL_PERSIST_SESSION,
                })),
            }),
            "approval",
        );

        assert_eq!(response, McpToolApprovalDecision::AcceptAndRemember);
    }

    #[test]
    fn accepted_elicitation_without_content_defaults_to_accept() {
        let response = parse_mcp_tool_approval_elicitation_response(
            Some(ElicitationResponse {
                action: ElicitationAction::Accept,
                content: None,
                meta: None,
            }),
            "approval",
        );

        assert_eq!(response, McpToolApprovalDecision::Accept);
    }
}
