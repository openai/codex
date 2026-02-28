#![cfg(not(target_os = "windows"))]

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_core::CodexAuth;
use codex_core::features::Feature;
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
use image::GenericImageView;
use image::ImageBuffer;
use image::Rgba;
use image::load_from_memory;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use tokio::time::Duration;
use wiremock::BodyPrintLimit;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::Respond;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path_regex;

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
    let original_width = 2304;
    let original_height = 864;
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
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    wait_for_event_with_timeout(
        &codex,
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
    let original_width = 2304;
    let original_height = 864;
    let image = ImageBuffer::from_pixel(original_width, original_height, Rgba([255u8, 0, 0, 255]));
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
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
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
async fn js_repl_view_image_tool_attaches_local_image() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::JsRepl);
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
console.log(out.output?.body?.text ?? "");
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
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    wait_for_event_with_timeout(
        &codex,
        |event| matches!(event, EventMsg::TurnComplete(_)),
        Duration::from_secs(10),
    )
    .await;

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
async fn js_repl_view_image_tool_attaches_multiple_local_images() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::JsRepl);
    });
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;

    let call_id = "js-repl-view-image-two";
    let js_input = r#"
const fs = await import("node:fs/promises");
const path = await import("node:path");
const png = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
  "base64"
);
const imagePathA = path.join(codex.tmpDir, "js-repl-view-image-a.png");
const imagePathB = path.join(codex.tmpDir, "js-repl-view-image-b.png");
await fs.writeFile(imagePathA, png);
await fs.writeFile(imagePathB, png);
await codex.tool("view_image", { path: imagePathA });
await codex.tool("view_image", { path: imagePathB });
console.log("attached-two-images");
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
                text: "use js_repl to write two images and attach them".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: Some(ReasoningSummary::Auto),
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    wait_for_event_with_timeout(
        &codex,
        |event| matches!(event, EventMsg::TurnComplete(_)),
        Duration::from_secs(10),
    )
    .await;

    let req = mock.single_request();
    let custom_output = req.custom_tool_call_output(call_id);
    assert_ne!(
        custom_output.get("success").and_then(Value::as_bool),
        Some(false),
        "js_repl call failed unexpectedly: {custom_output}"
    );
    let output_items = custom_output
        .get("output")
        .and_then(Value::as_array)
        .expect("custom_tool_call_output should be a content item array");
    let js_repl_output = output_items
        .iter()
        .find_map(|item| {
            (item.get("type").and_then(Value::as_str) == Some("input_text"))
                .then(|| item.get("text").and_then(Value::as_str))
                .flatten()
        })
        .expect("custom tool output text present");
    assert!(
        js_repl_output.contains("attached-two-images"),
        "expected js_repl output marker, got {js_repl_output}"
    );
    let image_urls = output_items
        .iter()
        .filter_map(|item| {
            (item.get("type").and_then(Value::as_str) == Some("input_image"))
                .then(|| item.get("image_url").and_then(Value::as_str))
                .flatten()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        image_urls.len(),
        2,
        "js_repl should include one input_image content item per nested view_image call"
    );
    for image_url in image_urls {
        assert!(
            image_url.starts_with("data:image/png;base64,"),
            "expected png data URL, got {image_url}"
        );
    }

    let body = req.body_json();
    assert_eq!(
        image_messages(&body).len(),
        0,
        "js_repl should not inject pending input image messages"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_poll_view_image_tool_attaches_local_image() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    const POLL_WAIT_MS: u64 = 30_000;
    const TURN_TIMEOUT_SECS: u64 = 30;

    fn request_body_json(request: &wiremock::Request) -> Value {
        let content_encoding = request
            .headers
            .get("content-encoding")
            .and_then(|value| value.to_str().ok());
        let body = if content_encoding
            .is_some_and(|value| value.split(',').any(|entry| entry.trim() == "zstd"))
        {
            zstd::stream::decode_all(std::io::Cursor::new(&request.body))
                .expect("decode zstd request body")
        } else {
            request.body.clone()
        };
        serde_json::from_slice(&body).expect("request body json")
    }

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::JsRepl);
        config.features.enable(Feature::JsReplPolling);
    });
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;

    let submit_call_id = "js-repl-poll-submit";
    let poll_call_id = "js-repl-poll-image";
    let js_input = r#"// codex-js-repl: poll=true
const fs = await import("node:fs/promises");
const path = await import("node:path");
const imagePath = path.join(codex.tmpDir, "js-repl-poll-view-image.png");
const png = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
  "base64"
);
await fs.writeFile(imagePath, png);
await codex.tool("view_image", { path: imagePath });
"#
    .to_string();

    #[derive(Clone)]
    struct PollSequenceResponder {
        requests: Arc<Mutex<Vec<wiremock::Request>>>,
        call_count: Arc<AtomicUsize>,
        submit_call_id: &'static str,
        poll_call_id: &'static str,
        js_input: String,
    }

    impl Respond for PollSequenceResponder {
        fn respond(&self, request: &wiremock::Request) -> ResponseTemplate {
            self.requests
                .lock()
                .expect("request log lock")
                .push(request.clone());
            let request_json = request_body_json(request);
            let body = match self.call_count.fetch_add(1, Ordering::SeqCst) {
                0 => sse(vec![
                    ev_response_created("resp-1"),
                    ev_custom_tool_call(self.submit_call_id, "js_repl", &self.js_input),
                    ev_completed("resp-1"),
                ]),
                1 => {
                    let submit_output = request_json["input"]
                        .as_array()
                        .expect("request input array")
                        .iter()
                        .find(|item| {
                            item.get("type").and_then(Value::as_str)
                                == Some("custom_tool_call_output")
                                && item.get("call_id").and_then(Value::as_str)
                                    == Some(self.submit_call_id)
                        })
                        .expect("js_repl polling submit output");
                    let submit_payload = submit_output["output"]
                        .as_str()
                        .expect("js_repl polling submit output string");
                    let submit_json: Value =
                        serde_json::from_str(submit_payload).expect("submit payload json");
                    let exec_id = submit_json["exec_id"]
                        .as_str()
                        .expect("exec_id present in submit payload");
                    let poll_args = serde_json::json!({
                        "exec_id": exec_id,
                        "yield_time_ms": POLL_WAIT_MS,
                    })
                    .to_string();
                    sse(vec![
                        ev_response_created("resp-2"),
                        ev_function_call(self.poll_call_id, "js_repl_poll", &poll_args),
                        ev_completed("resp-2"),
                    ])
                }
                2 => {
                    let poll_output = request_json["input"]
                        .as_array()
                        .expect("request input array")
                        .iter()
                        .find(|item| {
                            item.get("type").and_then(Value::as_str) == Some("function_call_output")
                                && item.get("call_id").and_then(Value::as_str)
                                    == Some(self.poll_call_id)
                        })
                        .expect("js_repl_poll function_call_output present");
                    assert!(
                        poll_output.get("output").is_some(),
                        "js_repl_poll output should be present"
                    );
                    sse(vec![
                        ev_assistant_message("msg-1", "done"),
                        ev_completed("resp-3"),
                    ])
                }
                call_num => panic!("unexpected extra responses request {call_num}"),
            };

            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(body)
        }
    }

    let requests = Arc::new(Mutex::new(Vec::new()));
    Mock::given(method("POST"))
        .and(path_regex(".*/responses$"))
        .respond_with(PollSequenceResponder {
            requests: Arc::clone(&requests),
            call_count: Arc::new(AtomicUsize::new(0)),
            submit_call_id,
            poll_call_id,
            js_input,
        })
        .up_to_n_times(3)
        .expect(3)
        .mount(&server)
        .await;

    let session_model = session_configured.model.clone();
    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "use js_repl polling to write an image and attach it".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    wait_for_event_with_timeout(
        &codex,
        |event| matches!(event, EventMsg::TurnComplete(_)),
        Duration::from_secs(TURN_TIMEOUT_SECS),
    )
    .await;

    let requests = requests.lock().expect("request log lock").clone();
    assert_eq!(
        requests.len(),
        3,
        "expected submit, poll, and completion requests"
    );

    let poll_request_json = request_body_json(
        requests
            .get(2)
            .expect("final request with js_repl_poll output should be present"),
    );
    let poll_output = poll_request_json["input"]
        .as_array()
        .expect("poll request input array")
        .iter()
        .find(|item| {
            item.get("type").and_then(Value::as_str) == Some("function_call_output")
                && item.get("call_id").and_then(Value::as_str) == Some(poll_call_id)
        })
        .expect("js_repl_poll function_call_output present");
    let output_items = poll_output["output"]
        .as_array()
        .expect("js_repl_poll output should be a content item array");

    let status_item = output_items
        .first()
        .expect("first poll output item should be status text");
    assert_eq!(
        status_item.get("type").and_then(Value::as_str),
        Some("input_text")
    );
    let status_json: Value = serde_json::from_str(
        status_item["text"]
            .as_str()
            .expect("status item text should be present"),
    )
    .expect("status item should be valid json");
    assert_eq!(status_json["status"].as_str(), Some("completed"));
    assert_eq!(status_json["error"], Value::Null);
    assert_eq!(status_json["logs"], Value::Null);

    let image_items = output_items
        .iter()
        .filter_map(|item| {
            (item.get("type").and_then(Value::as_str) == Some("input_image"))
                .then(|| item.get("image_url").and_then(Value::as_str))
                .flatten()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        image_items.len(),
        1,
        "expected one image item in poll output"
    );
    assert!(
        image_items[0].starts_with("data:image/png;base64,"),
        "expected png data URL, got {}",
        image_items[0]
    );

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
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

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
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    let request = mock.single_request();
    assert!(
        request.inputs_of_type("input_image").is_empty(),
        "non-image file should not produce an input_image message"
    );
    let function_output = request.function_call_output(call_id);
    let output_items = function_output
        .get("output")
        .and_then(Value::as_array)
        .expect("function_call_output should be a content item array");
    assert_eq!(
        output_items.len(),
        1,
        "non-image placeholder should be returned as a single content item"
    );
    assert_eq!(
        output_items[0].get("type").and_then(Value::as_str),
        Some("input_text"),
        "non-image placeholder should be returned as input_text"
    );
    let placeholder = output_items[0]
        .get("text")
        .and_then(Value::as_str)
        .expect("placeholder text present");

    assert!(
        placeholder.contains("Codex could not read the local image at")
            && placeholder.contains("unsupported MIME type `application/json`"),
        "placeholder should describe the unsupported file type: {placeholder}"
    );
    assert!(
        placeholder.contains(&abs_path.display().to_string()),
        "placeholder should mention path: {placeholder}"
    );

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
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

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
        prefer_websockets: false,
        used_fallback_model_metadata: false,
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
        truncation_policy: TruncationPolicyConfig::bytes(10_000),
        supports_parallel_tool_calls: false,
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

    let TestCodex { codex, cwd, .. } = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(|config| {
            config.model = Some(model_slug.to_string());
        })
        .build(&server)
        .await?;

    let rel_path = "assets/example.png";
    let abs_path = cwd.path().join(rel_path);
    if let Some(parent) = abs_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let image = ImageBuffer::from_pixel(20, 20, Rgba([255u8, 0, 0, 255]));
    image.save(&abs_path)?;

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
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: model_slug.to_string(),
            effort: None,
            summary: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

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
            summary: None,
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
