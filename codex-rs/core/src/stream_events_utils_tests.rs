use super::default_image_generation_output_dir;
use super::handle_non_tool_response_item;
use super::last_assistant_message_from_item;
use super::save_image_generation_result;
use crate::codex::make_session_and_context;
use crate::error::CodexErr;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use pretty_assertions::assert_eq;

fn assistant_output_text(text: &str) -> ResponseItem {
    ResponseItem::Message {
        id: Some("msg-1".to_string()),
        role: "assistant".to_string(),
        content: vec![ContentItem::OutputText {
            text: text.to_string(),
        }],
        end_turn: Some(true),
        phase: None,
    }
}

#[tokio::test]
async fn handle_non_tool_response_item_strips_citations_from_assistant_message() {
    let (session, turn_context) = make_session_and_context().await;
    let item = assistant_output_text("hello<oai-mem-citation>doc1</oai-mem-citation> world");

    let turn_item = handle_non_tool_response_item(&session, &turn_context, &item, false)
        .await
        .expect("assistant message should parse");

    let TurnItem::AgentMessage(agent_message) = turn_item else {
        panic!("expected agent message");
    };
    let text = agent_message
        .content
        .iter()
        .map(|entry| match entry {
            codex_protocol::items::AgentMessageContent::Text { text } => text.as_str(),
        })
        .collect::<String>();
    assert_eq!(text, "hello world");
}

#[test]
fn last_assistant_message_from_item_strips_citations_and_plan_blocks() {
    let item = assistant_output_text(
        "before<oai-mem-citation>doc1</oai-mem-citation>\n<proposed_plan>\n- x\n</proposed_plan>\nafter",
    );

    let message = last_assistant_message_from_item(&item, true)
        .expect("assistant text should remain after stripping");

    assert_eq!(message, "before\nafter");
}

#[test]
fn last_assistant_message_from_item_returns_none_for_citation_only_message() {
    let item = assistant_output_text("<oai-mem-citation>doc1</oai-mem-citation>");

    assert_eq!(last_assistant_message_from_item(&item, false), None);
}

#[test]
fn last_assistant_message_from_item_returns_none_for_plan_only_hidden_message() {
    let item = assistant_output_text("<proposed_plan>\n- x\n</proposed_plan>");

    assert_eq!(last_assistant_message_from_item(&item, true), None);
}

#[tokio::test]
async fn save_image_generation_result_saves_base64_to_png_in_temp_dir() {
    let expected_path = default_image_generation_output_dir().join("ig_save_base64.png");
    let _ = std::fs::remove_file(&expected_path);

    let saved_path = save_image_generation_result("ig_save_base64", "Zm9v")
        .await
        .expect("image should be saved");

    assert_eq!(saved_path, expected_path);
    assert_eq!(std::fs::read(&saved_path).expect("saved file"), b"foo");
    let _ = std::fs::remove_file(&saved_path);
}

#[tokio::test]
async fn save_image_generation_result_rejects_data_url_payload() {
    let result = "data:image/jpeg;base64,Zm9v";

    let err = save_image_generation_result("ig_456", result)
        .await
        .expect_err("data url payload should error");
    assert!(matches!(err, CodexErr::InvalidRequest(_)));
}

#[tokio::test]
async fn save_image_generation_result_overwrites_existing_file() {
    let existing_path = default_image_generation_output_dir().join("ig_overwrite.png");
    std::fs::write(&existing_path, b"existing").expect("seed existing image");

    let saved_path = save_image_generation_result("ig_overwrite", "Zm9v")
        .await
        .expect("image should be saved");

    assert_eq!(saved_path, existing_path);
    assert_eq!(std::fs::read(&saved_path).expect("saved file"), b"foo");
    let _ = std::fs::remove_file(&saved_path);
}

#[tokio::test]
async fn save_image_generation_result_sanitizes_call_id_for_temp_dir_output_path() {
    let expected_path = default_image_generation_output_dir().join("___ig___.png");
    let _ = std::fs::remove_file(&expected_path);

    let saved_path = save_image_generation_result("../ig/..", "Zm9v")
        .await
        .expect("image should be saved");

    assert_eq!(saved_path, expected_path);
    assert_eq!(std::fs::read(&saved_path).expect("saved file"), b"foo");
    let _ = std::fs::remove_file(&saved_path);
}

#[tokio::test]
async fn save_image_generation_result_rejects_non_standard_base64() {
    let err = save_image_generation_result("ig_urlsafe", "_-8")
        .await
        .expect_err("non-standard base64 should error");
    assert!(matches!(err, CodexErr::InvalidRequest(_)));
}

#[tokio::test]
async fn save_image_generation_result_rejects_non_base64_data_urls() {
    let err = save_image_generation_result("ig_svg", "data:image/svg+xml,<svg/>")
        .await
        .expect_err("non-base64 data url should error");
    assert!(matches!(err, CodexErr::InvalidRequest(_)));
}
