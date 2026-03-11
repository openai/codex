use std::sync::Arc;
use std::time::Duration;

use crate::client_common::tools::ToolSpec;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::config::Config;
use crate::features::Feature;
use crate::function_tool::FunctionCallError;
use crate::tools::ToolRouter;
use crate::tools::code_mode_description::augment_tool_spec_for_code_mode;
use crate::tools::code_mode_description::code_mode_tool_reference;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::context::ToolPayload;
use crate::tools::router::ToolCall;
use crate::tools::router::ToolCallSource;
use crate::truncate::TruncationPolicy;
use crate::truncate::formatted_truncate_text_content_items_with_policy;
use crate::truncate::truncate_function_output_items_with_policy;
use crate::unified_exec::resolve_max_tokens;
use codex_code_mode::EnabledTool;
use codex_code_mode::ToolKind as CodeModeToolKind;
use codex_code_mode::execute as execute_code_mode;
use codex_protocol::models::FunctionCallOutputContentItem;
use serde_json::Value as JsonValue;
use tokio::runtime::Handle;
use tokio::runtime::RuntimeFlavor;

pub(crate) const PUBLIC_TOOL_NAME: &str = "exec";

#[derive(Clone)]
struct ExecContext {
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    tracker: SharedTurnDiffTracker,
}
pub(crate) fn instructions(config: &Config) -> Option<String> {
    if !config.features.enabled(Feature::CodeMode) || !codex_code_mode::is_supported() {
        return None;
    }

    let mut section = String::from("## Exec\n");
    section.push_str(&format!(
        "- Use `{PUBLIC_TOOL_NAME}` for JavaScript execution in an embedded V8 runtime.\n",
    ));
    section.push_str(&format!(
        "- `{PUBLIC_TOOL_NAME}` is a freeform/custom tool. Direct `{PUBLIC_TOOL_NAME}` calls must send raw JavaScript tool input. Do not wrap code in JSON, quotes, or markdown code fences.\n",
    ));
    section.push_str(&format!(
        "- Direct tool calls remain available while `{PUBLIC_TOOL_NAME}` is enabled.\n",
    ));
    section.push_str("- Import nested tools from `tools.js`, for example `import { exec_command } from \"tools.js\"`, `import { tools } from \"tools.js\"`, or `import { ALL_TOOLS } from \"tools.js\"` to inspect the available `{ module, name, description }` entries. Namespaced tools are also available from `tools/<namespace...>.js`; MCP tools use `tools/mcp/<server>.js`, for example `import { append_notebook_logs_chart } from \"tools/mcp/ologs.js\"`. `tools[name]` and identifier wrappers like `await exec_command(args)` remain available for compatibility. Nested tool calls resolve to their code-mode result values.\n");
    section.push_str(&format!(
        "- Import `{{ output_text, output_image, set_max_output_tokens_per_exec_call, store, load }}` from `@openai/code_mode` (or `\"openai/code_mode\"`). `output_text(value)` surfaces text back to the model and stringifies non-string objects with `JSON.stringify(...)` when possible. `output_image(imageUrl)` appends an `input_image` content item for `http(s)` or `data:` URLs. `store(key, value)` persists JSON-serializable values across `{PUBLIC_TOOL_NAME}` calls in the current session, and `load(key)` returns a cloned stored value or `undefined`. `set_max_output_tokens_per_exec_call(value)` sets the token budget used to truncate the final Rust-side result of the current `{PUBLIC_TOOL_NAME}` execution; the default is `10000`. This guards the overall `{PUBLIC_TOOL_NAME}` output, not individual nested tool invocations. The returned content starts with a separate `Script completed` or `Script failed` text item that includes wall time. When truncation happens, the final text may include `Total output lines:` and the usual `…N tokens truncated…` marker.\n",
    ));
    section.push_str(
        "- Function tools require JSON object arguments. Freeform tools require raw strings.\n",
    );
    section.push_str("- `add_content(value)` remains available for compatibility. It is synchronous and accepts a content item, an array of content items, or a string. Structured nested-tool results should be converted to text first, for example with `JSON.stringify(...)`.\n");
    section
        .push_str("- Only content passed to `output_text(...)`, `output_image(...)`, or `add_content(value)` is surfaced back to the model.");
    Some(section)
}

pub(crate) async fn execute(
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    tracker: SharedTurnDiffTracker,
    code: String,
) -> Result<FunctionToolOutput, FunctionCallError> {
    let exec = ExecContext {
        session,
        turn,
        tracker,
    };
    let enabled_tools = build_enabled_tools(&exec).await;
    let stored_values = exec.session.services.code_mode_store.stored_values().await;
    let started_at = std::time::Instant::now();
    let callback_exec = exec.clone();
    let result = execute_code_mode(
        code,
        enabled_tools,
        stored_values,
        Box::new(move |tool_name, input| run_tool_call(&callback_exec, tool_name, input)),
    )
    .map_err(FunctionCallError::RespondToModel)?;
    exec.session
        .services
        .code_mode_store
        .replace_stored_values(result.stored_values)
        .await;
    let mut items = output_content_items_from_json_values(result.content_items)
        .map_err(FunctionCallError::RespondToModel)?;
    if !result.success {
        let error_text = result
            .error_text
            .unwrap_or_else(|| "JavaScript execution failed".to_string());
        items.push(FunctionCallOutputContentItem::InputText {
            text: format!("Script error:\n{error_text}"),
        });
    }
    let mut items = truncate_code_mode_result(items, Some(result.max_output_tokens_per_exec_call));
    prepend_script_status(&mut items, result.success, started_at.elapsed());
    Ok(FunctionToolOutput::from_content(
        items,
        Some(result.success),
    ))
}

fn prepend_script_status(
    content_items: &mut Vec<FunctionCallOutputContentItem>,
    success: bool,
    wall_time: Duration,
) {
    let wall_time_seconds = ((wall_time.as_secs_f32()) * 10.0).round() / 10.0;
    let header = format!(
        "{}\nWall time {wall_time_seconds:.1} seconds\nOutput:\n",
        if success {
            "Script completed"
        } else {
            "Script failed"
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

async fn build_enabled_tools(exec: &ExecContext) -> Vec<EnabledTool> {
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

fn enabled_tool_from_spec(spec: ToolSpec) -> Option<EnabledTool> {
    let tool_name = spec.name().to_string();
    if tool_name == PUBLIC_TOOL_NAME {
        return None;
    }

    let reference = code_mode_tool_reference(&tool_name);

    let (description, kind) = match spec {
        ToolSpec::Function(tool) => (tool.description, CodeModeToolKind::Function),
        ToolSpec::Freeform(tool) => (tool.description, CodeModeToolKind::Freeform),
        ToolSpec::LocalShell {} | ToolSpec::ImageGeneration { .. } | ToolSpec::WebSearch { .. } => {
            return None;
        }
    };

    Some(EnabledTool {
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
        Some(mcp_tools),
        None,
        exec.turn.dynamic_tools.as_slice(),
    )
}

fn run_tool_call(
    exec: &ExecContext,
    tool_name: String,
    input: Option<JsonValue>,
) -> Result<JsonValue, String> {
    match Handle::current().runtime_flavor() {
        RuntimeFlavor::MultiThread => tokio::task::block_in_place(|| {
            Handle::current().block_on(call_nested_tool(exec.clone(), tool_name, input))
        }),
        RuntimeFlavor::CurrentThread => Err(format!(
            "{PUBLIC_TOOL_NAME} tool calls require a multi-thread Tokio runtime"
        )),
        _ => Err(format!(
            "{PUBLIC_TOOL_NAME} tool calls require a supported Tokio runtime"
        )),
    }
}

async fn call_nested_tool(
    exec: ExecContext,
    tool_name: String,
    input: Option<JsonValue>,
) -> Result<JsonValue, String> {
    if tool_name == PUBLIC_TOOL_NAME {
        return Ok(JsonValue::String(format!(
            "{PUBLIC_TOOL_NAME} cannot invoke itself"
        )));
    }

    let router = build_nested_router(&exec).await;
    let specs = router.specs();
    let payload = if let Some((server, tool)) = exec.session.parse_mcp_tool_name(&tool_name).await {
        ToolPayload::Mcp {
            server,
            tool,
            raw_arguments: serialize_function_tool_arguments(&tool_name, input)?,
        }
    } else {
        build_nested_tool_payload(&specs, &tool_name, input)?
    };

    let call = ToolCall {
        tool_name: tool_name.clone(),
        call_id: format!("{PUBLIC_TOOL_NAME}-{}", uuid::Uuid::new_v4()),
        payload,
    };
    let result = router
        .dispatch_tool_call_with_code_mode_result(
            Arc::clone(&exec.session),
            Arc::clone(&exec.turn),
            Arc::clone(&exec.tracker),
            call,
            ToolCallSource::CodeMode,
        )
        .await
        .map_err(|error| error.to_string())?;

    Ok(result.code_mode_result())
}

fn tool_kind_for_spec(spec: &ToolSpec) -> CodeModeToolKind {
    if matches!(spec, ToolSpec::Freeform(_)) {
        CodeModeToolKind::Freeform
    } else {
        CodeModeToolKind::Function
    }
}

fn tool_kind_for_name(specs: &[ToolSpec], tool_name: &str) -> Result<CodeModeToolKind, String> {
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
    match tool_kind_for_name(specs, tool_name)? {
        CodeModeToolKind::Function => build_function_tool_payload(tool_name, input),
        CodeModeToolKind::Freeform => build_freeform_tool_payload(tool_name, input),
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
