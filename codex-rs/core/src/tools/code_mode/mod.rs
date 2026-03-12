mod execute_handler;
mod process;
mod protocol;
mod service;
mod wait_handler;
mod worker;

use std::sync::Arc;
use std::time::Duration;

use codex_protocol::models::FunctionCallOutputContentItem;
use serde_json::Value as JsonValue;

use crate::client_common::tools::ToolSpec;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::tools::ToolRouter;
use crate::tools::code_mode_description::augment_tool_spec_for_code_mode;
use crate::tools::code_mode_description::code_mode_tool_reference;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::context::ToolPayload;
use crate::tools::router::ToolCall;
use crate::tools::router::ToolCallSource;
use crate::tools::router::ToolRouterParams;
use crate::truncate::TruncationPolicy;
use crate::truncate::formatted_truncate_text_content_items_with_policy;
use crate::truncate::truncate_function_output_items_with_policy;
use crate::unified_exec::resolve_max_tokens;

const CODE_MODE_RUNNER_SOURCE: &str = include_str!("runner.cjs");
const CODE_MODE_BRIDGE_SOURCE: &str = include_str!("bridge.js");
const CODE_MODE_DESCRIPTION_TEMPLATE: &str = include_str!("description.md");
const CODE_MODE_WAIT_DESCRIPTION_TEMPLATE: &str = include_str!("wait_description.md");

pub(crate) const PUBLIC_TOOL_NAME: &str = "exec";
pub(crate) const WAIT_TOOL_NAME: &str = "exec_wait";
pub(crate) const DEFAULT_WAIT_YIELD_TIME_MS: u64 = 10_000;

#[derive(Clone)]
pub(super) struct ExecContext {
    pub(super) session: Arc<Session>,
    pub(super) turn: Arc<TurnContext>,
    pub(super) tracker: SharedTurnDiffTracker,
}

pub(crate) use execute_handler::CodeModeExecuteHandler;
pub(crate) use service::CodeModeService;
pub(crate) use wait_handler::CodeModeWaitHandler;

enum CodeModeSessionProgress {
    Finished(FunctionToolOutput),
    Yielded { output: FunctionToolOutput },
}

enum CodeModeExecutionStatus {
    Completed,
    Failed,
    Running(i32),
    Terminated,
}

pub(crate) fn tool_description(enabled_tool_names: &[String]) -> String {
    let enabled_list = if enabled_tool_names.is_empty() {
        "none".to_string()
    } else {
        enabled_tool_names.join(", ")
    };
    format!(
        "{}\n- Enabled nested tools: {enabled_list}.",
        CODE_MODE_DESCRIPTION_TEMPLATE.trim_end()
    )
}

pub(crate) fn wait_tool_description() -> &'static str {
    CODE_MODE_WAIT_DESCRIPTION_TEMPLATE
}

async fn handle_node_message(
    exec: &ExecContext,
    session_id: i32,
    message: protocol::NodeToHostMessage,
    poll_max_output_tokens: Option<Option<usize>>,
    started_at: std::time::Instant,
) -> Result<CodeModeSessionProgress, String> {
    match message {
        protocol::NodeToHostMessage::ToolCall { .. } => Err(protocol::unexpected_tool_call_error()),
        protocol::NodeToHostMessage::Yielded { content_items, .. } => {
            let mut delta_items = output_content_items_from_json_values(content_items)?;
            delta_items = truncate_code_mode_result(delta_items, poll_max_output_tokens.flatten());
            prepend_script_status(
                &mut delta_items,
                CodeModeExecutionStatus::Running(session_id),
                started_at.elapsed(),
            );
            Ok(CodeModeSessionProgress::Yielded {
                output: FunctionToolOutput::from_content(delta_items, Some(true)),
            })
        }
        protocol::NodeToHostMessage::Terminated { content_items, .. } => {
            let mut delta_items = output_content_items_from_json_values(content_items)?;
            delta_items = truncate_code_mode_result(delta_items, poll_max_output_tokens.flatten());
            prepend_script_status(
                &mut delta_items,
                CodeModeExecutionStatus::Terminated,
                started_at.elapsed(),
            );
            Ok(CodeModeSessionProgress::Finished(
                FunctionToolOutput::from_content(delta_items, Some(true)),
            ))
        }
        protocol::NodeToHostMessage::Result {
            content_items,
            stored_values,
            error_text,
            max_output_tokens_per_exec_call,
            ..
        } => {
            exec.session
                .services
                .code_mode_service
                .replace_stored_values(stored_values)
                .await;
            let mut delta_items = output_content_items_from_json_values(content_items)?;
            let success = error_text.is_none();
            if let Some(error_text) = error_text {
                delta_items.push(FunctionCallOutputContentItem::InputText {
                    text: format!("Script error:\n{error_text}"),
                });
            }

            let mut delta_items = truncate_code_mode_result(
                delta_items,
                poll_max_output_tokens.unwrap_or(max_output_tokens_per_exec_call),
            );
            prepend_script_status(
                &mut delta_items,
                if success {
                    CodeModeExecutionStatus::Completed
                } else {
                    CodeModeExecutionStatus::Failed
                },
                started_at.elapsed(),
            );
            Ok(CodeModeSessionProgress::Finished(
                FunctionToolOutput::from_content(delta_items, Some(success)),
            ))
        }
    }
}

fn prepend_script_status(
    content_items: &mut Vec<FunctionCallOutputContentItem>,
    status: CodeModeExecutionStatus,
    wall_time: Duration,
) {
    let wall_time_seconds = ((wall_time.as_secs_f32()) * 10.0).round() / 10.0;
    let header = format!(
        "{}\nWall time {wall_time_seconds:.1} seconds\nOutput:\n",
        match status {
            CodeModeExecutionStatus::Completed => "Script completed".to_string(),
            CodeModeExecutionStatus::Failed => "Script failed".to_string(),
            CodeModeExecutionStatus::Running(session_id) => {
                format!("Script running with session ID {session_id}")
            }
            CodeModeExecutionStatus::Terminated => "Script terminated".to_string(),
        }
    );
    content_items.insert(0, FunctionCallOutputContentItem::InputText { text: header });
}

fn truncate_code_mode_result(
    items: Vec<FunctionCallOutputContentItem>,
    max_output_tokens_per_exec_call: Option<usize>,
) -> Vec<FunctionCallOutputContentItem> {
    let max_output_tokens = resolve_max_tokens(max_output_tokens_per_exec_call);
    let policy = TruncationPolicy::Tokens(max_output_tokens);
    if items
        .iter()
        .all(|item| matches!(item, FunctionCallOutputContentItem::InputText { .. }))
    {
        let (truncated_items, _) =
            formatted_truncate_text_content_items_with_policy(&items, policy);
        return truncated_items;
    }

    truncate_function_output_items_with_policy(&items, policy)
}

fn output_content_items_from_json_values(
    content_items: Vec<JsonValue>,
) -> Result<Vec<FunctionCallOutputContentItem>, String> {
    content_items
        .into_iter()
        .enumerate()
        .map(|(index, item)| {
            serde_json::from_value(item).map_err(|err| {
                format!("invalid {PUBLIC_TOOL_NAME} content item at index {index}: {err}")
            })
        })
        .collect()
}

async fn build_enabled_tools(exec: &ExecContext) -> Vec<protocol::EnabledTool> {
    let router = build_nested_router(exec).await;
    let mut out = router
        .specs()
        .into_iter()
        .map(|spec| augment_tool_spec_for_code_mode(spec, true))
        .filter_map(enabled_tool_from_spec)
        .collect::<Vec<_>>();
    out.sort_by(|left, right| left.tool_name.cmp(&right.tool_name));
    out.dedup_by(|left, right| left.tool_name == right.tool_name);
    out
}

fn enabled_tool_from_spec(spec: ToolSpec) -> Option<protocol::EnabledTool> {
    let tool_name = spec.name().to_string();
    if tool_name == PUBLIC_TOOL_NAME || tool_name == WAIT_TOOL_NAME {
        return None;
    }

    let reference = code_mode_tool_reference(&tool_name);
    let (description, kind) = match spec {
        ToolSpec::Function(tool) => (tool.description, protocol::CodeModeToolKind::Function),
        ToolSpec::Freeform(tool) => (tool.description, protocol::CodeModeToolKind::Freeform),
        ToolSpec::LocalShell {}
        | ToolSpec::ImageGeneration { .. }
        | ToolSpec::ToolSearch { .. }
        | ToolSpec::WebSearch { .. } => {
            return None;
        }
    };

    Some(protocol::EnabledTool {
        tool_name,
        module_path: reference.module_path,
        namespace: reference.namespace,
        name: reference.tool_key,
        description,
        kind,
    })
}

async fn build_nested_router(exec: &ExecContext) -> ToolRouter {
    let nested_tools_config = exec.turn.tools_config.for_code_mode_nested_tools();
    let mcp_tools = exec
        .session
        .services
        .mcp_connection_manager
        .read()
        .await
        .list_all_tools()
        .await
        .into_iter()
        .map(|(name, tool_info)| (name, tool_info.tool))
        .collect();

    ToolRouter::from_config(
        &nested_tools_config,
        ToolRouterParams {
            mcp_tools: Some(mcp_tools),
            app_tools: None,
            discoverable_tools: None,
            dynamic_tools: exec.turn.dynamic_tools.as_slice(),
        },
    )
}

async fn call_nested_tool(
    exec: ExecContext,
    tool_name: String,
    input: Option<JsonValue>,
) -> JsonValue {
    if tool_name == PUBLIC_TOOL_NAME {
        return JsonValue::String(format!("{PUBLIC_TOOL_NAME} cannot invoke itself"));
    }

    let router = build_nested_router(&exec).await;
    let specs = router.specs();
    let payload =
        if let Some((server, tool)) = exec.session.parse_mcp_tool_name(&tool_name, &None).await {
            match serialize_function_tool_arguments(&tool_name, input) {
                Ok(raw_arguments) => ToolPayload::Mcp {
                    server,
                    tool,
                    raw_arguments,
                },
                Err(error) => return JsonValue::String(error),
            }
        } else {
            match build_nested_tool_payload(&specs, &tool_name, input) {
                Ok(payload) => payload,
                Err(error) => return JsonValue::String(error),
            }
        };

    let call = ToolCall {
        tool_name: tool_name.clone(),
        call_id: format!("{PUBLIC_TOOL_NAME}-{}", uuid::Uuid::new_v4()),
        tool_namespace: None,
        payload,
    };
    let result = router
        .dispatch_tool_call_with_code_mode_result(
            exec.session.clone(),
            exec.turn.clone(),
            exec.tracker.clone(),
            call,
            ToolCallSource::CodeMode,
        )
        .await;

    match result {
        Ok(result) => result.code_mode_result(),
        Err(error) => JsonValue::String(error.to_string()),
    }
}

fn tool_kind_for_spec(spec: &ToolSpec) -> protocol::CodeModeToolKind {
    if matches!(spec, ToolSpec::Freeform(_)) {
        protocol::CodeModeToolKind::Freeform
    } else {
        protocol::CodeModeToolKind::Function
    }
}

fn tool_kind_for_name(
    specs: &[ToolSpec],
    tool_name: &str,
) -> Result<protocol::CodeModeToolKind, String> {
    specs
        .iter()
        .find(|spec| spec.name() == tool_name)
        .map(tool_kind_for_spec)
        .ok_or_else(|| format!("tool `{tool_name}` is not enabled in {PUBLIC_TOOL_NAME}"))
}

fn build_nested_tool_payload(
    specs: &[ToolSpec],
    tool_name: &str,
    input: Option<JsonValue>,
) -> Result<ToolPayload, String> {
    let actual_kind = tool_kind_for_name(specs, tool_name)?;
    match actual_kind {
        protocol::CodeModeToolKind::Function => build_function_tool_payload(tool_name, input),
        protocol::CodeModeToolKind::Freeform => build_freeform_tool_payload(tool_name, input),
    }
}

fn build_function_tool_payload(
    tool_name: &str,
    input: Option<JsonValue>,
) -> Result<ToolPayload, String> {
    let arguments = serialize_function_tool_arguments(tool_name, input)?;
    Ok(ToolPayload::Function { arguments })
}

fn serialize_function_tool_arguments(
    tool_name: &str,
    input: Option<JsonValue>,
) -> Result<String, String> {
    match input {
        None => Ok("{}".to_string()),
        Some(JsonValue::Object(map)) => serde_json::to_string(&JsonValue::Object(map))
            .map_err(|err| format!("failed to serialize tool `{tool_name}` arguments: {err}")),
        Some(_) => Err(format!(
            "tool `{tool_name}` expects a JSON object for arguments"
        )),
    }
}

fn build_freeform_tool_payload(
    tool_name: &str,
    input: Option<JsonValue>,
) -> Result<ToolPayload, String> {
    match input {
        Some(JsonValue::String(input)) => Ok(ToolPayload::Custom { input }),
        _ => Err(format!("tool `{tool_name}` expects a string input")),
    }
}
