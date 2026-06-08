use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_config::types::AuthCredentialsStoreMode;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

const RESULT: &str = "cG5n";
const TINY_PNG_BYTES: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 11, 73, 68, 65, 84, 120, 156, 99, 96, 0, 2, 0, 0, 5, 0, 1,
    122, 94, 171, 63, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

#[derive(Clone, Copy)]
enum ImagegenTestMode {
    Direct,
    CodeModeOnly,
}

// macOS and Windows Bazel CI can spend tens of seconds starting app-server
// subprocesses or processing test RPCs under load.
#[cfg(any(target_os = "macos", windows))]
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(60);
#[cfg(not(any(target_os = "macos", windows)))]
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn standalone_image_generation_returns_saved_path_hint_to_model() -> Result<()> {
    let call_id = "image-run-1";
    let server = responses::start_mock_server().await;
    mount_image_response(&server).await;

    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_function_call_with_namespace(
                    call_id,
                    "image_gen",
                    "imagegen",
                    &json!({
                        "prompt": "paint a blue whale",
                    })
                    .to_string(),
                ),
                responses::ev_completed("resp-1"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("msg-1", "Done"),
                responses::ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), ImagegenTestMode::Direct)?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("access-chatgpt"),
        AuthCredentialsStoreMode::File,
    )?;

    let mut mcp =
        TestAppServer::new_with_env(codex_home.path(), &[("OPENAI_API_KEY", None)]).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;
    start_image_generation_turn(&mut mcp).await?;

    let completed = timeout(
        DEFAULT_READ_TIMEOUT,
        wait_for_image_generation_completed(&mut mcp),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let ThreadItem::ImageGeneration {
        status,
        revised_prompt,
        result,
        saved_path: Some(saved_path),
        ..
    } = completed.item
    else {
        panic!("expected completed image generation item with saved path");
    };
    assert_eq!(status, "completed");
    assert_eq!(revised_prompt.as_deref(), Some("paint a blue whale"));
    assert_eq!(result, RESULT);
    assert_eq!(std::fs::read(&saved_path)?, b"png");

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 2);
    let output = requests[1].function_call_output(call_id);
    assert_eq!(
        output["output"][0],
        json!({
            "type": "input_image",
            "image_url": format!("data:image/png;base64,{RESULT}"),
            "detail": "high",
        })
    );
    let output_hint = output["output"][1]["text"]
        .as_str()
        .context("image output should include model-visible path hint")?;
    assert!(
        output_hint.contains(&saved_path.display().to_string()),
        "output hint should identify the path core saved"
    );
    assert!(
        !requests[1]
            .message_input_texts("developer")
            .iter()
            .any(|text| text.contains("Generated images are saved to")),
        "standalone image generation should not emit the legacy developer-message hint"
    );

    Ok(())
}

#[tokio::test]
async fn standalone_image_edit_uses_attached_model_visible_image() -> Result<()> {
    let edit_request = run_image_edit_test(|codex_home| {
        let image_path = codex_home.join("attached.png");
        std::fs::write(&image_path, TINY_PNG_BYTES)?;
        Ok((
            json!({
                "prompt": "add a red hat",
                "referenced_image_paths": [image_path.display().to_string()],
            }),
            vec![
                V2UserInput::Text {
                    text: "Edit the attached image".to_string(),
                    text_elements: Vec::new(),
                },
                V2UserInput::LocalImage {
                    path: image_path,
                    detail: None,
                },
            ],
        ))
    })
    .await?;
    assert_eq!(edit_request["prompt"], "add a red hat");
    assert!(
        edit_request["images"][0]["image_url"]
            .as_str()
            .is_some_and(|image_url| image_url.starts_with("data:image/png;base64,"))
    );

    Ok(())
}

#[tokio::test]
async fn standalone_image_edit_uses_recent_pathless_image() -> Result<()> {
    let image_url = "https://example.com/reference.png";
    let edit_request = run_image_edit_test(|_| {
        Ok((
            json!({
                "prompt": "add a red hat",
                "num_last_images_to_include": 1,
            }),
            vec![
                V2UserInput::Text {
                    text: "Edit the attached image".to_string(),
                    text_elements: Vec::new(),
                },
                V2UserInput::Image {
                    url: image_url.to_string(),
                    detail: None,
                },
            ],
        ))
    })
    .await?;
    assert_eq!(edit_request["prompt"], "add a red hat");
    assert_eq!(edit_request["images"][0]["image_url"], image_url);

    Ok(())
}

#[tokio::test]
async fn standalone_image_generation_is_exposed_in_code_mode_only() -> Result<()> {
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
    create_config_toml(
        codex_home.path(),
        &server.uri(),
        ImagegenTestMode::CodeModeOnly,
    )?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("access-chatgpt"),
        AuthCredentialsStoreMode::File,
    )?;

    let mut mcp =
        TestAppServer::new_with_env(codex_home.path(), &[("OPENAI_API_KEY", None)]).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;
    start_image_generation_turn(&mut mcp).await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    assert!(
        response_mock
            .single_request()
            .body_contains_text("image_gen__imagegen")
    );

    Ok(())
}

#[cfg(not(windows))]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn standalone_image_generation_is_callable_from_code_mode_only() -> Result<()> {
    let call_id = "code-mode-image-run-1";
    let server = responses::start_mock_server().await;
    mount_image_response(&server).await;

    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_custom_tool_call(
                    call_id,
                    "exec",
                    r#"
const result = await tools.image_gen__imagegen({
  prompt: "paint a blue whale",
});
generatedImage(result);
"#,
                ),
                responses::ev_completed("resp-1"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("msg-1", "Done"),
                responses::ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let codex_home = TempDir::new()?;
    create_config_toml(
        codex_home.path(),
        &server.uri(),
        ImagegenTestMode::CodeModeOnly,
    )?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("access-chatgpt"),
        AuthCredentialsStoreMode::File,
    )?;

    let mut mcp =
        TestAppServer::new_with_env(codex_home.path(), &[("OPENAI_API_KEY", None)]).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;
    start_image_generation_turn(&mut mcp).await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].body_contains_text("image_gen__imagegen"));
    let output = requests[1].custom_tool_call_output(call_id);
    assert_eq!(
        output["output"][1],
        json!({
            "type": "input_image",
            "image_url": format!("data:image/png;base64,{RESULT}"),
            "detail": "high",
        })
    );
    assert!(
        output["output"][2]["text"]
            .as_str()
            .is_some_and(|text| text.contains("Generated images are saved"))
    );
    assert_eq!(output["output"].as_array().map(Vec::len), Some(3));

    Ok(())
}

async fn start_image_generation_turn(mcp: &mut TestAppServer) -> Result<()> {
    start_turn(
        mcp,
        vec![V2UserInput::Text {
            text: "Generate an image".to_string(),
            text_elements: Vec::new(),
        }],
    )
    .await
}

async fn run_image_edit_test(
    input: impl FnOnce(&Path) -> Result<(serde_json::Value, Vec<V2UserInput>)>,
) -> Result<serde_json::Value> {
    let call_id = "image-edit-1";
    let server = responses::start_mock_server().await;
    mount_image_edit_response(&server).await;

    let codex_home = TempDir::new()?;
    let (arguments, input) = input(codex_home.path())?;
    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_function_call_with_namespace(
                    call_id,
                    "image_gen",
                    "imagegen",
                    &arguments.to_string(),
                ),
                responses::ev_completed("resp-1"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("msg-1", "Done"),
                responses::ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    create_config_toml(codex_home.path(), &server.uri(), ImagegenTestMode::Direct)?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("access-chatgpt"),
        AuthCredentialsStoreMode::File,
    )?;

    let mut mcp =
        TestAppServer::new_with_env(codex_home.path(), &[("OPENAI_API_KEY", None)]).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;
    start_turn(&mut mcp, input).await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        wait_for_image_generation_completed(&mut mcp),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    assert_eq!(response_mock.requests().len(), 2);
    let requests = server
        .received_requests()
        .await
        .context("failed to fetch received requests")?;
    Ok(requests
        .iter()
        .find(|request| request.url.path() == "/api/codex/images/edits")
        .context("image edit request should be sent")?
        .body_json::<serde_json::Value>()?)
}

async fn start_turn(mcp: &mut TestAppServer, input: Vec<V2UserInput>) -> Result<()> {
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
            client_user_message_id: None,
            input,
            ..Default::default()
        })
        .await?;
    let turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req)),
    )
    .await??;
    let _turn: TurnStartResponse = to_response::<TurnStartResponse>(turn_resp)?;

    Ok(())
}

async fn wait_for_image_generation_completed(
    mcp: &mut TestAppServer,
) -> Result<ItemCompletedNotification> {
    loop {
        let notification = mcp
            .read_stream_until_notification_message("item/completed")
            .await?;
        let completed: ItemCompletedNotification = serde_json::from_value(
            notification
                .params
                .context("item/completed notification should include params")?,
        )?;
        if matches!(&completed.item, ThreadItem::ImageGeneration { .. }) {
            return Ok(completed);
        }
    }
}

async fn mount_image_response(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/api/codex/images/generations"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "created": 1,
            "data": [{"b64_json": RESULT}],
        })))
        .expect(1)
        .mount(server)
        .await;
}

async fn mount_image_edit_response(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/api/codex/images/edits"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "created": 1,
            "data": [{"b64_json": RESULT}],
        })))
        .expect(1)
        .mount(server)
        .await;
}

fn create_config_toml(
    codex_home: &Path,
    server_uri: &str,
    mode: ImagegenTestMode,
) -> std::io::Result<()> {
    let code_mode_only = match mode {
        ImagegenTestMode::Direct => "",
        ImagegenTestMode::CodeModeOnly => "code_mode_only = true",
    };
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "read-only"
model_provider = "openai-custom"
chatgpt_base_url = "{server_uri}"

[features]
imagegenext = true
{code_mode_only}

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
