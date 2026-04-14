use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::EnvironmentListParams;
use codex_app_server_protocol::EnvironmentListResponse;
use codex_app_server_protocol::EnvironmentRegisterParams;
use codex_app_server_protocol::EnvironmentRegisterResponse;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use tempfile::TempDir;
use tokio::time::timeout;

use super::thread_start::create_config_toml_without_approval_policy;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn environment_register_and_list_round_trip() -> Result<()> {
    let server = app_test_support::create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml_without_approval_policy(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let register_a = mcp
        .send_environment_register_request(EnvironmentRegisterParams {
            environment_id: "staging".to_string(),
            exec_server_url: Some(" ws://127.0.0.1:9001 ".to_string()),
        })
        .await?;
    let register_a_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(register_a)),
    )
    .await??;
    let EnvironmentRegisterResponse { environment } =
        to_response::<EnvironmentRegisterResponse>(register_a_response)?;
    assert_eq!(environment.id, "staging");
    assert_eq!(
        environment.exec_server_url.as_deref(),
        Some("ws://127.0.0.1:9001")
    );

    let register_b = mcp
        .send_environment_register_request(EnvironmentRegisterParams {
            environment_id: "dev".to_string(),
            exec_server_url: None,
        })
        .await?;
    let register_b_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(register_b)),
    )
    .await??;
    let EnvironmentRegisterResponse { environment } =
        to_response::<EnvironmentRegisterResponse>(register_b_response)?;
    assert_eq!(environment.id, "dev");
    assert_eq!(environment.exec_server_url, None);

    let list_request = mcp
        .send_environment_list_request(EnvironmentListParams::default())
        .await?;
    let list_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(list_request)),
    )
    .await??;
    let EnvironmentListResponse { data } = to_response::<EnvironmentListResponse>(list_response)?;

    assert_eq!(data.len(), 2);
    assert_eq!(data[0].id, "dev");
    assert_eq!(data[0].exec_server_url, None);
    assert_eq!(data[1].id, "staging");
    assert_eq!(
        data[1].exec_server_url.as_deref(),
        Some("ws://127.0.0.1:9001")
    );

    Ok(())
}
