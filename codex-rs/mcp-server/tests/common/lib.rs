mod mcp_process;
mod mock_model_server;
mod responses;

pub use core_test_support::format_with_current_shell;
pub use core_test_support::format_with_current_shell_display_non_login;
pub use core_test_support::format_with_current_shell_non_login;
pub use mcp_process::McpProcess;
pub use mock_model_server::create_mock_responses_server;
pub use responses::create_apply_patch_sse_response;
pub use responses::create_final_assistant_message_sse_response;
pub use responses::create_shell_command_sse_response;
use rmcp::model::JsonRpcResponse;
use serde::de::DeserializeOwned;

pub fn to_response<T: DeserializeOwned>(
    response: JsonRpcResponse<serde_json::Value>,
) -> anyhow::Result<T> {
    let value = serde_json::to_value(response.result)?;
    let codex_response = serde_json::from_value(value)?;
    Ok(codex_response)
}
