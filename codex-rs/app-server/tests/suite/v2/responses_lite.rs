use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use app_test_support::write_models_cache;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_config::types::AuthCredentialsStoreMode;
use core_test_support::responses;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;

#[cfg(any(target_os = "macos", windows))]
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(60);
#[cfg(not(any(target_os = "macos", windows)))]
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn responses_lite_uses_standalone_web_search_and_image_generation() -> Result<()> {
    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_assistant_message("msg-1", "Done"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;
    write_responses_lite_model_cache(codex_home.path())?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("access-chatgpt"),
        AuthCredentialsStoreMode::File,
    )?;

    let mut mcp =
        TestAppServer::new_with_env(codex_home.path(), &[("OPENAI_API_KEY", None)]).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_req = mcp
        .send_thread_start_request(ThreadStartParams::default())
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(thread_resp)?;

    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id,
            input: vec![V2UserInput::Text {
                text: "Use standalone tools".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req)),
    )
    .await??;
    let _turn: TurnStartResponse = to_response::<TurnStartResponse>(turn_resp)?;

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let request = response_mock.single_request();
    request
        .tool_by_name("web", "run")
        .context("Responses Lite should expose standalone web search")?;
    request
        .tool_by_name("image_gen", "imagegen")
        .context("Responses Lite should expose standalone image generation")?;

    let body = request.body_json();
    let tools = body["tools"]
        .as_array()
        .context("Responses request tools should be an array")?;
    assert!(
        !tools.iter().any(|tool| {
            matches!(
                tool.get("type").and_then(Value::as_str),
                Some("web_search" | "image_generation")
            )
        }),
        "Responses Lite should omit hosted Responses tools"
    );

    Ok(())
}

fn write_responses_lite_model_cache(codex_home: &Path) -> Result<()> {
    write_models_cache(codex_home)?;
    let cache_path = codex_home.join("models_cache.json");
    let mut cache: Value = serde_json::from_str(&std::fs::read_to_string(&cache_path)?)?;
    let model = cache["models"]
        .as_array_mut()
        .and_then(|models| models.first_mut())
        .context("models cache should contain at least one model")?;
    model["slug"] = json!("mock-model");
    model["display_name"] = json!("mock-model");
    model["use_responses_lite"] = json!(true);
    model["input_modalities"] = json!(["text", "image"]);
    std::fs::write(cache_path, serde_json::to_string_pretty(&cache)?)?;
    Ok(())
}

fn create_config_toml(codex_home: &Path, server_uri: &str) -> std::io::Result<()> {
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "read-only"
web_search = "live"
model_provider = "openai-custom"
chatgpt_base_url = "{server_uri}"

[features]
standalone_web_search = false
image_generation = false
imagegenext = false

[model_providers.openai-custom]
name = "OpenAI"
base_url = "{server_uri}/api/codex"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
supports_websockets = false
requires_openai_auth = true
"#
        ),
    )
}
