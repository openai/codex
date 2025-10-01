mod mcp_process;
mod mock_model_server;
mod responses;

use codex_app_server_protocol::JSONRPCResponse;
pub use mcp_process::McpProcess;
pub use mock_model_server::create_mock_chat_completions_server;
pub use responses::create_apply_patch_sse_response;
pub use responses::create_final_assistant_message_sse_response;
pub use responses::create_shell_sse_response;
use serde::de::DeserializeOwned;

pub fn to_response<T: DeserializeOwned>(response: JSONRPCResponse) -> anyhow::Result<T> {
    let value = serde_json::to_value(response.result)?;
    let codex_response = serde_json::from_value(value)?;
    Ok(codex_response)
}
