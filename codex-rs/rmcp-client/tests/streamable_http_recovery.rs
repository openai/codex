mod streamable_http_test_support;

use pretty_assertions::assert_eq;

use streamable_http_test_support::arm_session_post_failure;
use streamable_http_test_support::call_echo_tool;
use streamable_http_test_support::create_client;
use streamable_http_test_support::expected_echo_result;
use streamable_http_test_support::spawn_streamable_http_server;

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn streamable_http_404_session_expiry_recovers_and_retries_once() -> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server().await?;
    let client = create_client(&base_url).await?;

    let warmup = call_echo_tool(&client, "warmup").await?;
    assert_eq!(warmup, expected_echo_result("warmup"));

    arm_session_post_failure(&base_url, /*status*/ 404, /*remaining*/ 1).await?;

    let recovered = call_echo_tool(&client, "recovered").await?;
    assert_eq!(recovered, expected_echo_result("recovered"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn streamable_http_401_does_not_trigger_recovery() -> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server().await?;
    let client = create_client(&base_url).await?;

    let warmup = call_echo_tool(&client, "warmup").await?;
    assert_eq!(warmup, expected_echo_result("warmup"));

    arm_session_post_failure(&base_url, /*status*/ 401, /*remaining*/ 2).await?;

    let first_error = call_echo_tool(&client, "unauthorized").await.unwrap_err();
    assert!(first_error.to_string().contains("401"));

    let second_error = call_echo_tool(&client, "still-unauthorized")
        .await
        .unwrap_err();
    assert!(second_error.to_string().contains("401"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn streamable_http_404_recovery_only_retries_once() -> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server().await?;
    let client = create_client(&base_url).await?;

    let warmup = call_echo_tool(&client, "warmup").await?;
    assert_eq!(warmup, expected_echo_result("warmup"));

    arm_session_post_failure(&base_url, /*status*/ 404, /*remaining*/ 2).await?;

    let error = call_echo_tool(&client, "double-404").await.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("handshaking with MCP server failed")
            || error.to_string().contains("Transport channel closed")
    );

    let recovered = call_echo_tool(&client, "after-double-404").await?;
    assert_eq!(recovered, expected_echo_result("after-double-404"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn streamable_http_non_session_failure_does_not_trigger_recovery() -> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server().await?;
    let client = create_client(&base_url).await?;

    let warmup = call_echo_tool(&client, "warmup").await?;
    assert_eq!(warmup, expected_echo_result("warmup"));

    arm_session_post_failure(&base_url, /*status*/ 500, /*remaining*/ 2).await?;

    let first_error = call_echo_tool(&client, "server-error").await.unwrap_err();
    assert!(first_error.to_string().contains("500"));

    let second_error = call_echo_tool(&client, "still-server-error")
        .await
        .unwrap_err();
    assert!(second_error.to_string().contains("500"));

    Ok(())
}
