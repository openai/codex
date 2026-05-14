use crate::function_tool::FunctionCallError;
use crate::tools::context::ExecCommandToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::PostToolUsePayload;
use crate::tools::registry::ToolExecutor;
use crate::tools::registry::ToolHandler;
use crate::unified_exec::WriteStdinRequest;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::TerminalInteractionEvent;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use serde::Deserialize;

use super::super::shell_spec::create_write_stdin_tool;
use super::effective_max_output_tokens;
use super::post_unified_exec_tool_use_payload;

#[derive(Debug, Deserialize)]
struct WriteStdinArgs {
    // The model is trained on `session_id`.
    session_id: i32,
    #[serde(default)]
    chars: String,
    #[serde(default = "super::default_write_stdin_yield_time_ms")]
    yield_time_ms: u64,
    #[serde(default)]
    max_output_tokens: Option<usize>,
}

pub struct WriteStdinHandler;

#[async_trait::async_trait]
impl ToolExecutor<ToolInvocation> for WriteStdinHandler {
    type Output = ExecCommandToolOutput;

    fn tool_name(&self) -> ToolName {
        ToolName::plain("write_stdin")
    }

    fn spec(&self) -> Option<ToolSpec> {
        Some(create_write_stdin_tool())
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            payload,
            ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "write_stdin handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: WriteStdinArgs = parse_arguments(&arguments)?;
        let max_output_tokens =
            effective_max_output_tokens(args.max_output_tokens, turn.truncation_policy);
        let response = session
            .services
            .unified_exec_manager
            .write_stdin(WriteStdinRequest {
                process_id: args.session_id,
                input: &args.chars,
                yield_time_ms: args.yield_time_ms,
                max_output_tokens: Some(max_output_tokens),
            })
            .await
            .map_err(|err| {
                FunctionCallError::RespondToModel(format!("write_stdin failed: {err}"))
            })?;

        let interaction = TerminalInteractionEvent {
            call_id: response.event_call_id.clone(),
            process_id: args.session_id.to_string(),
            stdin: args.chars.clone(),
        };
        session
            .send_event(turn.as_ref(), EventMsg::TerminalInteraction(interaction))
            .await;

        Ok(response)
    }
}

impl ToolHandler for WriteStdinHandler {
    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    fn post_tool_use_payload(
        &self,
        invocation: &ToolInvocation,
        result: &Self::Output,
    ) -> Option<PostToolUsePayload> {
        post_unified_exec_tool_use_payload(invocation, result)
    }
}
