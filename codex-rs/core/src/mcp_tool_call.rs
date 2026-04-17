use std::collections::BTreeMap;
use std::collections::HashMap;
use std::time::Duration;
use std::time::Instant;

use codex_app_server_protocol::ConfigLayerSource;
use codex_app_server_protocol::McpElicitationObjectType;
use codex_app_server_protocol::McpElicitationSchema;
use codex_app_server_protocol::McpServerElicitationRequest;
use codex_app_server_protocol::McpServerElicitationRequestParams;
use tracing::error;

use crate::arc_monitor::ArcMonitorOutcome;
use crate::arc_monitor::monitor_action;
use crate::config::Config;
use crate::config::edit::ConfigEdit;
use crate::config::edit::ConfigEditsBuilder;
use crate::config::load_global_mcp_servers;
use crate::connectors;
use crate::guardian::GuardianApprovalRequest;
use crate::guardian::GuardianMcpAnnotations;
use crate::guardian::guardian_approval_request_to_json;
use crate::guardian::guardian_rejection_message;
use crate::guardian::guardian_timeout_message;
use crate::guardian::new_guardian_review_id;
use crate::guardian::review_approval_request;
use crate::guardian::routes_approval_to_guardian;
use crate::mcp_openai_file::rewrite_mcp_tool_arguments_for_openai_files;
use crate::mcp_tool_approval_templates::RenderedMcpToolApprovalParam;
use crate::mcp_tool_approval_templates::render_mcp_tool_approval_template;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use codex_analytics::AppInvocation;
use codex_analytics::InvocationType;
use codex_analytics::build_track_events_context;
use codex_config::types::AppToolApproval;
use codex_features::Feature;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::SandboxState;
use codex_mcp::declared_openai_file_input_param_names;
use codex_mcp::mcp_permission_prompt_is_auto_approved;
use codex_otel::sanitize_metric_tag_value;
use codex_protocol::mcp::CallToolResult;
use codex_protocol::openai_models::InputModality;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::McpInvocation;
use codex_protocol::protocol::McpToolCallBeginEvent;
use codex_protocol::protocol::McpToolCallEndEvent;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::request_user_input::RequestUserInputAnswer;
use codex_protocol::request_user_input::RequestUserInputArgs;
use codex_protocol::request_user_input::RequestUserInputQuestion;
use codex_protocol::request_user_input::RequestUserInputQuestionOption;
use codex_protocol::request_user_input::RequestUserInputResponse;
use codex_rmcp_client::ElicitationAction;
use codex_rmcp_client::ElicitationResponse;
use codex_rollout::state_db;
use codex_utils_absolute_path::AbsolutePathBuf;
use rmcp::model::ToolAnnotations;
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;
use toml_edit::value;
use tracing::Instrument;
use tracing::Span;
use tracing::field::Empty;
use url::Url;

const MCP_CALL_COUNT_METRIC: &str = "codex.mcp.call";
const MCP_CALL_DURATION_METRIC: &str = "codex.mcp.call.duration_ms";

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
    let mcp_app_resource_uri = metadata
        .as_ref()
        .and_then(|metadata| metadata.mcp_app_resource_uri.clone());
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
    let approval_mode = if server == CODEX_APPS_MCP_SERVER_NAME {
        app_tool_policy.approval
    } else {
        custom_mcp_tool_approval_mode(turn_context.as_ref(), &server, &tool_name)
    };

    if server == CODEX_APPS_MCP_SERVER_NAME && !app_tool_policy.enabled {
        let result = notify_mcp_tool_call_skip(
            sess.as_ref(),
            turn_context.as_ref(),
            &call_id,
            invocation,
            mcp_app_resource_uri.clone(),
            "MCP tool call blocked by app configuration".to_string(),
            /*already_started*/ false,
        )
        .await;
        let status = if result.is_ok() { "ok" } else { "error" };
        turn_context.session_telemetry.counter(
            MCP_CALL_COUNT_METRIC,
            /*inc*/ 1,
            &[("status", status)],
        );
        return CallToolResult::from_result(result);
    }
    let request_meta =
        build_mcp_tool_call_request_meta(turn_context.as_ref(), &server, metadata.as_ref());
    let connector_id = metadata
        .as_ref()
        .and_then(|metadata| metadata.connector_id.clone());
    let connector_name = metadata
        .as_ref()
        .and_then(|metadata| metadata.connector_name.clone());
    let server_origin = sess
        .services
        .mcp_connection_manager
        .read()
        .await
        .server_origin(&server)
        .map(str::to_string);

    let tool_call_begin_event = EventMsg::McpToolCallBegin(McpToolCallBeginEvent {
        call_id: call_id.clone(),
        invocation: invocation.clone(),
        mcp_app_resource_uri: mcp_app_resource_uri.clone(),
    });
    notify_mcp_tool_call_event(sess.as_ref(), turn_context.as_ref(), tool_call_begin_event).await;

    if let Some(decision) = maybe_request_mcp_tool_approval(
        &sess,
        turn_context,
        &call_id,
        &invocation,
        metadata.as_ref(),
        approval_mode,
    )
    .await
    {
        let (result, call_duration) = match decision {
            McpToolApprovalDecision::Accept
            | McpToolApprovalDecision::AcceptForSession
            | McpToolApprovalDecision::AcceptAndRemember => {
                maybe_mark_thread_memory_mode_polluted(sess.as_ref(), turn_context.as_ref()).await;

                let start = Instant::now();
                let result = async {
                    execute_mcp_tool_call(
                        sess.as_ref(),
                        turn_context.as_ref(),
                        &server,
                        &tool_name,
                        arguments_value.clone(),
                        metadata.as_ref(),
                        request_meta.clone(),
                    )
                    .await
                }
                .instrument(mcp_tool_call_span(
                    sess.as_ref(),
                    turn_context.as_ref(),
                    McpToolCallSpanFields {
                        server_name: &server,
                        tool_name: &tool_name,
                        call_id: &call_id,
                        server_origin: server_origin.as_deref(),
                        connector_id: connector_id.as_deref(),
                        connector_name: connector_name.as_deref(),
                    },
                ))
                .await;
                if let Err(error) = &result {
                    tracing::warn!("MCP tool call error: {error:?}");
                }
                let duration = start.elapsed();
                let tool_call_end_event = EventMsg::McpToolCallEnd(McpToolCallEndEvent {
                    call_id: call_id.clone(),
                    invocation,
                    mcp_app_resource_uri: mcp_app_resource_uri.clone(),
                    duration,
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
                (result, Some(duration))
            }
            McpToolApprovalDecision::Decline { message } => {
                let message = message.unwrap_or_else(|| "user rejected MCP tool call".to_string());
                (
                    notify_mcp_tool_call_skip(
                        sess.as_ref(),
                        turn_context.as_ref(),
                        &call_id,
                        invocation,
                        mcp_app_resource_uri.clone(),
                        message,
                        /*already_started*/ true,
                    )
                    .await,
                    None,
                )
            }
            McpToolApprovalDecision::Cancel => {
                let message = "user cancelled MCP tool call".to_string();
                (
                    notify_mcp_tool_call_skip(
                        sess.as_ref(),
                        turn_context.as_ref(),
                        &call_id,
                        invocation,
                        mcp_app_resource_uri.clone(),
                        message,
                        /*already_started*/ true,
                    )
                    .await,
                    None,
                )
            }
            McpToolApprovalDecision::BlockedBySafetyMonitor(message) => {
                (
                    notify_mcp_tool_call_skip(
                        sess.as_ref(),
                        turn_context.as_ref(),
                        &call_id,
                        invocation,
                        mcp_app_resource_uri.clone(),
                        message,
                        /*already_started*/ true,
                    )
                    .await,
                    None,
                )
            }
        };

        let status = if result.is_ok() { "ok" } else { "error" };
        emit_mcp_call_metrics(
            turn_context.as_ref(),
            status,
            &tool_name,
            connector_id.as_deref(),
            connector_name.as_deref(),
            call_duration,
        );

        return CallToolResult::from_result(result);
    }

    maybe_mark_thread_memory_mode_polluted(sess.as_ref(), turn_context.as_ref()).await;

    let start = Instant::now();
    let result = async {
        execute_mcp_tool_call(
            sess.as_ref(),
            turn_context.as_ref(),
            &server,
            &tool_name,
            arguments_value.clone(),
            metadata.as_ref(),
            request_meta,
        )
        .await
    }
    .instrument(mcp_tool_call_span(
        sess.as_ref(),
        turn_context.as_ref(),
        McpToolCallSpanFields {
            server_name: &server,
            tool_name: &tool_name,
            call_id: &call_id,
            server_origin: server_origin.as_deref(),
            connector_id: connector_id.as_deref(),
            connector_name: connector_name.as_deref(),
        },
    ))
    .await;
    if let Err(error) = &result {
        tracing::warn!("MCP tool call error: {error:?}");
    }
    let duration = start.elapsed();
    let tool_call_end_event = EventMsg::McpToolCallEnd(McpToolCallEndEvent {
        call_id: call_id.clone(),
        invocation,
        mcp_app_resource_uri,
        duration,
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
    emit_mcp_call_metrics(
        turn_context.as_ref(),
        status,
        &tool_name,
        connector_id.as_deref(),
        connector_name.as_deref(),
        Some(duration),
    );

    CallToolResult::from_result(result)
}

fn emit_mcp_call_metrics(
    turn_context: &TurnContext,
    status: &str,
    tool_name: &str,
    connector_id: Option<&str>,
    connector_name: Option<&str>,
    duration: Option<Duration>,
) {
    let tags = mcp_call_metric_tags(status, tool_name, connector_id, connector_name);
    let tag_refs: Vec<(&str, &str)> = tags
        .iter()
        .map(|(key, value)| (*key, value.as_str()))
        .collect();
    turn_context
        .session_telemetry
        .counter(MCP_CALL_COUNT_METRIC, /*inc*/ 1, &tag_refs);
    if let Some(duration) = duration {
        turn_context.session_telemetry.record_duration(
            MCP_CALL_DURATION_METRIC,
            duration,
            &tag_refs,
        );
    }
}

fn mcp_call_metric_tags(
    status: &str,
    tool_name: &str,
    connector_id: Option<&str>,
    connector_name: Option<&str>,
) -> Vec<(&'static str, String)> {
    let mut tags = vec![
        ("status", sanitize_metric_tag_value(status)),
        ("tool", sanitize_metric_tag_value(tool_name)),
    ];
    if let Some(connector_id) = connector_id.filter(|connector_id| !connector_id.is_empty()) {
        tags.push(("connector_id", sanitize_metric_tag_value(connector_id)));
    }
    if let Some(connector_name) = connector_name.filter(|connector_name| !connector_name.is_empty())
    {
        tags.push(("connector_name", sanitize_metric_tag_value(connector_name)));
    }
    tags
}

fn mcp_tool_call_span(
    session: &Session,
    turn_context: &TurnContext,
    fields: McpToolCallSpanFields<'_>,
) -> Span {
    let transport = match fields.server_origin {
        Some("stdio") => "stdio",
        Some(_) => "streamable_http",
        None => "",
    };
    let span = tracing::info_span!(
        "mcp.tools.call",
        otel.kind = "client",
        rpc.system = "jsonrpc",
        rpc.method = "tools/call",
        mcp.server.name = fields.server_name,
        mcp.server.origin = fields.server_origin.unwrap_or(""),
        mcp.transport = transport,
        mcp.connector.id = fields.connector_id.unwrap_or(""),
        mcp.connector.name = fields.connector_name.unwrap_or(""),
        tool.name = fields.tool_name,
        tool.call_id = fields.call_id,
        conversation.id = %session.conversation_id,
        session.id = %session.conversation_id,
        turn.id = turn_context.sub_id.as_str(),
        server.address = Empty,
        server.port = Empty,
    );
    record_server_fields(&span, fields.server_origin);
    span
}

struct McpToolCallSpanFields<'a> {
    server_name: &'a str,
    tool_name: &'a str,
    call_id: &'a str,
    server_origin: Option<&'a str>,
    connector_id: Option<&'a str>,
    connector_name: Option<&'a str>,
}

fn record_server_fields(span: &Span, url: Option<&str>) {
    let Some(url) = url else {
        return;
    };
    let Ok(parsed) = Url::parse(url) else {
        return;
    };
    if let Some(host) = parsed.host_str() {
        span.record("server.address", host);
    }
    if let Some(port) = parsed.port_or_known_default() {
        span.record("server.port", port as i64);
    }
}

async fn execute_mcp_tool_call(
    sess: &Session,
    turn_context: &TurnContext,
    server: &str,
    tool_name: &str,
    arguments_value: Option<serde_json::Value>,
    metadata: Option<&McpToolApprovalMetadata>,
    request_meta: Option<serde_json::Value>,
) -> Result<CallToolResult, String> {
    let rewritten_arguments = rewrite_mcp_tool_arguments_for_openai_files(
        sess,
        turn_context,
        arguments_value,
        metadata.and_then(|metadata| metadata.openai_file_input_params.as_deref()),
    )
    .await?;
    let request_meta =
        augment_mcp_tool_request_meta_with_sandbox_state(sess, turn_context, server, request_meta)
            .await
            .map_err(|e| format!("failed to build MCP tool request metadata: {e:#}"))?;
    let result = sess
        .call_tool(server, tool_name, rewritten_arguments, request_meta)
        .await
        .map_err(|e| format!("tool call error: {e:?}"))?;
    sanitize_mcp_tool_result_for_model(
        turn_context
            .model_info
            .input_modalities
            .contains(&InputModality::Image),
        Ok(result),
    )
}

async fn augment_mcp_tool_request_meta_with_sandbox_state(
    sess: &Session,
    turn_context: &TurnContext,
    server: &str,
    mut meta: Option<serde_json::Value>,
) -> anyhow::Result<Option<serde_json::Value>> {
    let supports_sandbox_state_meta = sess
        .services
        .mcp_connection_manager
        .read()
        .await
        .server_supports_sandbox_state_meta_capability(server)
        .await
        .unwrap_or(false);
    if !supports_sandbox_state_meta {
        return Ok(meta);
    }

    let sandbox_state = serde_json::to_value(SandboxState {
        permission_profile: Some(turn_context.permission_profile()),
        sandbox_policy: turn_context.sandbox_policy.get().clone(),
        codex_linux_sandbox_exe: turn_context.codex_linux_sandbox_exe.clone(),
        sandbox_cwd: turn_context.cwd.to_path_buf(),
        use_legacy_landlock: turn_context.features.use_legacy_landlock(),
    })?;

    match meta.as_mut() {
        Some(serde_json::Value::Object(map)) => {
            map.insert(
                codex_mcp::MCP_SANDBOX_STATE_META_CAPABILITY.to_string(),
                sandbox_state,
            );
        }
        Some(_) => {}
        None => {
            let mut map = serde_json::Map::new();
            map.insert(
                codex_mcp::MCP_SANDBOX_STATE_META_CAPABILITY.to_string(),
                sandbox_state,
            );
            meta = Some(serde_json::Value::Object(map));
        }
    }

    Ok(meta)
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
    Decline { message: Option<String> },
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
    mcp_app_resource_uri: Option<String>,
    codex_apps_meta: Option<serde_json::Map<String, serde_json::Value>>,
    openai_file_input_params: Option<Vec<String>>,
}

const MCP_TOOL_CODEX_APPS_META_KEY: &str = "_codex_apps";
const MCP_TOOL_OPENAI_OUTPUT_TEMPLATE_META_KEY: &str = "openai/outputTemplate";
const MCP_TOOL_UI_RESOURCE_URI_META_KEY: &str = "ui/resourceUri";

fn custom_mcp_tool_approval_mode(
    turn_context: &TurnContext,
    server: &str,
    tool_name: &str,
) -> AppToolApproval {
    turn_context
        .config
        .config_layer_stack
        .effective_config()
        .as_table()
        .and_then(|table| table.get("mcp_servers"))
        .cloned()
        .and_then(|value| {
            HashMap::<String, codex_config::types::McpServerConfig>::deserialize(value).ok()
        })
        .and_then(|servers| {
            let server_config = servers.get(server)?;
            server_config
                .tools
                .get(tool_name)
                .and_then(|tool| tool.approval_mode)
                .or(server_config.default_tools_approval_mode)
        })
        .unwrap_or_default()
}

fn build_mcp_tool_call_request_meta(
    turn_context: &TurnContext,
    server: &str,
    metadata: Option<&McpToolApprovalMetadata>,
) -> Option<serde_json::Value> {
    let mut request_meta = serde_json::Map::new();

    if let Some(turn_metadata) = turn_context.turn_metadata_state.current_meta_value() {
        request_meta.insert(
            crate::X_CODEX_TURN_METADATA_HEADER.to_string(),
            turn_metadata,
        );
    }

    if server == CODEX_APPS_MCP_SERVER_NAME
        && let Some(codex_apps_meta) =
            metadata.and_then(|metadata| metadata.codex_apps_meta.clone())
    {
        request_meta.insert(
            MCP_TOOL_CODEX_APPS_META_KEY.to_string(),
            serde_json::Value::Object(codex_apps_meta),
        );
    }

    (!request_meta.is_empty()).then_some(serde_json::Value::Object(request_meta))
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
// Internal-only token used when guardian auto-reviews delegated MCP approvals on the
// RequestUserInput compatibility path. That legacy MCP prompt has allow/cancel labels but no
// real "Decline" answer, so this lets guardian denials round-trip distinctly from user cancel.
// This is not a user-facing option.
pub(crate) const MCP_TOOL_APPROVAL_DECLINE_SYNTHETIC: &str = "__codex_mcp_decline__";
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
const MCP_TOOL_CALL_ARC_MONITOR_CALLSITE_DEFAULT: &str = "mcp_tool_call__default";
const MCP_TOOL_CALL_ARC_MONITOR_CALLSITE_ALWAYS_ALLOW: &str = "mcp_tool_call__always_allow";

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
    if mcp_permission_prompt_is_auto_approved(
        turn_context.approval_policy.value(),
        turn_context.sandbox_policy.get(),
    ) {
        return None;
    }

    let annotations = metadata.and_then(|metadata| metadata.annotations.as_ref());
    let approval_required = requires_mcp_tool_approval(annotations);
    if !approval_required && approval_mode != AppToolApproval::Prompt {
        return None;
    }

    let mut monitor_reason = None;
    let auto_approved_by_policy = approval_mode == AppToolApproval::Approve;

    if auto_approved_by_policy {
        match maybe_monitor_auto_approved_mcp_tool_call(
            sess,
            turn_context,
            invocation,
            metadata,
            approval_mode,
        )
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
        let review_id = new_guardian_review_id();
        let decision = review_approval_request(
            sess,
            turn_context,
            review_id.clone(),
            build_guardian_mcp_tool_review_request(call_id, invocation, metadata),
            monitor_reason.clone(),
        )
        .await;
        let decision = mcp_tool_approval_decision_from_guardian(sess, &review_id, decision).await;
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
    approval_mode: AppToolApproval,
) -> ArcMonitorOutcome {
    let action = prepare_arc_request_action(invocation, metadata);
    monitor_action(
        sess,
        turn_context,
        action,
        mcp_tool_approval_callsite_mode(approval_mode, turn_context),
    )
    .await
}

fn prepare_arc_request_action(
    invocation: &McpInvocation,
    metadata: Option<&McpToolApprovalMetadata>,
) -> serde_json::Value {
    let request = build_guardian_mcp_tool_review_request("arc-monitor", invocation, metadata);
    match guardian_approval_request_to_json(&request) {
        Ok(action) => action,
        Err(error) => {
            error!(error = %error, "failed to serialize guardian MCP approval request for ARC");
            serde_json::Value::Null
        }
    }
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
    session_mcp_tool_approval_key(invocation, metadata, approval_mode)
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

async fn mcp_tool_approval_decision_from_guardian(
    sess: &Session,
    review_id: &str,
    decision: ReviewDecision,
) -> McpToolApprovalDecision {
    match decision {
        ReviewDecision::Approved
        | ReviewDecision::ApprovedExecpolicyAmendment { .. }
        | ReviewDecision::NetworkPolicyAmendment { .. } => McpToolApprovalDecision::Accept,
        ReviewDecision::ApprovedForSession => McpToolApprovalDecision::AcceptForSession,
        ReviewDecision::Denied => McpToolApprovalDecision::Decline {
            message: Some(guardian_rejection_message(sess, review_id).await),
        },
        ReviewDecision::TimedOut => McpToolApprovalDecision::Decline {
            message: Some(guardian_timeout_message()),
        },
        ReviewDecision::Abort => McpToolApprovalDecision::Decline { message: None },
    }
}

fn mcp_tool_approval_callsite_mode(
    approval_mode: AppToolApproval,
    _turn_context: &TurnContext,
) -> &'static str {
    match approval_mode {
        AppToolApproval::Approve => MCP_TOOL_CALL_ARC_MONITOR_CALLSITE_ALWAYS_ALLOW,
        AppToolApproval::Auto | AppToolApproval::Prompt => {
            MCP_TOOL_CALL_ARC_MONITOR_CALLSITE_DEFAULT
        }
    }
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
        mcp_app_resource_uri: get_mcp_app_resource_uri(tool_info.tool.meta.as_deref()),
        codex_apps_meta: tool_info
            .tool
            .meta
            .as_ref()
            .and_then(|meta| meta.get(MCP_TOOL_CODEX_APPS_META_KEY))
            .and_then(serde_json::Value::as_object)
            .cloned(),
        openai_file_input_params: Some(declared_openai_file_input_param_names(
            tool_info.tool.meta.as_deref(),
        ))
        .filter(|params| !params.is_empty()),
    })
}

fn get_mcp_app_resource_uri(
    meta: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Option<String> {
    meta.and_then(|meta| {
        meta.get("ui")
            .and_then(serde_json::Value::as_object)
            .and_then(|ui| ui.get("resourceUri"))
            .and_then(serde_json::Value::as_str)
            .or_else(|| {
                meta.get(MCP_TOOL_UI_RESOURCE_URI_META_KEY)
                    .and_then(serde_json::Value::as_str)
            })
            .or_else(|| {
                meta.get(MCP_TOOL_OPENAI_OUTPUT_TEMPLATE_META_KEY)
                    .and_then(serde_json::Value::as_str)
            })
            .map(str::to_string)
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
                display_name: name.clone(),
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
        ElicitationAction::Decline => McpToolApprovalDecision::Decline { message: None },
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
        .any(|answer| answer == MCP_TOOL_APPROVAL_DECLINE_SYNTHETIC)
    {
        McpToolApprovalDecision::Decline { message: None }
    } else if answers
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
        | McpToolApprovalDecision::Decline { .. }
        | McpToolApprovalDecision::Cancel
        | McpToolApprovalDecision::BlockedBySafetyMonitor(_) => {}
    }
}

async fn maybe_persist_mcp_tool_approval(
    sess: &Session,
    turn_context: &TurnContext,
    key: McpToolApprovalKey,
) {
    let tool_name = key.tool_name.clone();

    let persist_result = if key.server == CODEX_APPS_MCP_SERVER_NAME {
        let Some(connector_id) = key.connector_id.clone() else {
            remember_mcp_tool_approval(sess, key).await;
            return;
        };
        persist_codex_app_tool_approval(&turn_context.config.codex_home, &connector_id, &tool_name)
            .await
    } else {
        persist_custom_mcp_tool_approval(&turn_context.config, &key.server, &tool_name).await
    };

    if let Err(err) = persist_result {
        error!(
            error = %err,
            server = key.server,
            tool_name,
            "failed to persist MCP tool approval"
        );
        remember_mcp_tool_approval(sess, key).await;
        return;
    }

    sess.reload_user_config_layer().await;
    remember_mcp_tool_approval(sess, key).await;
}

async fn persist_codex_app_tool_approval(
    codex_home: &AbsolutePathBuf,
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

async fn persist_custom_mcp_tool_approval(
    config: &Config,
    server: &str,
    tool_name: &str,
) -> anyhow::Result<()> {
    let config_folder = if let Some(project_config_folder) =
        project_mcp_tool_approval_config_folder(config, server)
    {
        project_config_folder
    } else {
        let servers = load_global_mcp_servers(&config.codex_home).await?;
        if !servers.contains_key(server) {
            anyhow::bail!("MCP server `{server}` is not configured in config.toml");
        }
        config.codex_home.clone()
    };

    ConfigEditsBuilder::new(&config_folder)
        .with_edits([ConfigEdit::SetPath {
            segments: vec![
                "mcp_servers".to_string(),
                server.to_string(),
                "tools".to_string(),
                tool_name.to_string(),
                "approval_mode".to_string(),
            ],
            value: value("approve"),
        }])
        .apply()
        .await
}

fn project_mcp_tool_approval_config_folder(
    config: &Config,
    server: &str,
) -> Option<AbsolutePathBuf> {
    config
        .config_layer_stack
        .layers_high_to_low()
        .into_iter()
        .find_map(|layer| {
            if !matches!(layer.name, ConfigLayerSource::Project { .. }) {
                return None;
            }

            let servers = layer
                .config
                .as_table()
                .and_then(|table| table.get("mcp_servers"))
                .cloned()
                .and_then(|value| {
                    HashMap::<String, codex_config::types::McpServerConfig>::deserialize(value).ok()
                })?;
            if servers.contains_key(server) {
                layer.config_folder()
            } else {
                None
            }
        })
}

fn requires_mcp_tool_approval(annotations: Option<&ToolAnnotations>) -> bool {
    let destructive_hint = annotations.and_then(|annotations| annotations.destructive_hint);
    if destructive_hint == Some(true) {
        return true;
    }

    let read_only_hint = annotations
        .and_then(|annotations| annotations.read_only_hint)
        .unwrap_or(false);
    if read_only_hint {
        return false;
    }

    destructive_hint.unwrap_or(true)
        || annotations
            .and_then(|annotations| annotations.open_world_hint)
            .unwrap_or(true)
}

async fn notify_mcp_tool_call_skip(
    sess: &Session,
    turn_context: &TurnContext,
    call_id: &str,
    invocation: McpInvocation,
    mcp_app_resource_uri: Option<String>,
    message: String,
    already_started: bool,
) -> Result<CallToolResult, String> {
    if !already_started {
        let tool_call_begin_event = EventMsg::McpToolCallBegin(McpToolCallBeginEvent {
            call_id: call_id.to_string(),
            invocation: invocation.clone(),
            mcp_app_resource_uri: mcp_app_resource_uri.clone(),
        });
        notify_mcp_tool_call_event(sess, turn_context, tool_call_begin_event).await;
    }

    let tool_call_end_event = EventMsg::McpToolCallEnd(McpToolCallEndEvent {
        call_id: call_id.to_string(),
        invocation,
        mcp_app_resource_uri,
        duration: Duration::ZERO,
        result: Err(message.clone()),
    });
    notify_mcp_tool_call_event(sess, turn_context, tool_call_end_event).await;
    Err(message)
}

#[cfg(test)]
#[path = "mcp_tool_call_tests.rs"]
mod tests;
