use async_trait::async_trait;
use serde::Deserialize;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

use super::CODE_MODE_PRAGMA_PREFIX;
use super::CodeModeSessionProgress;
use super::ExecContext;
use super::PUBLIC_TOOL_NAME;
use super::build_enabled_tools;
use super::handle_node_message;
use super::protocol::HostToNodeMessage;
use super::protocol::build_source;

pub struct CodeModeExecuteHandler;
const MAX_JS_SAFE_INTEGER: u64 = (1_u64 << 53) - 1;

#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct CodeModeExecPragma {
    #[serde(default)]
    yield_time_ms: Option<u64>,
    #[serde(default)]
    max_output_tokens: Option<usize>,
}

#[derive(Debug, PartialEq, Eq)]
struct CodeModeExecArgs {
    code: String,
    yield_time_ms: Option<u64>,
    max_output_tokens: Option<usize>,
}

impl CodeModeExecuteHandler {
    async fn execute(
        &self,
        session: std::sync::Arc<Session>,
        turn: std::sync::Arc<TurnContext>,
        code: String,
    ) -> Result<FunctionToolOutput, FunctionCallError> {
        let args = parse_freeform_args(&code)?;
        let exec = ExecContext { session, turn };
        let enabled_tools = build_enabled_tools(&exec).await;
        let service = &exec.session.services.code_mode_service;
        let stored_values = service.stored_values().await;
        let source =
            build_source(&args.code, &enabled_tools).map_err(FunctionCallError::RespondToModel)?;
        let cell_id = service.allocate_cell_id().await;
        let request_id = service.allocate_request_id().await;
        let process_slot = service
            .ensure_started()
            .await
            .map_err(|err| FunctionCallError::RespondToModel(err.to_string()))?;
        let started_at = std::time::Instant::now();
        let message = HostToNodeMessage::Start {
            request_id: request_id.clone(),
            cell_id: cell_id.clone(),
            default_yield_time_ms: super::DEFAULT_EXEC_YIELD_TIME_MS,
            enabled_tools,
            stored_values,
            source,
            yield_time_ms: args.yield_time_ms,
            max_output_tokens: args.max_output_tokens,
        };
        let result = {
            let mut process_slot = process_slot;
            let Some(process) = process_slot.as_mut() else {
                return Err(FunctionCallError::RespondToModel(format!(
                    "{PUBLIC_TOOL_NAME} runner failed to start"
                )));
            };
            let message = process
                .send(&request_id, &message)
                .await
                .map_err(|err| err.to_string());
            let message = match message {
                Ok(message) => message,
                Err(error) => return Err(FunctionCallError::RespondToModel(error)),
            };
            handle_node_message(&exec, cell_id, message, None, started_at).await
        };
        match result {
            Ok(CodeModeSessionProgress::Finished(output))
            | Ok(CodeModeSessionProgress::Yielded { output }) => Ok(output),
            Err(error) => Err(FunctionCallError::RespondToModel(error)),
        }
    }
}

fn parse_freeform_args(input: &str) -> Result<CodeModeExecArgs, FunctionCallError> {
    if input.trim().is_empty() {
        return Err(FunctionCallError::RespondToModel(
            "exec expects raw JavaScript source text (non-empty). Provide JS only, optionally with first-line `// @exec: {\"yield_time_ms\": 10000, \"max_output_tokens\": 1000}`.".to_string(),
        ));
    }

    let mut args = CodeModeExecArgs {
        code: input.to_string(),
        yield_time_ms: None,
        max_output_tokens: None,
    };

    let mut lines = input.splitn(2, '\n');
    let first_line = lines.next().unwrap_or_default();
    let rest = lines.next().unwrap_or_default();
    let trimmed = first_line.trim_start();
    let Some(pragma) = trimmed.strip_prefix(CODE_MODE_PRAGMA_PREFIX) else {
        return Ok(args);
    };

    if rest.trim().is_empty() {
        return Err(FunctionCallError::RespondToModel(
            "exec pragma must be followed by JavaScript source on subsequent lines".to_string(),
        ));
    }

    let directive = pragma.trim();
    if directive.is_empty() {
        return Err(FunctionCallError::RespondToModel(
            "exec pragma must be a JSON object with supported fields `yield_time_ms` and `max_output_tokens`"
                .to_string(),
        ));
    }

    let value: serde_json::Value = serde_json::from_str(directive).map_err(|err| {
        FunctionCallError::RespondToModel(format!(
            "exec pragma must be valid JSON with supported fields `yield_time_ms` and `max_output_tokens`: {err}"
        ))
    })?;
    let object = value.as_object().ok_or_else(|| {
        FunctionCallError::RespondToModel(
            "exec pragma must be a JSON object with supported fields `yield_time_ms` and `max_output_tokens`"
                .to_string(),
        )
    })?;
    for key in object.keys() {
        match key.as_str() {
            "yield_time_ms" | "max_output_tokens" => {}
            _ => {
                return Err(FunctionCallError::RespondToModel(format!(
                    "exec pragma only supports `yield_time_ms` and `max_output_tokens`; got `{key}`"
                )));
            }
        }
    }

    let pragma: CodeModeExecPragma = serde_json::from_value(value).map_err(|err| {
        FunctionCallError::RespondToModel(format!(
            "exec pragma fields `yield_time_ms` and `max_output_tokens` must be non-negative safe integers: {err}"
        ))
    })?;
    if pragma
        .yield_time_ms
        .is_some_and(|yield_time_ms| yield_time_ms > MAX_JS_SAFE_INTEGER)
    {
        return Err(FunctionCallError::RespondToModel(
            "exec pragma field `yield_time_ms` must be a non-negative safe integer".to_string(),
        ));
    }
    if pragma.max_output_tokens.is_some_and(|max_output_tokens| {
        u64::try_from(max_output_tokens)
            .map(|max_output_tokens| max_output_tokens > MAX_JS_SAFE_INTEGER)
            .unwrap_or(true)
    }) {
        return Err(FunctionCallError::RespondToModel(
            "exec pragma field `max_output_tokens` must be a non-negative safe integer".to_string(),
        ));
    }
    args.code = rest.to_string();
    args.yield_time_ms = pragma.yield_time_ms;
    args.max_output_tokens = pragma.max_output_tokens;
    Ok(args)
}

#[async_trait]
impl ToolHandler for CodeModeExecuteHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Custom { .. })
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            tool_name,
            payload,
            ..
        } = invocation;

        match payload {
            ToolPayload::Custom { input } if tool_name == PUBLIC_TOOL_NAME => {
                self.execute(session, turn, input).await
            }
            _ => Err(FunctionCallError::RespondToModel(format!(
                "{PUBLIC_TOOL_NAME} expects raw JavaScript source text"
            ))),
        }
    }
}

#[cfg(test)]
#[path = "execute_handler_tests.rs"]
mod execute_handler_tests;
