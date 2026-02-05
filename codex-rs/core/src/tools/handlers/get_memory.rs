use crate::function_tool::FunctionCallError;
use crate::state_db;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use async_trait::async_trait;
use codex_protocol::ThreadId;
use codex_protocol::models::FunctionCallOutputBody;
use serde::Deserialize;
use serde_json::json;

pub struct GetMemoryHandler;

#[derive(Deserialize)]
struct GetMemoryArgs {
    memory_id: String,
}

#[async_trait]
impl ToolHandler for GetMemoryHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session, payload, ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "get_memory handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: GetMemoryArgs = parse_arguments(&arguments)?;
        let thread_id = ThreadId::from_string(args.memory_id.as_str()).map_err(|err| {
            FunctionCallError::RespondToModel(format!("memory_id must be a valid thread id: {err}"))
        })?;

        let state_db_ctx = session.state_db();
        let memory =
            state_db::get_thread_memory(state_db_ctx.as_deref(), thread_id, "get_memory_tool")
                .await
                .ok_or_else(|| {
                    FunctionCallError::RespondToModel(format!(
                        "memory not found for memory_id={}",
                        args.memory_id
                    ))
                })?;

        let content = serde_json::to_string_pretty(&json!({
            "memory_id": args.memory_id,
            "trace_summary": memory.trace_summary,
            "memory_summary": memory.memory_summary,
        }))
        .map_err(|err| {
            FunctionCallError::Fatal(format!("failed to serialize memory payload: {err}"))
        })?;

        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text(content),
            success: Some(true),
        })
    }
}
