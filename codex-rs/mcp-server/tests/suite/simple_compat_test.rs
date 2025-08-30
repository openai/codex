use std::env;
use tempfile::TempDir;

use codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use mcp_test_support::McpProcess;

/// Very simple test to verify MCP server starts with compatibility flag
#[tokio::test]
async fn test_mcp_server_starts_with_compat_flag() {
    if env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!("Skipping test because network is disabled");
        return;
    }

    let codex_home = TempDir::new().unwrap();
    std::fs::write(
        codex_home.path().join("config.toml"),
        r#"
model = "mock-model"
approval_policy = "never"
sandbox_policy = "read-only"

[mcp]
compatibility_mode = true

model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider"
base_url = "http://localhost:9999/v1"
wire_api = "chat"
"#,
    )
    .unwrap();

    // Try to start the MCP server with compatibility flag
    let result = McpProcess::new_with_args(codex_home.path(), &["--compatibility-mode"]).await;

    assert!(
        result.is_ok(),
        "Should be able to start MCP server with compatibility flag"
    );

    // Initialize and verify it works
    if let Ok(mut process) = result {
        let init_result =
            tokio::time::timeout(std::time::Duration::from_secs(5), process.initialize()).await;

        assert!(
            init_result.is_ok(),
            "Should be able to initialize MCP server"
        );
    }
}
