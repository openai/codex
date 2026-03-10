use std::process::ExitStatus;
use std::sync::Arc;

use crate::client_common::tools::ToolSpec;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::config::Config;
use crate::exec_env::create_env;
use crate::features::Feature;
use crate::function_tool::FunctionCallError;
use crate::tools::ToolRouter;
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::context::ToolPayload;
use crate::tools::js_repl::resolve_compatible_node;
use crate::tools::router::ToolCall;
use crate::tools::router::ToolCallSource;
use codex_protocol::models::FunctionCallOutputContentItem;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;

const CODE_MODE_RUNNER_SOURCE: &str = include_str!("code_mode_runner.cjs");
const CODE_MODE_BRIDGE_SOURCE: &str = include_str!("code_mode_bridge.js");

#[derive(Clone)]
struct ExecContext {
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    tracker: SharedTurnDiffTracker,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum CodeModeToolKind {
    Function,
    Freeform,
}

#[derive(Clone, Debug, Serialize)]
struct EnabledTool {
    name: String,
    kind: CodeModeToolKind,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum HostToNodeMessage {
    Init {
        enabled_tools: Vec<EnabledTool>,
        source: String,
    },
    Response {
        id: String,
        code_mode_result: JsonValue,
    },
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum NodeToHostMessage {
    ToolCall {
        id: String,
        name: String,
        #[serde(default)]
        input: Option<JsonValue>,
    },
    Result {
        content_items: Vec<JsonValue>,
    },
}

pub(crate) fn instructions(config: &Config) -> Option<String> {
    if !config.features.enabled(Feature::CodeMode) {
        return None;
    }

    let mut section = String::from("## Code Mode\n");
    section.push_str(
        "- Use `code_mode` for JavaScript execution in a Node-backed `node:vm` context.\n",
    );
    section.push_str("- `code_mode` is a freeform/custom tool. Direct `code_mode` calls must send raw JavaScript tool input. Do not wrap code in JSON, quotes, or markdown code fences.\n");
    section.push_str("- Direct tool calls remain available while `code_mode` is enabled.\n");
    section.push_str("- `code_mode` uses the same Node runtime resolution as `js_repl`. If needed, point `js_repl_node_path` at the Node binary you want Codex to use.\n");
    section.push_str("- Import nested tools from `tools.js`, for example `import { exec_command } from \"tools.js\"` or `import { tools } from \"tools.js\"`. `tools[name]` and identifier wrappers like `await exec_command(args)` remain available for compatibility. Nested tool calls resolve to their code-mode result values.\n");
    section.push_str(
        "- Function tools require JSON object arguments. Freeform tools require raw strings.\n",
    );
    section.push_str("- `add_content(value)` is synchronous. It accepts a content item, an array of content items, or a string. Structured nested-tool results should be converted to text first, for example with `JSON.stringify(...)`.\n");
    section
        .push_str("- Only content passed to `add_content(value)` is surfaced back to the model.");
    Some(section)
}

pub(crate) async fn execute(
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    tracker: SharedTurnDiffTracker,
    code: String,
) -> Result<Vec<FunctionCallOutputContentItem>, FunctionCallError> {
    let exec = ExecContext {
        session,
        turn,
        tracker,
    };
    let enabled_tools = build_enabled_tools(&exec);
    let source = build_source(&code, &enabled_tools).map_err(FunctionCallError::RespondToModel)?;
    execute_node(exec, source, enabled_tools)
        .await
        .map_err(FunctionCallError::RespondToModel)
}

async fn execute_node(
    exec: ExecContext,
    source: String,
    enabled_tools: Vec<EnabledTool>,
) -> Result<Vec<FunctionCallOutputContentItem>, String> {
    let node_path = resolve_compatible_node(exec.turn.config.js_repl_node_path.as_deref()).await?;

    let env = create_env(&exec.turn.shell_environment_policy, None);
    let mut cmd = tokio::process::Command::new(&node_path);
    cmd.arg("--experimental-vm-modules");
    cmd.arg("--eval");
    cmd.arg(CODE_MODE_RUNNER_SOURCE);
    cmd.current_dir(&exec.turn.cwd);
    cmd.env_clear();
    cmd.envs(env);
    cmd.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);

    let mut child = cmd
        .spawn()
        .map_err(|err| format!("failed to start code_mode Node runtime: {err}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "code_mode runner missing stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "code_mode runner missing stderr".to_string())?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "code_mode runner missing stdin".to_string())?;

    let stderr_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr);
        let mut buf = Vec::new();
        let _ = reader.read_to_end(&mut buf).await;
        String::from_utf8_lossy(&buf).trim().to_string()
    });

    write_message(
        &mut stdin,
        &HostToNodeMessage::Init {
            enabled_tools: enabled_tools.clone(),
            source,
        },
    )
    .await?;

    let mut stdout_lines = BufReader::new(stdout).lines();
    let mut final_content_items = None;
    while let Some(line) = stdout_lines
        .next_line()
        .await
        .map_err(|err| format!("failed to read code_mode runner stdout: {err}"))?
    {
        if line.trim().is_empty() {
            continue;
        }
        let message: NodeToHostMessage = serde_json::from_str(&line)
            .map_err(|err| format!("invalid code_mode runner message: {err}; line={line}"))?;
        match message {
            NodeToHostMessage::ToolCall { id, name, input } => {
                let response = HostToNodeMessage::Response {
                    id,
                    code_mode_result: call_nested_tool(exec.clone(), name, input).await,
                };
                write_message(&mut stdin, &response).await?;
            }
            NodeToHostMessage::Result { content_items } => {
                final_content_items = Some(output_content_items_from_json_values(content_items)?);
                break;
            }
        }
    }

    drop(stdin);

    let status = child
        .wait()
        .await
        .map_err(|err| format!("failed to wait for code_mode runner: {err}"))?;
    let stderr = stderr_task
        .await
        .map_err(|err| format!("failed to collect code_mode stderr: {err}"))?;

    match final_content_items {
        Some(content_items) if status.success() => Ok(content_items),
        Some(_) => Err(format_runner_failure(
            "code_mode execution failed",
            status,
            &stderr,
        )),
        None => Err(format_runner_failure(
            "code_mode runner exited without returning a result",
            status,
            &stderr,
        )),
    }
}

async fn write_message(
    stdin: &mut tokio::process::ChildStdin,
    message: &HostToNodeMessage,
) -> Result<(), String> {
    let line = serde_json::to_string(message)
        .map_err(|err| format!("failed to serialize code_mode message: {err}"))?;
    stdin
        .write_all(line.as_bytes())
        .await
        .map_err(|err| format!("failed to write code_mode message: {err}"))?;
    stdin
        .write_all(b"\n")
        .await
        .map_err(|err| format!("failed to write code_mode message newline: {err}"))?;
    stdin
        .flush()
        .await
        .map_err(|err| format!("failed to flush code_mode message: {err}"))
}

fn append_stderr(message: String, stderr: &str) -> String {
    if stderr.trim().is_empty() {
        return message;
    }
    format!("{message}\n\nnode stderr:\n{stderr}")
}

fn format_runner_failure(message: &str, status: ExitStatus, stderr: &str) -> String {
    append_stderr(format!("{message} (status {status})"), stderr)
}

fn build_source(user_code: &str, enabled_tools: &[EnabledTool]) -> Result<String, String> {
    let enabled_tools_json = serde_json::to_string(enabled_tools)
        .map_err(|err| format!("failed to serialize enabled tools: {err}"))?;
    Ok(CODE_MODE_BRIDGE_SOURCE
        .replace(
            "__CODE_MODE_ENABLED_TOOLS_PLACEHOLDER__",
            &enabled_tools_json,
        )
        .replace("__CODE_MODE_USER_CODE_PLACEHOLDER__", user_code))
}

fn build_enabled_tools(exec: &ExecContext) -> Vec<EnabledTool> {
    let nested_tools_config = exec.turn.tools_config.for_code_mode_nested_tools();
    let router = ToolRouter::from_config(
        &nested_tools_config,
        None,
        None,
        exec.turn.dynamic_tools.as_slice(),
    );
    let mut out = router
        .specs()
        .into_iter()
        .map(|spec| EnabledTool {
            name: spec.name().to_string(),
            kind: tool_kind_for_spec(&spec),
        })
        .filter(|tool| tool.name != "code_mode")
        .collect::<Vec<_>>();
    out.sort_by(|left, right| left.name.cmp(&right.name));
    out.dedup_by(|left, right| left.name == right.name);
    out
}

async fn call_nested_tool(
    exec: ExecContext,
    tool_name: String,
    input: Option<JsonValue>,
) -> JsonValue {
    if tool_name == "code_mode" {
        return JsonValue::String("code_mode cannot invoke itself".to_string());
    }

    let nested_config = exec.turn.tools_config.for_code_mode_nested_tools();
    let router = ToolRouter::from_config(
        &nested_config,
        None,
        None,
        exec.turn.dynamic_tools.as_slice(),
    );

    let specs = router.specs();
    let payload = match build_nested_tool_payload(&specs, &tool_name, input) {
        Ok(payload) => payload,
        Err(error) => return JsonValue::String(error),
    };

    let call = ToolCall {
        tool_name: tool_name.clone(),
        call_id: format!("code_mode-{}", uuid::Uuid::new_v4()),
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
        .await;

    match result {
        Ok(result) => result.code_mode_result(),
        Err(error) => JsonValue::String(error.to_string()),
    }
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
        .ok_or_else(|| format!("tool `{tool_name}` is not enabled in code_mode"))
}

fn build_nested_tool_payload(
    specs: &[ToolSpec],
    tool_name: &str,
    input: Option<JsonValue>,
) -> Result<ToolPayload, String> {
    let actual_kind = tool_kind_for_name(specs, tool_name)?;
    match actual_kind {
        CodeModeToolKind::Function => build_function_tool_payload(tool_name, input),
        CodeModeToolKind::Freeform => build_freeform_tool_payload(tool_name, input),
    }
}

fn build_function_tool_payload(
    tool_name: &str,
    input: Option<JsonValue>,
) -> Result<ToolPayload, String> {
    let arguments = match input {
        None => "{}".to_string(),
        Some(JsonValue::Object(map)) => serde_json::to_string(&JsonValue::Object(map))
            .map_err(|err| format!("failed to serialize tool `{tool_name}` arguments: {err}"))?,
        Some(_) => {
            return Err(format!(
                "tool `{tool_name}` expects a JSON object for arguments"
            ));
        }
    };
    Ok(ToolPayload::Function { arguments })
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
            serde_json::from_value(item)
                .map_err(|err| format!("invalid code_mode content item at index {index}: {err}"))
        })
        .collect()
}
