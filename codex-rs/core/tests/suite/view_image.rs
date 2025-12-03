#![cfg(not(target_os = "windows"))]

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use image::GenericImageView;
use image::ImageBuffer;
use image::Rgba;
use image::load_from_memory;
use serde_json::Value;

fn find_image_message(body: &Value) -> Option<&Value> {
    body.get("input")
        .and_then(Value::as_array)
        .and_then(|items| {
            items.iter().find(|item| {
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
        })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_turn_with_local_image_attaches_image() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = test_codex().build(&server).await?;

    let rel_path = "user-turn/example.png";
    let abs_path = cwd.path().join(rel_path);
    if let Some(parent) = abs_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let image = ImageBuffer::from_pixel(4096, 1024, Rgba([20u8, 40, 60, 255]));
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
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await?;

    wait_for_event(&codex, |event| matches!(event, EventMsg::TaskComplete(_))).await;

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
    assert!(width < 4096);
    assert!(height < 1024);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn view_image_tool_attaches_local_image() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = test_codex().build(&server).await?;

    let rel_path = "assets/example.png";
    let abs_path = cwd.path().join(rel_path);
    if let Some(parent) = abs_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let image = ImageBuffer::from_pixel(4096, 1024, Rgba([255u8, 0, 0, 255]));
    image.save(&abs_path)?;

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
            }],
            final_output_json_schema: None,
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await?;

    let mut tool_event = None;
    wait_for_event(&codex, |event| match event {
        EventMsg::ViewImageToolCall(_) => {
            tool_event = Some(event.clone());
            false
        }
        EventMsg::TaskComplete(_) => true,
        _ => false,
    })
    .await;

    let tool_event = match tool_event.expect("view image tool event emitted") {
        EventMsg::ViewImageToolCall(event) => event,
        _ => unreachable!("stored event must be ViewImageToolCall"),
    };
    assert_eq!(tool_event.call_id, call_id);
    assert_eq!(tool_event.path, abs_path);

    let req = mock.single_request();
    let body = req.body_json();
    let output_text = req
        .function_call_output_content_and_success(call_id)
        .and_then(|(content, _)| content)
        .expect("output text present");
    assert_eq!(output_text, "attached local image path");

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
    let (resized_width, resized_height) = resized.dimensions();
    assert!(resized_width <= 2048);
    assert!(resized_height <= 768);
    assert!(resized_width < 4096);
    assert!(resized_height < 1024);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn view_image_tool_errors_when_path_is_directory() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = test_codex().build(&server).await?;

    let rel_path = "assets";
    let abs_path = cwd.path().join(rel_path);
    std::fs::create_dir_all(&abs_path)?;

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
            }],
            final_output_json_schema: None,
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await?;

    wait_for_event(&codex, |event| matches!(event, EventMsg::TaskComplete(_))).await;

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
async fn view_image_tool_placeholder_for_non_image_files() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = test_codex().build(&server).await?;

    let rel_path = "assets/example.json";
    let abs_path = cwd.path().join(rel_path);
    if let Some(parent) = abs_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&abs_path, br#"{ "message": "hello" }"#)?;

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
            }],
            final_output_json_schema: None,
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await?;

    wait_for_event(&codex, |event| matches!(event, EventMsg::TaskComplete(_))).await;

    let request = mock.single_request();
    assert!(
        request.inputs_of_type("input_image").is_empty(),
        "non-image file should not produce an input_image message"
    );

    let placeholder = request
        .inputs_of_type("message")
        .iter()
        .find_map(|item| {
            let content = item.get("content").and_then(Value::as_array)?;
            content.iter().find_map(|span| {
                if span.get("type").and_then(Value::as_str) == Some("input_text") {
                    let text = span.get("text").and_then(Value::as_str)?;
                    if text.contains("Codex could not read the local image at")
                        && text.contains("unsupported MIME type `application/json`")
                    {
                        return Some(text.to_string());
                    }
                }
                None
            })
        })
        .expect("placeholder text found");

    assert!(
        placeholder.contains(&abs_path.display().to_string()),
        "placeholder should mention path: {placeholder}"
    );

    let output_text = mock
        .single_request()
        .function_call_output_content_and_success(call_id)
        .and_then(|(content, _)| content)
        .expect("output text present");
    assert_eq!(output_text, "attached local image path");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn view_image_tool_errors_when_file_missing() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = test_codex().build(&server).await?;

    let rel_path = "missing/example.png";
    let abs_path = cwd.path().join(rel_path);

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
            }],
            final_output_json_schema: None,
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await?;

    wait_for_event(&codex, |event| matches!(event, EventMsg::TaskComplete(_))).await;

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

    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = test_codex().build(&server).await?;

    let rel_path = "assets/poisoned.png";
    let abs_path = cwd.path().join(rel_path);
    if let Some(parent) = abs_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let image = ImageBuffer::from_pixel(1024, 512, Rgba([10u8, 20, 30, 255]));
    image.save(&abs_path)?;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::LocalImage {
                path: abs_path.clone(),
            }],
            final_output_json_schema: None,
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await?;

    wait_for_event(&codex, |event| matches!(event, EventMsg::TaskComplete(_))).await;

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
