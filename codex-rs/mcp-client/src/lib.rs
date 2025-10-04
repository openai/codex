mod mcp_client;

pub use mcp_client::McpClient;
#[cfg(target_os = "windows")]
pub use mcp_client::build_test_command;
