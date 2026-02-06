use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use app_test_support::to_response;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::InitializeResponse;
use codex_app_server_protocol::JSONRPCMessage;
use pretty_assertions::assert_eq;
use std::path::Path;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn initialize_uses_client_info_name_as_originator() -> Result<()> {
    let responses = Vec::new();
    let server = create_mock_responses_server_sequence_unchecked(responses).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;

    let message = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.initialize_with_client_info(ClientInfo {
            name: "codex_vscode".to_string(),
            title: Some("Codex VS Code Extension".to_string()),
            version: "0.1.0".to_string(),
        }),
    )
    .await??;

    let JSONRPCMessage::Response(response) = message else {
        anyhow::bail!("expected initialize response, got {message:?}");
    };
    let InitializeResponse { user_agent } = to_response::<InitializeResponse>(response)?;

    assert!(user_agent.starts_with("codex_vscode/"));
    Ok(())
}

#[tokio::test]
async fn initialize_respects_originator_override_env_var() -> Result<()> {
    let responses = Vec::new();
    let server = create_mock_responses_server_sequence_unchecked(responses).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = McpProcess::new_with_env(
        codex_home.path(),
        &[(
            "CODEX_INTERNAL_ORIGINATOR_OVERRIDE",
            Some("codex_originator_via_env_var"),
        )],
    )
    .await?;

    let message = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.initialize_with_client_info(ClientInfo {
            name: "codex_vscode".to_string(),
            title: Some("Codex VS Code Extension".to_string()),
            version: "0.1.0".to_string(),
        }),
    )
    .await??;

    let JSONRPCMessage::Response(response) = message else {
        anyhow::bail!("expected initialize response, got {message:?}");
    };
    let InitializeResponse { user_agent } = to_response::<InitializeResponse>(response)?;

    assert!(user_agent.starts_with("codex_originator_via_env_var/"));
    Ok(())
}

#[tokio::test]
async fn initialize_rejects_invalid_client_name() -> Result<()> {
    let responses = Vec::new();
    let server = create_mock_responses_server_sequence_unchecked(responses).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = McpProcess::new_with_env(
        codex_home.path(),
        &[("CODEX_INTERNAL_ORIGINATOR_OVERRIDE", None)],
    )
    .await?;

    let message = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.initialize_with_client_info(ClientInfo {
            name: "bad\rname".to_string(),
            title: Some("Bad Client".to_string()),
            version: "0.1.0".to_string(),
        }),
    )
    .await??;

    let JSONRPCMessage::Error(error) = message else {
        anyhow::bail!("expected initialize error, got {message:?}");
    };

    assert_eq!(error.error.code, -32600);
    assert_eq!(
        error.error.message,
        "Invalid clientInfo.name: 'bad\rname'. Must be a valid HTTP header value."
    );
    assert_eq!(error.error.data, None);
    Ok(())
}

// Helper to create a config.toml pointing at the mock model server.
fn create_config_toml(
    codex_home: &Path,
    server_uri: &str,
    approval_policy: &str,
) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "mock-model"
approval_policy = "{approval_policy}"
sandbox_mode = "read-only"

model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#
        ),
    )
}
