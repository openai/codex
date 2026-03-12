use async_trait::async_trait;
use serde::Deserialize;

use crate::function_tool::FunctionCallError;
use crate::tools::code_mode;
use crate::tools::code_mode::DEFAULT_WAIT_YIELD_TIME_MS;
use crate::tools::code_mode::PUBLIC_TOOL_NAME;
use crate::tools::code_mode::WAIT_TOOL_NAME;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub struct CodeModeHandler;
pub struct CodeModeWaitHandler;

#[derive(Debug, Deserialize)]
struct ExecWaitArgs {
    session_id: i32,
    #[serde(default = "default_wait_yield_time_ms")]
    yield_time_ms: u64,
    #[serde(default)]
    max_tokens: Option<usize>,
    #[serde(default)]
    terminate: bool,
}

fn default_wait_yield_time_ms() -> u64 {
    DEFAULT_WAIT_YIELD_TIME_MS
}

#[async_trait]
impl ToolHandler for CodeModeHandler {
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
            tracker,
            tool_name,
            payload,
            ..
        } = invocation;

        match payload {
            ToolPayload::Custom { input } if tool_name == PUBLIC_TOOL_NAME => {
                code_mode::execute(session, turn, tracker, input).await
            }
            _ => Err(FunctionCallError::RespondToModel(format!(
                "{PUBLIC_TOOL_NAME} expects raw JavaScript source text"
            ))),
        }
    }
}

#[async_trait]
impl ToolHandler for CodeModeWaitHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            tracker,
            tool_name,
            payload,
            ..
        } = invocation;

        match payload {
            ToolPayload::Function { arguments } if tool_name == WAIT_TOOL_NAME => {
                let args: ExecWaitArgs = parse_arguments(&arguments)?;
                code_mode::wait(
                    session,
                    turn,
                    tracker,
                    args.session_id,
                    args.yield_time_ms,
                    args.max_tokens,
                    args.terminate,
                )
                .await
            }
            _ => Err(FunctionCallError::RespondToModel(format!(
                "{WAIT_TOOL_NAME} expects JSON arguments"
            ))),
        }
    }
}
