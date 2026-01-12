use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::RolloutItem;
use codex_core::protocol::RolloutLine;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::user_input::UserInput;
use codex_utils_cargo_bin::find_resource;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
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

fn load_expected_response_item(path: &str, image_url: &str) -> ResponseItem {
    let full_path = find_resource!(path).expect("fixture path should resolve");
    let raw = std::fs::read_to_string(full_path).expect("read fixture");
    let replaced = raw.replace("__IMAGE_URL__", image_url);
    serde_json::from_str(&replaced).expect("parse response item fixture")
}

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
        {
            if let RolloutItem::ResponseItem(item) = rollout.item.clone() {
                return Some(item);
            }
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
                },
            ],
            final_output_json_schema: None,
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await?;

    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    let rollout_text = std::fs::read_to_string(codex.rollout_path())?;
    let actual = find_user_message_with_image(&rollout_text)
        .expect("expected user message with input image in rollout");

    let image_url = extract_image_url(&actual).expect("expected image url in rollout");
    let expected = load_expected_response_item(
        "tests/fixtures/rollout_copy_paste_local_image.json",
        &image_url,
    );

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
                },
            ],
            final_output_json_schema: None,
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await?;

    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    let rollout_text = std::fs::read_to_string(codex.rollout_path())?;
    let actual = find_user_message_with_image(&rollout_text)
        .expect("expected user message with input image in rollout");

    let image_url = extract_image_url(&actual).expect("expected image url in rollout");
    let expected =
        load_expected_response_item("tests/fixtures/rollout_drag_drop_image.json", &image_url);

    assert_eq!(actual, expected);

    Ok(())
}
