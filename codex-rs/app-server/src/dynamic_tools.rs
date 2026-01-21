use codex_app_server_protocol::DynamicToolCallResponse;
use codex_core::CodexThread;
use codex_protocol::dynamic_tools::DynamicToolResponse as CoreDynamicToolResponse;
use codex_protocol::protocol::Op;
use std::sync::Arc;
use tokio::sync::oneshot;
use tracing::error;

pub(crate) const RESERVED_DYNAMIC_TOOL_NAMES: &[&str] = &[
    "shell",
    "container.exec",
    "local_shell",
    "shell_command",
    "exec_command",
    "write_stdin",
    "apply_patch",
    "update_plan",
    "list_mcp_resources",
    "list_mcp_resource_templates",
    "read_mcp_resource",
    "request_user_input",
    "view_image",
    "spawn_agent",
    "send_input",
    "wait",
    "close_agent",
    "web_search",
    "grep_files",
    "read_file",
    "list_dir",
    "test_sync_tool",
];

pub(crate) async fn on_call_response(
    call_id: String,
    receiver: oneshot::Receiver<serde_json::Value>,
    conversation: Arc<CodexThread>,
) {
    let response = receiver.await;
    let value = match response {
        Ok(value) => value,
        Err(err) => {
            error!("request failed: {err:?}");
            let fallback = CoreDynamicToolResponse {
                call_id: call_id.clone(),
                output: "dynamic tool request failed".to_string(),
                success: false,
            };
            if let Err(err) = conversation
                .submit(Op::DynamicToolResponse {
                    id: call_id.clone(),
                    response: fallback,
                })
                .await
            {
                error!("failed to submit DynamicToolResponse: {err}");
            }
            return;
        }
    };

    let response = serde_json::from_value::<DynamicToolCallResponse>(value).unwrap_or_else(|err| {
        error!("failed to deserialize DynamicToolCallResponse: {err}");
        DynamicToolCallResponse {
            output: "dynamic tool response was invalid".to_string(),
            success: false,
        }
    });
    let response = CoreDynamicToolResponse {
        call_id: call_id.clone(),
        output: response.output,
        success: response.success,
    };
    if let Err(err) = conversation
        .submit(Op::DynamicToolResponse {
            id: call_id,
            response,
        })
        .await
    {
        error!("failed to submit DynamicToolResponse: {err}");
    }
}
