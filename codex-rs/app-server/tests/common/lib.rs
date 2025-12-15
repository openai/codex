mod auth_fixtures;
mod mcp_process;
mod mock_model_server;
mod models_cache;
mod responses;
mod rollout;

pub use auth_fixtures::ChatGptAuthFixture;
pub use auth_fixtures::ChatGptIdTokenClaims;
pub use auth_fixtures::encode_id_token;
pub use auth_fixtures::write_chatgpt_auth;
use codex_app_server_protocol::JSONRPCResponse;
pub use core_test_support::format_with_current_shell;
pub use core_test_support::format_with_current_shell_display;
pub use core_test_support::format_with_current_shell_display_non_login;
pub use core_test_support::format_with_current_shell_non_login;
pub use core_test_support::test_path_buf_with_windows;
pub use core_test_support::test_tmp_path;
pub use core_test_support::test_tmp_path_buf;
pub use mcp_process::McpProcess;
pub use mock_model_server::create_mock_chat_completions_server;
pub use mock_model_server::create_mock_chat_completions_server_unchecked;
pub use models_cache::write_models_cache;
pub use models_cache::write_models_cache_with_models;
pub use responses::create_apply_patch_sse_response;
pub use responses::create_exec_command_sse_response;
pub use responses::create_final_assistant_message_sse_response;
pub use responses::create_shell_command_sse_response;
pub use rollout::create_fake_rollout;
use serde::de::DeserializeOwned;

pub fn to_response<T: DeserializeOwned>(response: JSONRPCResponse) -> anyhow::Result<T> {
    let value = serde_json::to_value(response.result)?;
    let codex_response = serde_json::from_value(value)?;
    Ok(codex_response)
}
