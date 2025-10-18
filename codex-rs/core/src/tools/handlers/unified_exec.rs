use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::borrow::Cow;
use std::convert::TryFrom;

use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use crate::unified_exec::UnifiedExecError;
use crate::unified_exec::UnifiedExecMode;
use crate::unified_exec::UnifiedExecRequest;
use crate::unified_exec::UnifiedExecResult;

pub struct UnifiedExecHandler;

#[derive(Deserialize)]
struct UnifiedExecArgs {
    #[serde(default)]
    cmd: Option<String>,
    #[serde(default)]
    session_id: Option<JsonValue>,
    #[serde(default)]
    chars: Option<String>,
    #[serde(default)]
    yield_time_ms: Option<u64>,
    #[serde(default)]
    max_output_tokens: Option<usize>,
    #[serde(default)]
    output_chunk_id: Option<bool>,
    #[serde(default)]
    output_wall_time: Option<bool>,
    #[serde(default)]
    output_json: Option<bool>,
    #[serde(default)]
    shell: Option<String>,
    #[serde(default)]
    login: Option<bool>,
    #[serde(default)]
    cwd: Option<String>,
}

#[async_trait]
impl ToolHandler for UnifiedExecHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::UnifiedExec
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(
            payload,
            ToolPayload::UnifiedExec { .. } | ToolPayload::Function { .. }
        )
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session, payload, ..
        } = invocation;

        let args = match payload {
            ToolPayload::UnifiedExec { arguments } | ToolPayload::Function { arguments } => {
                serde_json::from_str::<UnifiedExecArgs>(&arguments).map_err(|err| {
                    FunctionCallError::RespondToModel(format!(
                        "failed to parse function arguments: {err:?}"
                    ))
                })?
            }
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "unified_exec handler received unsupported payload".to_string(),
                ));
            }
        };

        let UnifiedExecArgs {
            cmd,
            session_id,
            chars,
            yield_time_ms,
            max_output_tokens,
            output_chunk_id,
            output_wall_time,
            output_json,
            shell,
            login,
            cwd,
        } = args;

        let chars = chars.unwrap_or_default();

        let mode = if let Some(raw_session_id) = session_id {
            if cmd.is_some() {
                return Err(FunctionCallError::RespondToModel(
                    "provide either cmd or session_id, not both".to_string(),
                ));
            }
            let session_id = parse_session_id(raw_session_id)?;
            UnifiedExecMode::Write {
                session_id,
                chars: chars.as_str(),
                yield_time_ms,
                max_output_tokens,
            }
        } else {
            let cmd_value = cmd.ok_or_else(|| {
                FunctionCallError::RespondToModel(
                    "cmd is required when session_id is not provided".to_string(),
                )
            })?;
            UnifiedExecMode::Start {
                cmd: Cow::Owned(cmd_value),
                yield_time_ms,
                max_output_tokens,
                shell: shell.as_deref(),
                login,
                cwd: cwd.as_deref(),
            }
        };

        let request = UnifiedExecRequest {
            mode,
            output_chunk_id,
            output_wall_time,
            output_json,
        };

        let result = session
            .run_unified_exec_request(request)
            .await
            .map_err(map_unified_exec_error)?;

        Ok(tool_output_from_result(result))
    }
}

fn tool_output_from_result(result: UnifiedExecResult) -> ToolOutput {
    let content = result.content.into_string();
    ToolOutput::Function {
        content,
        success: Some(true),
    }
}

fn parse_session_id(value: JsonValue) -> Result<i32, FunctionCallError> {
    match value {
        JsonValue::Number(num) => {
            if let Some(int) = num.as_i64() {
                i32::try_from(int).map_err(|_| {
                    FunctionCallError::RespondToModel(format!(
                        "session_id value {int} exceeds i32 range"
                    ))
                })
            } else {
                Err(FunctionCallError::RespondToModel(
                    "session_id must be an integer".to_string(),
                ))
            }
        }
        JsonValue::String(text) => text.parse::<i32>().map_err(|err| {
            FunctionCallError::RespondToModel(format!("invalid session_id '{text}': {err}"))
        }),
        other => Err(FunctionCallError::RespondToModel(format!(
            "session_id must be a string or integer, got {other}"
        ))),
    }
}

fn map_unified_exec_error(err: UnifiedExecError) -> FunctionCallError {
    match err {
        UnifiedExecError::SessionExited {
            session_id,
            exit_code,
        } => {
            let detail = exit_code
                .map(|code| format!(" with code {code}"))
                .unwrap_or_default();
            FunctionCallError::RespondToModel(format!(
                "session {session_id} has already exited{detail}. Start a new session with cmd."
            ))
        }
        UnifiedExecError::WriteToStdin { session_id } => FunctionCallError::RespondToModel(
            format!("failed to write to session {session_id}; the process may have exited"),
        ),
        other => FunctionCallError::RespondToModel(format!("unified exec failed: {other}")),
    }
}
