use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::write_chatgpt_auth;
use codex_app_server_protocol::ImageGenerationFailure;
use codex_app_server_protocol::ImageGenerationItem;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_config::types::AuthCredentialsStoreMode;
use codex_features::Feature;
use core_test_support::responses;
use core_test_support::skip_if_remote;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

const RESULT: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";
const TINY_PNG_BYTES: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 240, 31, 0,
    5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];
const TINY_PNG_DATA_URL: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";

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

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;
    let turn_id = start_image_generation_turn(
        &mut mcp,
        ThreadStartParams {
            service_name: Some("chatgpt_cca".to_string()),
            ..Default::default()
        },
    )
    .await?;

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

    let ThreadItem::ImageGeneration(ImageGenerationItem {
        status,
        revised_prompt,
        result,
        transparent_background,
        saved_path: Some(saved_path),
        ..
    }) = completed.item
    else {
        panic!("expected completed image generation item with saved path");
    };
    assert_eq!(status, "completed");
    assert_eq!(revised_prompt.as_deref(), Some("paint a blue whale"));
    assert_eq!(result, RESULT);
    assert_eq!(transparent_background, Some(false));
    assert_eq!(std::fs::read(&saved_path)?, TINY_PNG_BYTES);

    let image_request = server
        .received_requests()
        .await
        .context("failed to fetch received requests")?
        .into_iter()
        .find(|request| request.url.path() == "/api/codex/images/generations")
        .context("image generation request should be sent")?;
    assert_eq!(
        image_request
            .headers
            .get("originator")
            .context("standalone image generation should include the thread originator")?
            .to_str()
            .context("standalone image generation originator should be valid ASCII")?,
        "chatgpt_cca"
    );
    assert_image_turn_id_header(&image_request, &turn_id)?;

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
        "output hint should identify the path the extension saved"
    );
    assert!(
        output_hint.contains("already displayed to the user"),
        "output hint should tell the model not to repeat the generated image: {output_hint}"
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
async fn transparent_image_preserves_output_metadata_and_persisted_history() -> Result<()> {
    let call_id = "transparent-image-run-1";
    let server = responses::start_mock_server().await;
    mount_image_response_with_background(&server, "transparent").await;
    responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_function_call_with_namespace(
                    call_id,
                    "image_gen",
                    "imagegen",
                    &json!({"prompt": "a blue whale on a transparent background"}).to_string(),
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
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;
    start_image_generation_turn(&mut mcp, ThreadStartParams::default()).await?;

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
    let thread_id = completed.thread_id.clone();
    let ThreadItem::ImageGeneration(ImageGenerationItem {
        status,
        result,
        transparent_background,
        ..
    }) = completed.item
    else {
        panic!("expected completed image-generation item");
    };
    assert_eq!(status, "completed");
    assert_eq!(result, RESULT);
    assert_eq!(transparent_background, Some(true));

    drop(mcp);
    let mut resumed = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;
    let read_id = resumed
        .send_thread_read_request(ThreadReadParams {
            thread_id: thread_id.clone(),
            include_turns: true,
        })
        .await?;
    let ThreadReadResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, resumed.read_response(read_id)).await??;
    let persisted_image = thread
        .turns
        .iter()
        .flat_map(|turn| turn.items.iter())
        .find_map(|item| match item {
            ThreadItem::ImageGeneration(item) => Some(item),
            _ => None,
        })
        .context("persisted legacy history should contain the generated image")?;
    assert_eq!(persisted_image.transparent_background, Some(true));

    let resume_id = resumed
        .send_thread_resume_request(ThreadResumeParams {
            thread_id,
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, resumed.read_response(resume_id)).await??;
    let resumed_image = thread
        .turns
        .iter()
        .flat_map(|turn| turn.items.iter())
        .find_map(|item| match item {
            ThreadItem::ImageGeneration(item) => Some(item),
            _ => None,
        })
        .context("resumed legacy history should contain the generated image")?;
    assert_eq!(resumed_image.transparent_background, Some(true));

    Ok(())
}

#[tokio::test]
async fn automatic_image_background_preserves_unknown_transparency() -> Result<()> {
    let call_id = "automatic-image-run-1";
    let server = responses::start_mock_server().await;
    mount_image_response_with_background(&server, "auto").await;
    responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_function_call_with_namespace(
                    call_id,
                    "image_gen",
                    "imagegen",
                    &json!({"prompt": "paint a blue whale"}).to_string(),
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
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;
    start_image_generation_turn(&mut mcp, ThreadStartParams::default()).await?;
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
    let ThreadItem::ImageGeneration(image) = completed.item else {
        panic!("expected completed image-generation item");
    };
    assert_eq!(image.transparent_background, None);
    let value = serde_json::to_value(&image)?;
    assert_eq!(
        value.get("transparentBackground"),
        Some(&serde_json::Value::Null),
        "v2 image-generation items must always include nullable transparency metadata"
    );
    Ok(())
}

#[tokio::test]
async fn standalone_image_generation_failure_emits_terminal_item() -> Result<()> {
    let call_id = "image-run-failed";
    let server = responses::start_mock_server().await;
    Mock::given(method("POST"))
        .and(path("/api/codex/images/generations"))
        .respond_with(ResponseTemplate::new(500).set_body_string("image backend failed"))
        .expect(1)
        .mount(&server)
        .await;
    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_function_call_with_namespace(
                    call_id,
                    "image_gen",
                    "imagegen",
                    &json!({"prompt": "paint a blue whale"}).to_string(),
                ),
                responses::ev_completed("resp-1"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("msg-1", "I could not generate the image."),
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
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;
    start_image_generation_turn(&mut mcp, ThreadStartParams::default()).await?;

    let completed = timeout(
        DEFAULT_READ_TIMEOUT,
        wait_for_image_generation_completed(&mut mcp),
    )
    .await??;
    assert_eq!(
        completed.item,
        ThreadItem::ImageGeneration(ImageGenerationItem {
            id: call_id.to_string(),
            status: "failed".to_string(),
            revised_prompt: Some("paint a blue whale".to_string()),
            result: String::new(),
            transparent_background: None,
            failure: None,
            saved_path: None,
        })
    );

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    let requests = response_mock.requests();
    assert_eq!(requests.len(), 2);
    let (output, _) = requests[1]
        .function_call_output_content_and_success(call_id)
        .context("image generation function output should be present")?;
    assert_eq!(
        output.as_deref(),
        Some(
            "image generation failed: http 500 Internal Server Error: Some(\"image backend failed\")"
        )
    );

    Ok(())
}

#[tokio::test]
async fn image_generation_usage_limit_preserves_correlated_failure_metadata() -> Result<()> {
    let call_id = "image-run-limited";
    let reset_at = 1_786_150_800;
    let server = responses::start_mock_server().await;
    Mock::given(method("POST"))
        .and(path("/api/codex/images/generations"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("x-codex-active-limit", "image_gen")
                .insert_header("x-image-gen-primary-used-percent", "100")
                .insert_header("x-image-gen-primary-window-minutes", "1440")
                .insert_header("x-image-gen-primary-reset-at", reset_at.to_string())
                .set_body_json(json!({
                    "error": {
                        "type": "usage_limit_reached",
                        "message": "image limit reached",
                        "resets_at": reset_at,
                        "plan_type": "plus"
                    }
                })),
        )
        .expect(1)
        .mount(&server)
        .await;
    responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_function_call_with_namespace(
                    call_id,
                    "image_gen",
                    "imagegen",
                    &json!({"prompt": "paint a blue whale"}).to_string(),
                ),
                responses::ev_completed("resp-1"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("msg-1", "The image limit was reached."),
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
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;
    start_image_generation_turn(&mut mcp, ThreadStartParams::default()).await?;

    let completed = timeout(
        DEFAULT_READ_TIMEOUT,
        wait_for_image_generation_completed(&mut mcp),
    )
    .await??;
    let thread_id = completed.thread_id.clone();
    let ThreadItem::ImageGeneration(image) = completed.item else {
        panic!("expected failed image-generation item");
    };
    assert_eq!(image.status, "failed");
    assert_eq!(
        image.failure,
        Some(ImageGenerationFailure::UsageLimitExceeded {
            limit_id: "image_gen".to_string(),
            resets_at: Some(reset_at),
        })
    );

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    let read_id = mcp
        .send_thread_read_request(ThreadReadParams {
            thread_id: thread_id.clone(),
            include_turns: true,
        })
        .await?;
    let ThreadReadResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(read_id)).await??;
    let persisted_failure = thread
        .turns
        .iter()
        .flat_map(|turn| turn.items.iter())
        .find_map(|item| match item {
            ThreadItem::ImageGeneration(item) => item.failure.as_ref(),
            _ => None,
        });
    assert_eq!(
        persisted_failure,
        Some(&ImageGenerationFailure::UsageLimitExceeded {
            limit_id: "image_gen".to_string(),
            resets_at: Some(reset_at),
        })
    );

    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id,
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(resume_id)).await??;
    let resumed_failure = thread
        .turns
        .iter()
        .flat_map(|turn| turn.items.iter())
        .find_map(|item| match item {
            ThreadItem::ImageGeneration(item) => item.failure.as_ref(),
            _ => None,
        });
    assert_eq!(
        resumed_failure,
        Some(&ImageGenerationFailure::UsageLimitExceeded {
            limit_id: "image_gen".to_string(),
            resets_at: Some(reset_at),
        })
    );

    Ok(())
}

#[tokio::test]
async fn standalone_image_edit_uses_attached_model_visible_image() -> Result<()> {
    skip_if_remote!(
        Ok(()),
        "remote executors use different imagegen storage approaches, so host-local image paths are unavailable"
    );

    let (edit_request, _) = run_image_edit_test(|codex_home| {
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
    assert_eq!(edit_request["images"][0]["image_url"], TINY_PNG_DATA_URL);

    Ok(())
}

#[tokio::test]
async fn transparent_image_edit_preserves_metadata_and_recent_pathless_image() -> Result<()> {
    let image_url = TINY_PNG_DATA_URL;
    let (edit_request, completed_image) = run_image_edit_test(|_| {
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
    assert_eq!(completed_image.transparent_background, Some(true));

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

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;
    start_image_generation_turn(&mut mcp, ThreadStartParams::default()).await?;
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

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;
    start_image_generation_turn(&mut mcp, ThreadStartParams::default()).await?;
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

async fn start_image_generation_turn(
    mcp: &mut TestAppServer,
    thread_start_params: ThreadStartParams,
) -> Result<String> {
    start_turn(
        mcp,
        thread_start_params,
        vec![V2UserInput::Text {
            text: "Generate an image".to_string(),
            text_elements: Vec::new(),
        }],
    )
    .await
}

async fn run_image_edit_test(
    input: impl FnOnce(&Path) -> Result<(serde_json::Value, Vec<V2UserInput>)>,
) -> Result<(serde_json::Value, ImageGenerationItem)> {
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

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;
    let turn_id = start_turn(
        &mut mcp,
        ThreadStartParams {
            service_name: Some("chatgpt_cca".to_string()),
            ..Default::default()
        },
        input,
    )
    .await?;
    let completed = timeout(
        DEFAULT_READ_TIMEOUT,
        wait_for_image_generation_completed(&mut mcp),
    )
    .await??;
    let ThreadItem::ImageGeneration(completed_image) = completed.item else {
        panic!("expected completed image-generation item");
    };
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
    let image_request = requests
        .iter()
        .find(|request| request.url.path() == "/api/codex/images/edits")
        .context("image edit request should be sent")?;
    assert_eq!(
        image_request
            .headers
            .get("originator")
            .context("standalone image edit should include the thread originator")?
            .to_str()
            .context("standalone image edit originator should be valid ASCII")?,
        "chatgpt_cca"
    );
    assert_image_turn_id_header(image_request, &turn_id)?;
    Ok((
        image_request.body_json::<serde_json::Value>()?,
        completed_image,
    ))
}

fn assert_image_turn_id_header(request: &wiremock::Request, expected_turn_id: &str) -> Result<()> {
    let turn_id = request
        .headers
        .get("x-codex-image-turn-id")
        .context("image request should include the current turn id")?
        .to_str()
        .context("image turn id should be valid ASCII")?;
    uuid::Uuid::parse_str(turn_id).context("image turn id should be a UUID")?;
    assert_eq!(turn_id, expected_turn_id);
    Ok(())
}

async fn start_turn(
    mcp: &mut TestAppServer,
    thread_start_params: ThreadStartParams,
    input: Vec<V2UserInput>,
) -> Result<String> {
    let thread_req = mcp
        .send_thread_start_request_with_auto_env(thread_start_params)
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(thread_req)).await??;

    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id,
            client_user_message_id: None,
            input,
            ..Default::default()
        })
        .await?;
    let TurnStartResponse { turn } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(turn_req)).await??;

    Ok(turn.id)
}

async fn wait_for_image_generation_completed(
    mcp: &mut TestAppServer,
) -> Result<ItemCompletedNotification> {
    loop {
        let completed: ItemCompletedNotification = mcp.read_notification("item/completed").await?;
        if matches!(&completed.item, ThreadItem::ImageGeneration(_)) {
            return Ok(completed);
        }
    }
}

async fn mount_image_response(server: &MockServer) {
    mount_image_response_with_background(server, "opaque").await;
}

async fn mount_image_response_with_background(server: &MockServer, background: &str) {
    Mock::given(method("POST"))
        .and(path("/api/codex/images/generations"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "created": 1,
            "background": background,
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
            "background": "transparent",
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
    let mut config = MockResponsesConfig::new(server_uri)
        .with_model_provider("openai-custom")
        .with_provider_name("OpenAI")
        .with_provider_base_url(&format!("{server_uri}/api/codex"))
        .with_root_config(&format!("chatgpt_base_url = \"{server_uri}\""))
        .with_provider_config("supports_websockets = false\nrequires_openai_auth = true");
    if matches!(mode, ImagegenTestMode::CodeModeOnly) {
        config = config.enable_feature(Feature::CodeModeOnly);
    }
    config.write(codex_home)
}
