use anyhow::Context;
use codex_core::features::Feature;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::JsReplToolCallPayloadKind;
use codex_protocol::protocol::JsReplToolCallResponseEvent;
use codex_protocol::protocol::JsReplToolCallResponseSummary;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_custom_tool_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use image::ImageBuffer;
use image::Rgba;
use pretty_assertions::assert_eq;
use std::path::Path;
use std::time::Duration;

fn find_user_message_with_image(text: &str) -> Option<ResponseItem> {
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let rollout: RolloutLine = match serde_json::from_str(trimmed) {
            Ok(rollout) => rollout,
            Err(_) => continue,
        };
        if let RolloutItem::ResponseItem(ResponseItem::Message { role, content, .. }) =
            &rollout.item
            && role == "user"
            && content
                .iter()
                .any(|span| matches!(span, ContentItem::InputImage { .. }))
            && let RolloutItem::ResponseItem(item) = rollout.item.clone()
        {
            return Some(item);
        }
    }
    None
}

fn extract_image_url(item: &ResponseItem) -> Option<String> {
    match item {
        ResponseItem::Message { content, .. } => content.iter().find_map(|span| match span {
            ContentItem::InputImage { image_url } => Some(image_url.clone()),
            _ => None,
        }),
        _ => None,
    }
}

async fn read_rollout_text(path: &Path) -> anyhow::Result<String> {
    for _ in 0..50 {
        if path.exists()
            && let Ok(text) = std::fs::read_to_string(path)
            && !text.trim().is_empty()
        {
            return Ok(text);
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    std::fs::read_to_string(path)
        .with_context(|| format!("read rollout file at {}", path.display()))
}

fn find_js_repl_tool_response_event(text: &str) -> Option<JsReplToolCallResponseEvent> {
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let rollout: RolloutLine = match serde_json::from_str(trimmed) {
            Ok(rollout) => rollout,
            Err(_) => continue,
        };
        if let RolloutItem::EventMsg(EventMsg::JsReplToolCallResponse(event)) = rollout.item
            && event.tool_name == "view_image"
        {
            return Some(event);
        }
    }
    None
}

fn expected_js_repl_view_image_summary() -> JsReplToolCallResponseSummary {
    JsReplToolCallResponseSummary {
        response_type: Some("function_call_output".to_string()),
        payload_kind: Some(JsReplToolCallPayloadKind::FunctionContentItems),
        payload_text_preview: None,
        payload_text_length: None,
        payload_item_count: Some(1),
        text_item_count: Some(0),
        image_item_count: Some(1),
        structured_content_present: None,
        result_is_error: None,
    }
}

async fn run_js_repl_nested_tool_response_rollout(
    persist_extended_history: bool,
) -> anyhow::Result<JsReplToolCallResponseEvent> {
    let server = start_mock_server().await;

    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::JsRepl);
    });
    let initial = builder.build(&server).await?;
    initial.codex.submit(Op::Shutdown).await?;
    wait_for_event(&initial.codex, |event| {
        matches!(event, EventMsg::ShutdownComplete)
    })
    .await;

    let new_thread = initial
        .thread_manager
        .start_thread_with_tools(initial.config.clone(), Vec::new(), persist_extended_history)
        .await?;
    let codex = new_thread.thread;
    let session_model = new_thread.session_configured.model.clone();
    let cwd = initial.cwd;

    let abs_path = cwd.path().join("images/js-repl-rollout.png");
    write_test_png(&abs_path, [90, 45, 200, 255])?;
    let image_path_json = serde_json::to_string(&abs_path.display().to_string())?;

    let call_id = "js-repl-rollout";
    let js_input = format!(
        r#"
const out = await codex.tool("view_image", {{ path: {image_path_json} }});
console.log(out.type);
"#
    );

    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_custom_tool_call(call_id, "js_repl", &js_input),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once(&server, first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);
    responses::mount_sse_once(&server, second_response).await;

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "record the nested tool response".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    codex.submit(Op::Shutdown).await?;
    wait_for_event(&codex, |event| matches!(event, EventMsg::ShutdownComplete)).await;

    let rollout_path = codex.rollout_path().context("rollout path missing")?;
    let rollout_text = read_rollout_text(&rollout_path).await?;
    find_js_repl_tool_response_event(&rollout_text)
        .context("expected js_repl nested tool response in rollout")
}

fn write_test_png(path: &Path, color: [u8; 4]) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let image = ImageBuffer::from_pixel(2, 2, Rgba(color));
    image.save(path)?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn copy_paste_local_image_persists_rollout_request_shape() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let TestCodex {
        codex,
        cwd,
        session_configured,
        home: _home,
        ..
    } = test_codex().build(&server).await?;

    let rel_path = "images/paste.png";
    let abs_path = cwd.path().join(rel_path);
    write_test_png(&abs_path, [12, 34, 56, 255])?;

    let response = sse(vec![
        ev_response_created("resp-1"),
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once(&server, response).await;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![
                UserInput::LocalImage {
                    path: abs_path.clone(),
                },
                UserInput::Text {
                    text: "pasted image".to_string(),
                    text_elements: Vec::new(),
                },
            ],
            final_output_json_schema: None,
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    codex.submit(Op::Shutdown).await?;
    wait_for_event(&codex, |event| matches!(event, EventMsg::ShutdownComplete)).await;

    let rollout_path = codex.rollout_path().expect("rollout path");
    let rollout_text = read_rollout_text(&rollout_path).await?;
    let actual = find_user_message_with_image(&rollout_text)
        .expect("expected user message with input image in rollout");

    let image_url = extract_image_url(&actual).expect("expected image url in rollout");
    let expected = ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![
            ContentItem::InputText {
                text: codex_protocol::models::local_image_open_tag_text(1),
            },
            ContentItem::InputImage { image_url },
            ContentItem::InputText {
                text: codex_protocol::models::image_close_tag_text(),
            },
            ContentItem::InputText {
                text: "pasted image".to_string(),
            },
        ],
        end_turn: None,
        phase: None,
    };

    assert_eq!(actual, expected);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn drag_drop_image_persists_rollout_request_shape() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let TestCodex {
        codex,
        cwd,
        session_configured,
        home: _home,
        ..
    } = test_codex().build(&server).await?;

    let image_url = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR4nGNgYAAAAAMAASsJTYQAAAAASUVORK5CYII=".to_string();

    let response = sse(vec![
        ev_response_created("resp-1"),
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once(&server, response).await;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![
                UserInput::Image {
                    image_url: image_url.clone(),
                },
                UserInput::Text {
                    text: "dropped image".to_string(),
                    text_elements: Vec::new(),
                },
            ],
            final_output_json_schema: None,
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    codex.submit(Op::Shutdown).await?;
    wait_for_event(&codex, |event| matches!(event, EventMsg::ShutdownComplete)).await;

    let rollout_path = codex.rollout_path().expect("rollout path");
    let rollout_text = read_rollout_text(&rollout_path).await?;
    let actual = find_user_message_with_image(&rollout_text)
        .expect("expected user message with input image in rollout");

    let image_url = extract_image_url(&actual).expect("expected image url in rollout");
    let expected = ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![
            ContentItem::InputText {
                text: codex_protocol::models::image_open_tag_text(),
            },
            ContentItem::InputImage { image_url },
            ContentItem::InputText {
                text: codex_protocol::models::image_close_tag_text(),
            },
            ContentItem::InputText {
                text: "dropped image".to_string(),
            },
        ],
        end_turn: None,
        phase: None,
    };

    assert_eq!(actual, expected);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_nested_tool_response_summary_is_persisted_in_limited_rollout_history()
-> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let event = run_js_repl_nested_tool_response_rollout(false).await?;

    assert!(
        event.ok,
        "expected resolved js_repl tool event, got {event:?}"
    );
    assert_eq!(event.summary, expected_js_repl_view_image_summary());
    assert_eq!(event.response, None);
    assert_eq!(event.error, None);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_nested_tool_response_is_persisted_in_extended_rollout_history()
-> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let event = run_js_repl_nested_tool_response_rollout(true).await?;

    assert!(
        event.ok,
        "expected resolved js_repl tool event, got {event:?}"
    );
    assert_eq!(event.summary, expected_js_repl_view_image_summary());
    assert_eq!(event.error, None);

    let response = event
        .response
        .expect("expected nested tool response payload");
    assert_eq!(
        response.get("type").and_then(serde_json::Value::as_str),
        Some("function_call_output")
    );
    assert!(
        response
            .get("call_id")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|call_id| !call_id.is_empty()),
        "expected nested tool call_id in response: {response}"
    );

    let output = response
        .get("output")
        .and_then(serde_json::Value::as_array)
        .expect("expected function_call_output array payload");
    assert_eq!(output.len(), 1);
    assert_eq!(
        output[0].get("type").and_then(serde_json::Value::as_str),
        Some("input_image")
    );
    assert!(
        output[0]
            .get("image_url")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|image_url| image_url.starts_with("data:image/png;base64,")),
        "expected input_image payload in nested tool response: {response}"
    );

    Ok(())
}
