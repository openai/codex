#![cfg(not(target_os = "windows"))]

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_core::CodexAuth;
use codex_exec_server::CreateDirectoryOptions;
use codex_features::Feature;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::InputModality;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;
use codex_protocol::openai_models::TruncationPolicyConfig;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_custom_tool_call;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_models_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_with_timeout;
use image::DynamicImage;
use image::GenericImageView;
use image::ImageBuffer;
use image::Rgba;
use image::load_from_memory;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::io::Cursor;
use std::path::Path;
use std::path::PathBuf;
use tokio::time::Duration;
use wiremock::BodyPrintLimit;
use wiremock::MockServer;
#[cfg(not(debug_assertions))]
use wiremock::ResponseTemplate;
#[cfg(not(debug_assertions))]
use wiremock::matchers::body_string_contains;

fn image_messages(body: &Value) -> Vec<&Value> {
    body.get("input")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter(|item| {
                    item.get("type").and_then(Value::as_str) == Some("message")
                        && item
                            .get("content")
                            .and_then(Value::as_array)
                            .map(|content| {
                                content.iter().any(|span| {
                                    span.get("type").and_then(Value::as_str) == Some("input_image")
                                })
                            })
                            .unwrap_or(false)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn find_image_message(body: &Value) -> Option<&Value> {
    image_messages(body).into_iter().next()
}

fn absolute_path(path: &Path) -> anyhow::Result<codex_utils_absolute_path::AbsolutePathBuf> {
    codex_utils_absolute_path::AbsolutePathBuf::try_from(path.to_path_buf())
        .map_err(|err| anyhow::anyhow!("invalid absolute path {}: {err}", path.display()))
}

fn png_bytes(width: u32, height: u32, rgba: [u8; 4]) -> anyhow::Result<Vec<u8>> {
    let image = ImageBuffer::from_pixel(width, height, Rgba(rgba));
    let mut cursor = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image).write_to(&mut cursor, image::ImageFormat::Png)?;
    Ok(cursor.into_inner())
}

async fn create_workspace_directory(test: &TestCodex, rel_path: &str) -> anyhow::Result<PathBuf> {
    let abs_path = test.config.cwd.join(rel_path);
    test.fs()
        .create_directory(
            &absolute_path(&abs_path)?,
            CreateDirectoryOptions { recursive: true },
        )
        .await?;
    Ok(abs_path)
}

async fn write_workspace_file(
    test: &TestCodex,
    rel_path: &str,
    contents: Vec<u8>,
) -> anyhow::Result<PathBuf> {
    let abs_path = test.config.cwd.join(rel_path);
    if let Some(parent) = abs_path.parent() {
        test.fs()
            .create_directory(
                &absolute_path(parent)?,
                CreateDirectoryOptions { recursive: true },
            )
            .await?;
    }
    test.fs()
        .write_file(&absolute_path(&abs_path)?, contents)
        .await?;
    Ok(abs_path)
}

async fn write_workspace_png(
    test: &TestCodex,
    rel_path: &str,
    width: u32,
    height: u32,
    rgba: [u8; 4],
) -> anyhow::Result<PathBuf> {
    write_workspace_file(test, rel_path, png_bytes(width, height, rgba)?).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_turn_with_local_image_attaches_image() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex();
    let test = builder.build_remote_aware(&server).await?;
    let TestCodex {
        codex,
        config,
        session_configured,
        ..
    } = &test;

    let original_width = 2304;
    let original_height = 864;
    let local_image_dir = tempfile::tempdir()?;
    let abs_path = local_image_dir.path().join("example.png");
    let image = ImageBuffer::from_pixel(original_width, original_height, Rgba([20u8, 40, 60, 255]));
    image.save(&abs_path)?;

    let response = sse(vec![
        ev_response_created("resp-1"),
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-1"),
    ]);
    let mock = responses::mount_sse_once(&server, response).await;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::LocalImage {
                path: abs_path.clone(),
            }],
            final_output_json_schema: None,
            cwd: config.cwd.clone(),
            approval_policy: AskForApproval::Never,
            approvals_reviewer: None,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    wait_for_event_with_timeout(
        codex,
        |event| matches!(event, EventMsg::TurnComplete(_)),
        // Empirically, image attachment can be slow under Bazel/RBE.
        Duration::from_secs(10),
    )
    .await;

    let body = mock.single_request().body_json();
    let image_message =
        find_image_message(&body).expect("pending input image message not included in request");
    let image_url = image_message
        .get("content")
        .and_then(Value::as_array)
        .and_then(|content| {
            content.iter().find_map(|span| {
                if span.get("type").and_then(Value::as_str) == Some("input_image") {
                    span.get("image_url").and_then(Value::as_str)
                } else {
                    None
                }
            })
        })
        .expect("image_url present");

    let (prefix, encoded) = image_url
        .split_once(',')
        .expect("image url contains data prefix");
    assert_eq!(prefix, "data:image/png;base64");

    let decoded = BASE64_STANDARD
        .decode(encoded)
        .expect("image data decodes from base64 for request");
    let resized = load_from_memory(&decoded).expect("load resized image");
    let (width, height) = resized.dimensions();
    assert!(width <= 2048);
    assert!(height <= 768);
    assert!(width < original_width);
    assert!(height < original_height);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn view_image_tool_attaches_local_image() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex();
    let test = builder.build_remote_aware(&server).await?;
    let TestCodex {
        codex,
        session_configured,
        config,
        ..
    } = &test;
    let cwd = config.cwd.clone();

    let rel_path = "assets/example.png";
    let abs_path = cwd.join(rel_path);
    let original_width = 2304;
    let original_height = 864;
    write_workspace_png(
        &test,
        rel_path,
        original_width,
        original_height,
        [255u8, 0, 0, 255],
    )
    .await?;

    let call_id = "view-image-call";
    let arguments = serde_json::json!({ "path": rel_path }).to_string();

    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_function_call(call_id, "view_image", &arguments),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once(&server, first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);
    let mock = responses::mount_sse_once(&server, second_response).await;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "please add the screenshot".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: cwd.clone(),
            approval_policy: AskForApproval::Never,
            approvals_reviewer: None,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    let mut tool_event = None;
    wait_for_event_with_timeout(
        codex,
        |event| match event {
            EventMsg::ViewImageToolCall(_) => {
                tool_event = Some(event.clone());
                false
            }
            EventMsg::TurnComplete(_) => true,
            _ => false,
        },
        // Empirically, we have seen this run slow when run under
        // Bazel on arm Linux.
        Duration::from_secs(10),
    )
    .await;

    let tool_event = match tool_event.expect("view image tool event emitted") {
        EventMsg::ViewImageToolCall(event) => event,
        _ => unreachable!("stored event must be ViewImageToolCall"),
    };
    assert_eq!(tool_event.call_id, call_id);
    assert_eq!(tool_event.path, abs_path);

    let req = mock.single_request();
    let body = req.body_json();
    assert!(
        find_image_message(&body).is_none(),
        "view_image tool should not inject a separate image message"
    );

    let function_output = req.function_call_output(call_id);
    let output_items = function_output
        .get("output")
        .and_then(Value::as_array)
        .expect("function_call_output should be a content item array");
    assert_eq!(
        output_items.len(),
        1,
        "view_image should return only the image content item (no tag/label text)"
    );
    assert_eq!(
        output_items[0].get("type").and_then(Value::as_str),
        Some("input_image"),
        "view_image should return only an input_image content item"
    );
    let image_url = output_items[0]
        .get("image_url")
        .and_then(Value::as_str)
        .expect("image_url present");

    let (prefix, encoded) = image_url
        .split_once(',')
        .expect("image url contains data prefix");
    assert_eq!(prefix, "data:image/png;base64");

    let decoded = BASE64_STANDARD
        .decode(encoded)
        .expect("image data decodes from base64 for request");
    let resized = load_from_memory(&decoded).expect("load resized image");
    let (resized_width, resized_height) = resized.dimensions();
    assert!(resized_width <= 2048);
    assert!(resized_height <= 768);
    assert!(resized_width < original_width);
    assert!(resized_height < original_height);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn view_image_tool_can_preserve_original_resolution_when_requested_on_gpt5_3_codex()
-> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex()
        .with_model("gpt-5.3-codex")
        .with_config(|config| {
            config
                .features
                .enable(Feature::ImageDetailOriginal)
                .expect("test config should allow feature update");
        });
    let test = builder.build_remote_aware(&server).await?;
    let TestCodex {
        codex,
        config,
        session_configured,
        ..
    } = &test;

    let rel_path = "assets/original-example.png";
    let original_width = 2304;
    let original_height = 864;
    write_workspace_png(
        &test,
        rel_path,
        original_width,
        original_height,
        [0u8, 80, 255, 255],
    )
    .await?;

    let call_id = "view-image-original";
    let arguments = serde_json::json!({ "path": rel_path, "detail": "original" }).to_string();

    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_function_call(call_id, "view_image", &arguments),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once(&server, first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);
    let mock = responses::mount_sse_once(&server, second_response).await;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "please add the original screenshot".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: config.cwd.clone(),
            approval_policy: AskForApproval::Never,
            approvals_reviewer: None,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            service_tier: None,
            summary: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    wait_for_event_with_timeout(
        codex,
        |event| matches!(event, EventMsg::TurnComplete(_)),
        Duration::from_secs(10),
    )
    .await;

    let req = mock.single_request();
    let function_output = req.function_call_output(call_id);
    let output_items = function_output
        .get("output")
        .and_then(Value::as_array)
        .expect("function_call_output should be a content item array");
    assert_eq!(output_items.len(), 1);
    assert_eq!(
        output_items[0].get("detail").and_then(Value::as_str),
        Some("original")
    );
    let image_url = output_items[0]
        .get("image_url")
        .and_then(Value::as_str)
        .expect("image_url present");

    let (_, encoded) = image_url
        .split_once(',')
        .expect("image url contains data prefix");
    let decoded = BASE64_STANDARD
        .decode(encoded)
        .expect("image data decodes from base64 for request");
    let preserved = load_from_memory(&decoded).expect("load preserved image");
    let (width, height) = preserved.dimensions();
    assert_eq!(width, original_width);
    assert_eq!(height, original_height);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn view_image_tool_errors_clearly_for_unsupported_detail_values() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex()
        .with_model("gpt-5.3-codex")
        .with_config(|config| {
            config
                .features
                .enable(Feature::ImageDetailOriginal)
                .expect("test config should allow feature update");
        });
    let test = builder.build_remote_aware(&server).await?;
    let TestCodex {
        codex,
        config,
        session_configured,
        ..
    } = &test;

    let rel_path = "assets/unsupported-detail.png";
    write_workspace_png(&test, rel_path, 256, 128, [0u8, 80, 255, 255]).await?;

    let call_id = "view-image-unsupported-detail";
    let arguments = serde_json::json!({ "path": rel_path, "detail": "low" }).to_string();

    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_function_call(call_id, "view_image", &arguments),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once(&server, first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);
    let mock = responses::mount_sse_once(&server, second_response).await;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "please attach the image at low detail".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: config.cwd.clone(),
            approval_policy: AskForApproval::Never,
            approvals_reviewer: None,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            service_tier: None,
            summary: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    wait_for_event(codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    let req = mock.single_request();
    let body_with_tool_output = req.body_json();
    let output_text = req
        .function_call_output_content_and_success(call_id)
        .and_then(|(content, _)| content)
        .expect("output text present");
    assert_eq!(
        output_text,
        "view_image.detail only supports `original`; omit `detail` for default resized behavior, got `low`"
    );

    assert!(
        find_image_message(&body_with_tool_output).is_none(),
        "unsupported detail values should not produce an input_image message"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn view_image_tool_treats_null_detail_as_omitted() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex()
        .with_model("gpt-5.3-codex")
        .with_config(|config| {
            config
                .features
                .enable(Feature::ImageDetailOriginal)
                .expect("test config should allow feature update");
        });
    let test = builder.build_remote_aware(&server).await?;
    let TestCodex {
        codex,
        config,
        session_configured,
        ..
    } = &test;

    let rel_path = "assets/null-detail.png";
    let original_width = 2304;
    let original_height = 864;
    write_workspace_png(
        &test,
        rel_path,
        original_width,
        original_height,
        [0u8, 80, 255, 255],
    )
    .await?;

    let call_id = "view-image-null-detail";
    let arguments = serde_json::json!({ "path": rel_path, "detail": null }).to_string();

    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_function_call(call_id, "view_image", &arguments),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once(&server, first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);
    let mock = responses::mount_sse_once(&server, second_response).await;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "please attach the image with a null detail".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: config.cwd.clone(),
            approval_policy: AskForApproval::Never,
            approvals_reviewer: None,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            service_tier: None,
            summary: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    wait_for_event(codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    let req = mock.single_request();
    let function_output = req.function_call_output(call_id);
    let output_items = function_output
        .get("output")
        .and_then(Value::as_array)
        .expect("function_call_output should be a content item array");
    assert_eq!(output_items.len(), 1);
    assert_eq!(output_items[0].get("detail"), None);
    let image_url = output_items[0]
        .get("image_url")
        .and_then(Value::as_str)
        .expect("image_url present");

    let (_, encoded) = image_url
        .split_once(',')
        .expect("image url contains data prefix");
    let decoded = BASE64_STANDARD
        .decode(encoded)
        .expect("image data decodes from base64 for request");
    let resized = load_from_memory(&decoded).expect("load resized image");
    let (width, height) = resized.dimensions();
    assert!(width <= 2048);
    assert!(height <= 768);
    assert!(width < original_width);
    assert!(height < original_height);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn view_image_tool_resizes_when_model_lacks_original_detail_support() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("gpt-5.2").with_config(|config| {
        config
            .features
            .enable(Feature::ImageDetailOriginal)
            .expect("test config should allow feature update");
    });
    let test = builder.build_remote_aware(&server).await?;
    let TestCodex {
        codex,
        config,
        session_configured,
        ..
    } = &test;

    let rel_path = "assets/original-example-lower-model.png";
    let original_width = 2304;
    let original_height = 864;
    write_workspace_png(
        &test,
        rel_path,
        original_width,
        original_height,
        [0u8, 80, 255, 255],
    )
    .await?;

    let call_id = "view-image-original-lower-model";
    let arguments = serde_json::json!({ "path": rel_path }).to_string();

    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_function_call(call_id, "view_image", &arguments),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once(&server, first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);
    let mock = responses::mount_sse_once(&server, second_response).await;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "please add the screenshot".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: config.cwd.clone(),
            approval_policy: AskForApproval::Never,
            approvals_reviewer: None,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            service_tier: None,
            summary: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    wait_for_event_with_timeout(
        codex,
        |event| matches!(event, EventMsg::TurnComplete(_)),
        Duration::from_secs(10),
    )
    .await;

    let req = mock.single_request();
    let function_output = req.function_call_output(call_id);
    let output_items = function_output
        .get("output")
        .and_then(Value::as_array)
        .expect("function_call_output should be a content item array");
    assert_eq!(output_items.len(), 1);
    assert_eq!(output_items[0].get("detail"), None);

    let image_url = output_items[0]
        .get("image_url")
        .and_then(Value::as_str)
        .expect("image_url present");

    let (prefix, encoded) = image_url
        .split_once(',')
        .expect("image url contains data prefix");
    assert_eq!(prefix, "data:image/png;base64");

    let decoded = BASE64_STANDARD
        .decode(encoded)
        .expect("image data decodes from base64 for request");
    let resized = load_from_memory(&decoded).expect("load resized image");
    let (resized_width, resized_height) = resized.dimensions();
    assert!(resized_width <= 2048);
    assert!(resized_height <= 768);
    assert!(resized_width < original_width);
    assert!(resized_height < original_height);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn view_image_tool_does_not_force_original_resolution_with_capability_feature_only()
-> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex()
        .with_model("gpt-5.3-codex")
        .with_config(|config| {
            config
                .features
                .enable(Feature::ImageDetailOriginal)
                .expect("test config should allow feature update");
        });
    let test = builder.build_remote_aware(&server).await?;
    let TestCodex {
        codex,
        config,
        session_configured,
        ..
    } = &test;

    let rel_path = "assets/original-example-capability-only.png";
    let original_width = 2304;
    let original_height = 864;
    write_workspace_png(
        &test,
        rel_path,
        original_width,
        original_height,
        [0u8, 80, 255, 255],
    )
    .await?;

    let call_id = "view-image-capability-only";
    let arguments = serde_json::json!({ "path": rel_path }).to_string();

    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_function_call(call_id, "view_image", &arguments),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once(&server, first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);
    let mock = responses::mount_sse_once(&server, second_response).await;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "please add the screenshot".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: config.cwd.clone(),
            approval_policy: AskForApproval::Never,
            approvals_reviewer: None,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            service_tier: None,
            summary: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    wait_for_event_with_timeout(
        codex,
        |event| matches!(event, EventMsg::TurnComplete(_)),
        Duration::from_secs(10),
    )
    .await;

    let req = mock.single_request();
    let function_output = req.function_call_output(call_id);
    let output_items = function_output
        .get("output")
        .and_then(Value::as_array)
        .expect("function_call_output should be a content item array");
    assert_eq!(output_items.len(), 1);
    assert_eq!(output_items[0].get("detail"), None);
    let image_url = output_items[0]
        .get("image_url")
        .and_then(Value::as_str)
        .expect("image_url present");

    let (_, encoded) = image_url
        .split_once(',')
        .expect("image url contains data prefix");
    let decoded = BASE64_STANDARD
        .decode(encoded)
        .expect("image data decodes from base64 for request");
    let resized = load_from_memory(&decoded).expect("load resized image");
    let (resized_width, resized_height) = resized.dimensions();
    assert!(resized_width <= 2048);
    assert!(resized_height <= 768);
    assert!(resized_width < original_width);
    assert!(resized_height < original_height);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_emit_image_attaches_local_image() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::JsRepl)
            .expect("test config should allow feature update");
    });
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;

    let call_id = "js-repl-view-image";
    let js_input = r#"
const fs = await import("node:fs/promises");
const path = await import("node:path");
const imagePath = path.join(codex.tmpDir, "js-repl-view-image.png");
const png = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
  "base64"
);
await fs.writeFile(imagePath, png);
const out = await codex.tool("view_image", { path: imagePath });
await codex.emitImage(out);
"#;

    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_custom_tool_call(call_id, "js_repl", js_input),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once(&server, first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);
    let mock = responses::mount_sse_once(&server, second_response).await;

    let session_model = session_configured.model.clone();
    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "use js_repl to write an image and attach it".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            approvals_reviewer: None,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    let mut tool_event = None;
    wait_for_event_with_timeout(
        &codex,
        |event| match event {
            EventMsg::ViewImageToolCall(_) => {
                tool_event = Some(event.clone());
                false
            }
            EventMsg::TurnComplete(_) => true,
            _ => false,
        },
        Duration::from_secs(10),
    )
    .await;
    let tool_event = match tool_event {
        Some(EventMsg::ViewImageToolCall(event)) => event,
        other => panic!("expected ViewImageToolCall event, got {other:?}"),
    };
    assert!(
        tool_event.path.ends_with("js-repl-view-image.png"),
        "unexpected image path: {}",
        tool_event.path.display()
    );

    let req = mock.single_request();
    let body = req.body_json();
    assert_eq!(
        image_messages(&body).len(),
        0,
        "js_repl view_image should not inject a pending input image message"
    );

    let custom_output = req.custom_tool_call_output(call_id);
    let output_items = custom_output
        .get("output")
        .and_then(Value::as_array)
        .expect("custom_tool_call_output should be a content item array");
    let image_url = output_items
        .iter()
        .find_map(|item| {
            (item.get("type").and_then(Value::as_str) == Some("input_image"))
                .then(|| item.get("image_url").and_then(Value::as_str))
                .flatten()
        })
        .expect("image_url present in js_repl custom tool output");
    assert!(
        image_url.starts_with("data:image/png;base64,"),
        "expected png data URL, got {image_url}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_view_image_requires_explicit_emit() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    #[allow(clippy::expect_used)]
    let mut builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::JsRepl)
            .expect("test config should allow feature update");
    });
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;

    let call_id = "js-repl-view-image-no-emit";
    let js_input = r#"
const fs = await import("node:fs/promises");
const path = await import("node:path");
const imagePath = path.join(codex.tmpDir, "js-repl-view-image-no-emit.png");
const png = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
  "base64"
);
await fs.writeFile(imagePath, png);
const out = await codex.tool("view_image", { path: imagePath });
console.log(out.type);
"#;

    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_custom_tool_call(call_id, "js_repl", js_input),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once(&server, first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);
    let mock = responses::mount_sse_once(&server, second_response).await;

    let session_model = session_configured.model.clone();
    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "use js_repl to write an image but do not emit it".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            approvals_reviewer: None,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            service_tier: None,
            summary: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    let mut tool_event = None;
    wait_for_event_with_timeout(
        &codex,
        |event| match event {
            EventMsg::ViewImageToolCall(_) => {
                tool_event = Some(event.clone());
                false
            }
            EventMsg::TurnComplete(_) => true,
            _ => false,
        },
        Duration::from_secs(10),
    )
    .await;
    let tool_event = match tool_event {
        Some(EventMsg::ViewImageToolCall(event)) => event,
        other => panic!("expected ViewImageToolCall event, got {other:?}"),
    };
    assert!(
        tool_event.path.ends_with("js-repl-view-image-no-emit.png"),
        "unexpected image path: {}",
        tool_event.path.display()
    );

    let req = mock.single_request();
    let custom_output = req.custom_tool_call_output(call_id);
    let output_items = custom_output.get("output").and_then(Value::as_array);
    assert!(
        output_items.is_none_or(|items| items
            .iter()
            .all(|item| item.get("type").and_then(Value::as_str) != Some("input_image"))),
        "nested view_image should not auto-populate js_repl output"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn view_image_tool_errors_when_path_is_directory() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex();
    let test = builder.build_remote_aware(&server).await?;
    let TestCodex {
        codex,
        config,
        session_configured,
        ..
    } = &test;

    let rel_path = "assets";
    let abs_path = create_workspace_directory(&test, rel_path).await?;

    let call_id = "view-image-directory";
    let arguments = serde_json::json!({ "path": rel_path }).to_string();

    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_function_call(call_id, "view_image", &arguments),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once(&server, first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);
    let mock = responses::mount_sse_once(&server, second_response).await;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "please attach the folder".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: config.cwd.clone(),
            approval_policy: AskForApproval::Never,
            approvals_reviewer: None,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    wait_for_event(codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    let req = mock.single_request();
    let body_with_tool_output = req.body_json();
    let output_text = req
        .function_call_output_content_and_success(call_id)
        .and_then(|(content, _)| content)
        .expect("output text present");
    let expected_message = format!("image path `{}` is not a file", abs_path.display());
    assert_eq!(output_text, expected_message);

    assert!(
        find_image_message(&body_with_tool_output).is_none(),
        "directory path should not produce an input_image message"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn view_image_tool_errors_for_non_image_files() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex();
    let test = builder.build_remote_aware(&server).await?;
    let TestCodex {
        codex,
        config,
        session_configured,
        ..
    } = &test;

    let rel_path = "assets/example.json";
    let abs_path =
        write_workspace_file(&test, rel_path, br#"{ "message": "hello" }"#.to_vec()).await?;

    let call_id = "view-image-non-image";
    let arguments = serde_json::json!({ "path": rel_path }).to_string();

    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_function_call(call_id, "view_image", &arguments),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once(&server, first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);
    let mock = responses::mount_sse_once(&server, second_response).await;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "please use the view_image tool to read the json file".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: config.cwd.clone(),
            approval_policy: AskForApproval::Never,
            approvals_reviewer: None,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    wait_for_event(codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    let request = mock.single_request();
    assert!(
        request.inputs_of_type("input_image").is_empty(),
        "non-image file should not produce an input_image message"
    );
    let (error_text, success) = request
        .function_call_output_content_and_success(call_id)
        .expect("function_call_output should be present");
    assert_eq!(success, None);
    let error_text = error_text.expect("error text present");

    let expected_error = format!(
        "unable to process image at `{}`: unsupported image `application/json`",
        abs_path.display()
    );
    assert!(
        error_text.contains(&expected_error),
        "error should describe unsupported file type: {error_text}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn view_image_tool_errors_when_file_missing() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex();
    let test = builder.build_remote_aware(&server).await?;
    let TestCodex {
        codex,
        config,
        session_configured,
        ..
    } = &test;

    let rel_path = "missing/example.png";
    let abs_path = config.cwd.join(rel_path);

    let call_id = "view-image-missing";
    let arguments = serde_json::json!({ "path": rel_path }).to_string();

    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_function_call(call_id, "view_image", &arguments),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once(&server, first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);
    let mock = responses::mount_sse_once(&server, second_response).await;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "please attach the missing image".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: config.cwd.clone(),
            approval_policy: AskForApproval::Never,
            approvals_reviewer: None,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    wait_for_event(codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    let req = mock.single_request();
    let body_with_tool_output = req.body_json();
    let output_text = req
        .function_call_output_content_and_success(call_id)
        .and_then(|(content, _)| content)
        .expect("output text present");
    let expected_prefix = format!("unable to locate image at `{}`:", abs_path.display());
    assert!(
        output_text.starts_with(&expected_prefix),
        "expected error to start with `{expected_prefix}` but got `{output_text}`"
    );

    assert!(
        find_image_message(&body_with_tool_output).is_none(),
        "missing file should not produce an input_image message"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn view_image_tool_returns_unsupported_message_for_text_only_model() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    // Use MockServer directly (not start_mock_server) so the first /models request returns our
    // text-only model. start_mock_server mounts empty models first, causing get_model_info to
    // fall back to model_info_from_slug with default_input_modalities (Text+Image), which would
    // incorrectly allow view_image.
    let server = MockServer::builder()
        .body_print_limit(BodyPrintLimit::Limited(80_000))
        .start()
        .await;
    let model_slug = "text-only-view-image-test-model";
    let text_only_model = ModelInfo {
        slug: model_slug.to_string(),
        display_name: "Text-only view_image test model".to_string(),
        description: Some("Remote model for view_image unsupported-path coverage".to_string()),
        default_reasoning_level: Some(ReasoningEffort::Medium),
        supported_reasoning_levels: vec![ReasoningEffortPreset {
            effort: ReasoningEffort::Medium,
            description: ReasoningEffort::Medium.to_string(),
        }],
        shell_type: ConfigShellToolType::ShellCommand,
        visibility: ModelVisibility::List,
        supported_in_api: true,
        input_modalities: vec![InputModality::Text],
        used_fallback_model_metadata: false,
        supports_search_tool: false,
        priority: 1,
        upgrade: None,
        base_instructions: "base instructions".to_string(),
        model_messages: None,
        supports_reasoning_summaries: false,
        default_reasoning_summary: ReasoningSummary::Auto,
        support_verbosity: false,
        default_verbosity: None,
        availability_nux: None,
        apply_patch_tool_type: None,
        web_search_tool_type: Default::default(),
        truncation_policy: TruncationPolicyConfig::bytes(10_000),
        supports_parallel_tool_calls: false,
        supports_image_detail_original: false,
        context_window: Some(272_000),
        auto_compact_token_limit: None,
        effective_context_window_percent: 95,
        experimental_supported_tools: Vec::new(),
    };
    mount_models_once(
        &server,
        ModelsResponse {
            models: vec![text_only_model],
        },
    )
    .await;

    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(|config| {
            config.model = Some(model_slug.to_string());
        });
    let test = builder.build_remote_aware(&server).await?;
    let TestCodex { codex, config, .. } = &test;

    let rel_path = "assets/example.png";
    write_workspace_png(&test, rel_path, 20, 20, [255u8, 0, 0, 255]).await?;

    let call_id = "view-image-unsupported-model";
    let arguments = serde_json::json!({ "path": rel_path }).to_string();
    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_function_call(call_id, "view_image", &arguments),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once(&server, first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);
    let mock = responses::mount_sse_once(&server, second_response).await;

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "please attach the image".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: config.cwd.clone(),
            approval_policy: AskForApproval::Never,
            approvals_reviewer: None,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: model_slug.to_string(),
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    wait_for_event(codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    let output_text = mock
        .single_request()
        .function_call_output_content_and_success(call_id)
        .and_then(|(content, _)| content)
        .expect("output text present");
    assert_eq!(
        output_text,
        "view_image is not allowed because you do not support image inputs"
    );

    Ok(())
}

#[cfg(not(debug_assertions))]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn replaces_invalid_local_image_after_bad_request() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    const INVALID_IMAGE_ERROR: &str =
        "The image data you provided does not represent a valid image";

    let invalid_image_mock = responses::mount_response_once_match(
        &server,
        body_string_contains("\"input_image\""),
        ResponseTemplate::new(400)
            .insert_header("content-type", "text/plain")
            .set_body_string(INVALID_IMAGE_ERROR),
    )
    .await;

    let success_response = sse(vec![
        ev_response_created("resp-2"),
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);

    let completion_mock = responses::mount_sse_once(&server, success_response).await;

    let mut builder = test_codex();
    let test = builder.build_remote_aware(&server).await?;
    let TestCodex {
        codex,
        config,
        session_configured,
        ..
    } = &test;

    let rel_path = "assets/poisoned.png";
    let abs_path = write_workspace_png(&test, rel_path, 1024, 512, [10u8, 20, 30, 255]).await?;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::LocalImage {
                path: abs_path.clone(),
            }],
            final_output_json_schema: None,
            cwd: config.cwd.clone(),
            approval_policy: AskForApproval::Never,
            approvals_reviewer: None,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    let first_body = invalid_image_mock.single_request().body_json();
    assert!(
        find_image_message(&first_body).is_some(),
        "initial request should include the uploaded image"
    );

    let second_request = completion_mock.single_request();
    let second_body = second_request.body_json();
    assert!(
        find_image_message(&second_body).is_none(),
        "second request should replace the invalid image"
    );
    let user_texts = second_request.message_input_texts("user");
    assert!(user_texts.iter().any(|text| text == "Invalid image"));

    Ok(())
}
