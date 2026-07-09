use std::time::Duration;

use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::create_fake_rollout;
use app_test_support::to_response;
use codex_app_server_protocol::ConfigValueWriteParams;
use codex_app_server_protocol::ConfigWriteResponse;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::LoginAccountResponse;
use codex_app_server_protocol::LogoutAccountResponse;
use codex_app_server_protocol::MergeStrategy;
use codex_app_server_protocol::ModelListParams;
use codex_app_server_protocol::ModelListResponse;
use codex_app_server_protocol::ModelProviderCapabilitiesReadParams;
use codex_app_server_protocol::ModelProviderCapabilitiesReadResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadListResponse;
use codex_app_server_protocol::WriteStatus;
use codex_model_provider_info::AMAZON_BEDROCK_GPT_5_5_MODEL_ID;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

fn write_mock_provider_config(codex_home: &TempDir) -> Result<()> {
    std::fs::write(
        codex_home.path().join("config.toml"),
        r#"
model = "mock-model"
model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider"
base_url = "http://127.0.0.1:0/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#,
    )?;
    Ok(())
}

async fn read_capabilities(
    mcp: &mut TestAppServer,
) -> Result<ModelProviderCapabilitiesReadResponse> {
    let request_id = mcp
        .send_model_provider_capabilities_read_request(ModelProviderCapabilitiesReadParams {})
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}

async fn list_models(mcp: &mut TestAppServer) -> Result<ModelListResponse> {
    let request_id = mcp
        .send_list_models_request(ModelListParams {
            limit: Some(100),
            cursor: None,
            include_hidden: Some(true),
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}

async fn list_threads_for_default_provider(mcp: &mut TestAppServer) -> Result<ThreadListResponse> {
    let request_id = mcp
        .send_thread_list_request(ThreadListParams {
            cursor: None,
            limit: Some(100),
            sort_key: None,
            sort_direction: None,
            model_providers: None,
            source_kinds: None,
            archived: None,
            cwd: None,
            use_state_db_only: false,
            search_term: None,
            parent_thread_id: None,
            ancestor_thread_id: None,
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}

fn bedrock_capabilities() -> ModelProviderCapabilitiesReadResponse {
    ModelProviderCapabilitiesReadResponse {
        namespace_tools: true,
        image_generation: false,
        web_search: false,
    }
}

#[tokio::test]
async fn config_write_publishes_provider_runtime_before_responding() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_mock_provider_config(&codex_home)?;
    let mock_thread_id = create_fake_rollout(
        codex_home.path(),
        "2026-01-01T00-00-00",
        "2026-01-01T00:00:00Z",
        "mock provider thread",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;
    let bedrock_thread_id = create_fake_rollout(
        codex_home.path(),
        "2026-01-01T00-00-01",
        "2026-01-01T00:00:01Z",
        "Bedrock provider thread",
        Some("amazon-bedrock"),
        /*git_info*/ None,
    )?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build()
        .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    assert_eq!(
        list_threads_for_default_provider(&mut mcp)
            .await?
            .data
            .into_iter()
            .map(|thread| thread.id)
            .collect::<Vec<_>>(),
        vec![mock_thread_id]
    );

    let request_id = mcp
        .send_config_value_write_request(ConfigValueWriteParams {
            key_path: "model_provider".to_string(),
            value: json!("amazon-bedrock"),
            merge_strategy: MergeStrategy::Replace,
            file_path: None,
            expected_version: None,
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(
        to_response::<ConfigWriteResponse>(response)?.status,
        WriteStatus::Ok
    );

    assert_eq!(read_capabilities(&mut mcp).await?, bedrock_capabilities());
    assert!(
        list_models(&mut mcp)
            .await?
            .data
            .iter()
            .any(|model| model.model == AMAZON_BEDROCK_GPT_5_5_MODEL_ID)
    );
    assert_eq!(
        list_threads_for_default_provider(&mut mcp)
            .await?
            .data
            .into_iter()
            .map(|thread| thread.id)
            .collect::<Vec<_>>(),
        vec![bedrock_thread_id]
    );
    Ok(())
}

#[tokio::test]
async fn managed_bedrock_login_and_logout_publish_provider_runtime() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_mock_provider_config(&codex_home)?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .build()
        .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let login_request_id = mcp
        .send_login_account_amazon_bedrock_request("managed-bedrock-api-key", "us-west-2")
        .await?;
    let login_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(login_request_id)),
    )
    .await??;
    assert_eq!(
        to_response::<LoginAccountResponse>(login_response)?,
        LoginAccountResponse::AmazonBedrock {}
    );
    assert_eq!(read_capabilities(&mut mcp).await?, bedrock_capabilities());

    let logout_request_id = mcp.send_logout_account_request().await?;
    let logout_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(logout_request_id)),
    )
    .await??;
    assert_eq!(
        to_response::<LogoutAccountResponse>(logout_response)?,
        LogoutAccountResponse {}
    );
    assert_eq!(
        read_capabilities(&mut mcp).await?,
        ModelProviderCapabilitiesReadResponse {
            namespace_tools: true,
            image_generation: true,
            web_search: true,
        }
    );
    Ok(())
}
