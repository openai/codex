use serde::Deserialize;

use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

use super::DEFAULT_WAIT_YIELD_TIME_MS;
use super::ExecContext;
use super::WAIT_TOOL_NAME;
use super::execute_handler::emit_code_cell_ended;
use super::handle_runtime_response;

pub struct CodeModeWaitHandler;

#[derive(Debug, Deserialize)]
struct ExecWaitArgs {
    cell_id: String,
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

fn parse_arguments<T>(arguments: &str) -> Result<T, FunctionCallError>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_str(arguments).map_err(|err| {
        FunctionCallError::RespondToModel(format!("failed to parse function arguments: {err}"))
    })
}

impl ToolHandler for CodeModeWaitHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            call_id,
            tool_name,
            payload,
            ..
        } = invocation;

        match payload {
            ToolPayload::Function { arguments }
                if tool_name.namespace.is_none() && tool_name.name.as_str() == WAIT_TOOL_NAME =>
            {
                let args: ExecWaitArgs = parse_arguments(&arguments)?;
                let exec = ExecContext { session, turn };
                let started_at = std::time::Instant::now();
                let response = exec
                    .session
                    .services
                    .code_mode_service
                    .wait(codex_code_mode::WaitRequest {
                        cell_id: args.cell_id.clone(),
                        yield_time_ms: args.yield_time_ms,
                        terminate: args.terminate,
                    })
                    .await
                    .map_err(FunctionCallError::RespondToModel)?;
                if !matches!(&response, codex_code_mode::RuntimeResponse::Yielded { .. })
                    && !is_missing_cell_response(&response, &args.cell_id)
                {
                    // The initial `exec` call only records a terminal end when
                    // it finishes immediately. Yielded cells close later via
                    // `wait`, so the runtime object stays live until this point.
                    // Duplicate waits for a removed cell return a model-visible
                    // error but must not rewrite an already-finished CodeCell.
                    emit_code_cell_ended(
                        &exec,
                        &args.cell_id,
                        &response,
                        Some(&call_id),
                        /*result_payload*/ None,
                    );
                }
                handle_runtime_response(&exec, response, args.max_tokens, started_at)
                    .await
                    .map_err(FunctionCallError::RespondToModel)
            }
            _ => Err(FunctionCallError::RespondToModel(format!(
                "{WAIT_TOOL_NAME} expects JSON arguments"
            ))),
        }
    }
}

fn is_missing_cell_response(response: &codex_code_mode::RuntimeResponse, cell_id: &str) -> bool {
    let codex_code_mode::RuntimeResponse::Result {
        error_text: Some(error_text),
        ..
    } = response
    else {
        return false;
    };
    error_text == &format!("exec cell {cell_id} not found")
}
