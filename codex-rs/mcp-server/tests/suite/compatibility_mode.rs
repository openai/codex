use std::env;
use std::path::Path;
use tempfile::TempDir;
use tokio::time::timeout;

use codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use codex_mcp_server::CodexToolCallParam;
use mcp_test_support::McpProcess;
use mcp_types::RequestId;
use serde_json::json;

// Allow ample time on slower CI or under load to avoid flakes.
const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);
// Compatibility mode should respond immediately (< 100ms)
const IMMEDIATE_RESPONSE_TIMEOUT_MS: u64 = 100;
// Time to wait to ensure no notifications arrive
const NO_NOTIFICATION_WAIT_MS: u64 = 500;

/// Test that compatibility mode returns immediate responses with session ID
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_compatibility_mode_immediate_response() {
    if env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!("Skipping test because it cannot execute when network is disabled");
        return;
    }

    if let Err(err) = compatibility_mode_immediate_response().await {
        panic!("failure: {err}");
    }
}

async fn compatibility_mode_immediate_response() -> anyhow::Result<()> {
    // In compatibility mode, we don't actually call the LLM for the initial response
    // So we don't need a mock server for this test

    // Create config with compatibility mode enabled
    let codex_home = TempDir::new()?;
    create_config_with_compatibility_mode(codex_home.path(), "http://localhost:9999", true)?;

    // Start MCP server with --compatibility-mode flag
    let mut mcp_process =
        McpProcess::new_with_args(codex_home.path(), &["--compatibility-mode"]).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp_process.initialize()).await??;

    // Send a codex tool call
    let codex_request_id = mcp_process
        .send_codex_tool_call(CodexToolCallParam {
            prompt: "Hello, test!".to_string(),
            ..Default::default()
        })
        .await?;

    // Verify we get an immediate response (within 100ms)
    let response = mcp_process
        .read_immediate_response(
            RequestId::Integer(codex_request_id),
            IMMEDIATE_RESPONSE_TIMEOUT_MS,
        )
        .await?;

    // Verify response contains session ID
    let session_id = response
        .result
        .get("structuredContent")
        .and_then(|sc| sc.get("sessionId"))
        .and_then(|id| id.as_str())
        .ok_or_else(|| anyhow::anyhow!("No session ID in response"))?;

    assert!(!session_id.is_empty(), "Session ID should not be empty");
    assert!(
        response.result.get("content").is_some(),
        "Response should contain content"
    );

    // Verify NO async notifications arrive
    let has_notifications = mcp_process
        .check_for_notifications(NO_NOTIFICATION_WAIT_MS)
        .await;
    assert!(
        !has_notifications,
        "Should not receive notifications in compatibility mode"
    );

    Ok(())
}

/// Test session continuity with codex-reply tool
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_compatibility_mode_session_continuity() {
    if env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!("Skipping test because it cannot execute when network is disabled");
        return;
    }

    if let Err(err) = compatibility_mode_session_continuity().await {
        panic!("failure: {err}");
    }
}

async fn compatibility_mode_session_continuity() -> anyhow::Result<()> {
    // In compatibility mode, we don't need mock servers for immediate responses
    let codex_home = TempDir::new()?;
    create_config_with_compatibility_mode(codex_home.path(), "http://localhost:9999", true)?;

    let mut mcp_process =
        McpProcess::new_with_args(codex_home.path(), &["--compatibility-mode"]).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp_process.initialize()).await??;

    // First call to codex tool
    let first_request_id = mcp_process
        .send_codex_tool_call(CodexToolCallParam {
            prompt: "Start conversation".to_string(),
            ..Default::default()
        })
        .await?;

    let first_response = mcp_process
        .read_immediate_response(
            RequestId::Integer(first_request_id),
            IMMEDIATE_RESPONSE_TIMEOUT_MS,
        )
        .await?;

    let session_id = first_response
        .result
        .get("structuredContent")
        .and_then(|sc| sc.get("sessionId"))
        .and_then(|id| id.as_str())
        .ok_or_else(|| anyhow::anyhow!("No session ID in first response"))?
        .to_string();

    // Test that the codex-reply call is accepted and handled synchronously

    // Second call using codex-reply with session ID
    let second_request_id = mcp_process
        .send_codex_reply_tool_call(session_id.clone(), "Continue conversation".to_string())
        .await?;

    let (second_response, elapsed) = mcp_process
        .measure_response_time(RequestId::Integer(second_request_id))
        .await?;

    // Verify response is immediate in compatibility mode
    assert!(
        elapsed.as_millis() < IMMEDIATE_RESPONSE_TIMEOUT_MS as u128,
        "Reply should be immediate, took {:?}",
        elapsed
    );

    // Verify we got a valid response
    assert!(
        second_response.result.get("content").is_some(),
        "Reply response should contain content"
    );

    Ok(())
}

/// Test that original async mode still works (without compatibility flag)
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_original_async_mode_works() {
    if env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!("Skipping test because it cannot execute when network is disabled");
        return;
    }

    if let Err(err) = original_async_mode_works().await {
        panic!("failure: {err}");
    }
}

async fn original_async_mode_works() -> anyhow::Result<()> {
    // Create config WITHOUT compatibility mode
    // In async mode, the actual LLM call happens asynchronously
    let codex_home = TempDir::new()?;
    create_config_with_compatibility_mode(codex_home.path(), "http://localhost:9999", false)?;

    // Start MCP server WITHOUT --compatibility-mode flag
    let mut mcp_process = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp_process.initialize()).await??;

    // Send a codex tool call
    let codex_request_id = mcp_process
        .send_codex_tool_call(CodexToolCallParam {
            prompt: "Test async mode".to_string(),
            ..Default::default()
        })
        .await?;

    // In async mode, we should NOT get an immediate response
    // Try to read a response immediately - this should timeout
    let immediate_result = mcp_process
        .read_immediate_response(
            RequestId::Integer(codex_request_id),
            IMMEDIATE_RESPONSE_TIMEOUT_MS,
        )
        .await;

    // In async mode, we expect this to timeout (no immediate response)
    assert!(
        immediate_result.is_err(),
        "Should NOT receive immediate response in async mode"
    );

    // Note: We can't test the full async flow without a real server
    // but we've verified that async mode doesn't send immediate responses

    Ok(())
}

/// Test error handling for invalid session ID in codex-reply
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_invalid_session_id_error() {
    if env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!("Skipping test because it cannot execute when network is disabled");
        return;
    }

    if let Err(err) = invalid_session_id_error().await {
        panic!("failure: {err}");
    }
}

async fn invalid_session_id_error() -> anyhow::Result<()> {
    let codex_home = TempDir::new()?;
    create_config_with_compatibility_mode(codex_home.path(), "http://localhost:9999", true)?;

    let mut mcp_process =
        McpProcess::new_with_args(codex_home.path(), &["--compatibility-mode"]).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp_process.initialize()).await??;

    // Try to use codex-reply with invalid session ID
    let request_id = mcp_process
        .send_codex_reply_tool_call(
            "invalid-session-id-12345".to_string(),
            "This should fail".to_string(),
        )
        .await?;

    // Should get either an error response or a regular response with an error
    let response_result = mcp_process
        .read_immediate_response(
            RequestId::Integer(request_id),
            IMMEDIATE_RESPONSE_TIMEOUT_MS,
        )
        .await;

    // In compatibility mode, we should get some kind of immediate response
    match response_result {
        Ok(response) => {
            // Check if it's an error embedded in the content
            if let Some(content) = response.result.get("content").and_then(|c| c.as_array()) {
                let text = content
                    .iter()
                    .filter_map(|item| item.get("text").and_then(|t| t.as_str()))
                    .collect::<Vec<_>>()
                    .join(" ");
                assert!(
                    text.to_lowercase().contains("session")
                        || text.to_lowercase().contains("invalid"),
                    "Response should mention session issue: {}",
                    text
                );
            } else {
                panic!("Expected error content but got: {:?}", response.result);
            }
        }
        Err(_) => {
            // Try reading an error message instead
            let error = mcp_process
                .read_stream_until_error_message(RequestId::Integer(request_id))
                .await?;
            assert!(
                error.error.message.contains("session") || error.error.message.contains("Session"),
                "Error message should mention session issue"
            );
        }
    }

    Ok(())
}

/// Test that missing required arguments return proper errors
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_missing_arguments_error() {
    if env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!("Skipping test because it cannot execute when network is disabled");
        return;
    }

    if let Err(err) = missing_arguments_error().await {
        panic!("failure: {err}");
    }
}

async fn missing_arguments_error() -> anyhow::Result<()> {
    let codex_home = TempDir::new()?;
    create_config_with_compatibility_mode(codex_home.path(), "http://localhost:9999", true)?;

    let mut mcp_process =
        McpProcess::new_with_args(codex_home.path(), &["--compatibility-mode"]).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp_process.initialize()).await??;

    // Send codex-reply without session ID
    let request_id = mcp_process
        .send_tool_call("codex-reply", Some(json!({"prompt": "test"})))
        .await?;

    // Should get either an error response or a regular response with an error
    let response_result = mcp_process
        .read_immediate_response(
            RequestId::Integer(request_id),
            IMMEDIATE_RESPONSE_TIMEOUT_MS,
        )
        .await;

    // In compatibility mode, we should get some kind of immediate response
    match response_result {
        Ok(response) => {
            // Check if it's an error embedded in the content
            if let Some(content) = response.result.get("content").and_then(|c| c.as_array()) {
                let text = content
                    .iter()
                    .filter_map(|item| item.get("text").and_then(|t| t.as_str()))
                    .collect::<Vec<_>>()
                    .join(" ");
                assert!(
                    text.to_lowercase().contains("sessionid")
                        || text.to_lowercase().contains("session"),
                    "Response should mention sessionId issue: {}",
                    text
                );
            } else {
                panic!("Expected error content but got: {:?}", response.result);
            }
        }
        Err(_) => {
            // Try reading an error message instead
            let error = mcp_process
                .read_stream_until_error_message(RequestId::Integer(request_id))
                .await?;
            assert!(
                error.error.message.contains("sessionId")
                    || error.error.message.contains("session"),
                "Error message should mention missing sessionId"
            );
        }
    }

    Ok(())
}

/// Test performance: compatibility mode should be faster than async mode
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_performance_sync_vs_async() {
    if env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!("Skipping test because it cannot execute when network is disabled");
        return;
    }

    if let Err(err) = performance_sync_vs_async().await {
        panic!("failure: {err}");
    }
}

async fn performance_sync_vs_async() -> anyhow::Result<()> {
    // Test sync mode performance
    let codex_home = TempDir::new()?;
    create_config_with_compatibility_mode(codex_home.path(), "http://localhost:9999", true)?;

    let mut sync_process =
        McpProcess::new_with_args(codex_home.path(), &["--compatibility-mode"]).await?;
    timeout(DEFAULT_READ_TIMEOUT, sync_process.initialize()).await??;

    let sync_request_id = sync_process
        .send_codex_tool_call(CodexToolCallParam {
            prompt: "Performance test".to_string(),
            ..Default::default()
        })
        .await?;

    let (_, sync_time) = sync_process
        .measure_response_time(RequestId::Integer(sync_request_id))
        .await?;

    // Verify sync mode is fast (should be under 100ms for immediate response)
    println!("Sync mode time: {:?}", sync_time);

    assert!(
        sync_time.as_millis() < 200, // Allow some margin for CI
        "Sync mode should respond quickly ({:?} should be < 200ms)",
        sync_time
    );

    // Note: We can't properly test async mode without a real server,
    // but we've verified that sync mode provides immediate responses

    Ok(())
}

/// Helper function to create config.toml with compatibility mode setting
fn create_config_with_compatibility_mode(
    codex_home: &Path,
    server_uri: &str,
    compatibility_mode: bool,
) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_policy = "read-only"

[mcp]
compatibility_mode = {}

model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{}/v1"
wire_api = "chat"
request_max_retries = 0
stream_max_retries = 0
"#,
            compatibility_mode, server_uri
        ),
    )
}
