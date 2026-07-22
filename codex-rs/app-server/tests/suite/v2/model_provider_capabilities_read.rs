use std::time::Duration;

use anyhow::Result;
use app_test_support::TestAppServer;
use codex_app_server_protocol::ModelProviderCapabilitiesReadParams;
use codex_app_server_protocol::ModelProviderCapabilitiesReadResponse;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

#[tokio::test]
async fn read_default_provider_capabilities() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_model_provider_capabilities_read_request(ModelProviderCapabilitiesReadParams {})
        .await?;
    let received: ModelProviderCapabilitiesReadResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;

    let expected = ModelProviderCapabilitiesReadResponse {
        namespace_tools: true,
        image_generation: true,
        web_search: true,
    };
    assert_eq!(received, expected);
    Ok(())
}

#[tokio::test]
async fn read_amazon_bedrock_provider_capabilities() -> Result<()> {
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        r#"model_provider = "amazon-bedrock"
"#,
    )?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_model_provider_capabilities_read_request(ModelProviderCapabilitiesReadParams {})
        .await?;
    let received: ModelProviderCapabilitiesReadResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;

    let expected = ModelProviderCapabilitiesReadResponse {
        namespace_tools: true,
        image_generation: false,
        web_search: false,
    };
    assert_eq!(received, expected);
    Ok(())
}
