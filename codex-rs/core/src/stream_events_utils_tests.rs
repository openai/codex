use super::completed_item_defers_mailbox_delivery_to_next_turn;
use super::handle_non_tool_response_item;
use super::image_generation_artifact_path;
use super::last_assistant_message_from_item;
use super::response_item_may_include_external_context;
use super::save_image_generation_result;
use crate::session::tests::make_session_and_context;
use codex_protocol::error::CodexErr;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::LocalShellAction;
use codex_protocol::models::LocalShellExecAction;
use codex_protocol::models::LocalShellStatus;
use codex_protocol::models::MessagePhase;
use codex_protocol::models::ResponseItem;
use codex_utils_absolute_path::test_support::PathExt;
use pretty_assertions::assert_eq;

fn assistant_output_text(text: &str) -> ResponseItem {
    assistant_output_text_with_phase(text, /*phase*/ None)
}

fn assistant_output_text_with_phase(text: &str, phase: Option<MessagePhase>) -> ResponseItem {
    ResponseItem::Message {
        id: Some("msg-1".to_string()),
        role: "assistant".to_string(),
        content: vec![ContentItem::OutputText {
            text: text.to_string(),
        }],
        end_turn: Some(true),
        phase,
    }
}

#[test]
fn external_context_pollution_items_include_web_search_and_tool_search() {
    let polluting_items = [
        ResponseItem::WebSearchCall {
            id: None,
            status: Some("completed".to_string()),
            action: None,
        },
        ResponseItem::ToolSearchCall {
            id: None,
            call_id: Some("search-1".to_string()),
            status: None,
            execution: "client".to_string(),
            arguments: serde_json::json!({"query": "calendar"}),
        },
        ResponseItem::ToolSearchOutput {
            call_id: Some("search-1".to_string()),
            status: "completed".to_string(),
            execution: "client".to_string(),
            tools: Vec::new(),
        },
    ];

    assert!(
        polluting_items
            .iter()
            .all(response_item_may_include_external_context)
    );
}

#[test]
fn external_context_pollution_items_exclude_local_tool_calls() {
    let non_polluting_items = [
        ResponseItem::LocalShellCall {
            id: None,
            call_id: Some("shell-1".to_string()),
            status: LocalShellStatus::Completed,
            action: LocalShellAction::Exec(LocalShellExecAction {
                command: vec!["cat".to_string(), "README.md".to_string()],
                timeout_ms: None,
                working_directory: None,
                env: None,
                user: None,
            }),
        },
        ResponseItem::FunctionCall {
            id: None,
            name: "shell".to_string(),
            namespace: None,
            arguments: "{}".to_string(),
            call_id: "call-1".to_string(),
        },
        ResponseItem::FunctionCallOutput {
            call_id: "call-1".to_string(),
            output: FunctionCallOutputPayload::from_text("ok".to_string()),
        },
        ResponseItem::CustomToolCall {
            id: None,
            status: None,
            call_id: "custom-1".to_string(),
            name: "apply_patch".to_string(),
            input: "*** Begin Patch\n*** End Patch\n".to_string(),
        },
        ResponseItem::CustomToolCallOutput {
            call_id: "custom-1".to_string(),
            name: Some("apply_patch".to_string()),
            output: FunctionCallOutputPayload::from_text("ok".to_string()),
        },
        assistant_output_text("plain assistant text"),
    ];

    assert!(
        !non_polluting_items
            .iter()
            .any(response_item_may_include_external_context)
    );
}

#[tokio::test]
async fn handle_non_tool_response_item_strips_citations_from_assistant_message() {
    let (session, turn_context) = make_session_and_context().await;
    let item = assistant_output_text(
        "hello<oai-mem-citation><citation_entries>\nMEMORY.md:1-2|note=[x]\n</citation_entries>\n<rollout_ids>\n019cc2ea-1dff-7902-8d40-c8f6e5d83cc4\n</rollout_ids></oai-mem-citation> world",
    );

    let turn_item =
        handle_non_tool_response_item(&session, &turn_context, &item, /*plan_mode*/ false)
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
    let memory_citation = agent_message
        .memory_citation
        .expect("memory citation should be parsed");
    assert_eq!(memory_citation.entries.len(), 1);
    assert_eq!(memory_citation.entries[0].path, "MEMORY.md");
    assert_eq!(
        memory_citation.rollout_ids,
        vec!["019cc2ea-1dff-7902-8d40-c8f6e5d83cc4".to_string()]
    );
}

#[test]
fn last_assistant_message_from_item_strips_citations_and_plan_blocks() {
    let item = assistant_output_text(
        "before<oai-mem-citation>doc1</oai-mem-citation>\n<proposed_plan>\n- x\n</proposed_plan>\nafter",
    );

    let message = last_assistant_message_from_item(&item, /*plan_mode*/ true)
        .expect("assistant text should remain after stripping");

    assert_eq!(message, "before\nafter");
}

#[test]
fn last_assistant_message_from_item_returns_none_for_citation_only_message() {
    let item = assistant_output_text("<oai-mem-citation>doc1</oai-mem-citation>");

    assert_eq!(
        last_assistant_message_from_item(&item, /*plan_mode*/ false),
        None
    );
}

#[test]
fn last_assistant_message_from_item_returns_none_for_plan_only_hidden_message() {
    let item = assistant_output_text("<proposed_plan>\n- x\n</proposed_plan>");

    assert_eq!(
        last_assistant_message_from_item(&item, /*plan_mode*/ true),
        None
    );
}

#[test]
fn completed_item_defers_mailbox_delivery_for_unknown_phase_messages() {
    let item = assistant_output_text("final answer");

    assert!(completed_item_defers_mailbox_delivery_to_next_turn(
        &item, /*plan_mode*/ false,
    ));
}

#[test]
fn completed_item_keeps_mailbox_delivery_open_for_commentary_messages() {
    let item = assistant_output_text_with_phase("still working", Some(MessagePhase::Commentary));

    assert!(!completed_item_defers_mailbox_delivery_to_next_turn(
        &item, /*plan_mode*/ false,
    ));
}

#[test]
fn completed_item_defers_mailbox_delivery_for_image_generation_calls() {
    let item = ResponseItem::ImageGenerationCall {
        id: "ig-1".to_string(),
        status: "completed".to_string(),
        revised_prompt: None,
        result: "Zm9v".to_string(),
    };

    assert!(completed_item_defers_mailbox_delivery_to_next_turn(
        &item, /*plan_mode*/ false,
    ));
}

#[tokio::test]
async fn save_image_generation_result_saves_base64_to_png_in_codex_home() {
    let codex_home = tempfile::tempdir().expect("create codex home");
    let codex_home = codex_home.path().abs();
    let expected_path = image_generation_artifact_path(&codex_home, "session-1", "ig_save_base64");
    let _ = std::fs::remove_file(&expected_path);

    let saved_path =
        save_image_generation_result(&codex_home, "session-1", "ig_save_base64", "Zm9v")
            .await
            .expect("image should be saved");

    assert_eq!(saved_path, expected_path);
    assert_eq!(std::fs::read(&saved_path).expect("saved file"), b"foo");
    let _ = std::fs::remove_file(&saved_path);
}

#[tokio::test]
async fn save_image_generation_result_rejects_data_url_payload() {
    let result = "data:image/jpeg;base64,Zm9v";
    let codex_home = tempfile::tempdir().expect("create codex home");
    let codex_home = codex_home.path().abs();

    let err = save_image_generation_result(&codex_home, "session-1", "ig_456", result)
        .await
        .expect_err("data url payload should error");
    assert!(matches!(err, CodexErr::InvalidRequest(_)));
}

#[tokio::test]
async fn save_image_generation_result_overwrites_existing_file() {
    let codex_home = tempfile::tempdir().expect("create codex home");
    let codex_home = codex_home.path().abs();
    let existing_path = image_generation_artifact_path(&codex_home, "session-1", "ig_overwrite");
    std::fs::create_dir_all(
        existing_path
            .parent()
            .expect("generated image path should have a parent"),
    )
    .expect("create image output dir");
    std::fs::write(&existing_path, b"existing").expect("seed existing image");

    let saved_path = save_image_generation_result(&codex_home, "session-1", "ig_overwrite", "Zm9v")
        .await
        .expect("image should be saved");

    assert_eq!(saved_path, existing_path);
    assert_eq!(std::fs::read(&saved_path).expect("saved file"), b"foo");
    let _ = std::fs::remove_file(&saved_path);
}

#[tokio::test]
async fn save_image_generation_result_sanitizes_call_id_for_codex_home_output_path() {
    let codex_home = tempfile::tempdir().expect("create codex home");
    let codex_home = codex_home.path().abs();
    let expected_path = image_generation_artifact_path(&codex_home, "session-1", "../ig/..");
    let _ = std::fs::remove_file(&expected_path);

    let saved_path = save_image_generation_result(&codex_home, "session-1", "../ig/..", "Zm9v")
        .await
        .expect("image should be saved");

    assert_eq!(saved_path, expected_path);
    assert_eq!(std::fs::read(&saved_path).expect("saved file"), b"foo");
    let _ = std::fs::remove_file(&saved_path);
}

#[tokio::test]
async fn save_image_generation_result_rejects_non_standard_base64() {
    let codex_home = tempfile::tempdir().expect("create codex home");
    let codex_home = codex_home.path().abs();
    let err = save_image_generation_result(&codex_home, "session-1", "ig_urlsafe", "_-8")
        .await
        .expect_err("non-standard base64 should error");
    assert!(matches!(err, CodexErr::InvalidRequest(_)));
}

#[tokio::test]
async fn save_image_generation_result_rejects_non_base64_data_urls() {
    let codex_home = tempfile::tempdir().expect("create codex home");
    let codex_home = codex_home.path().abs();
    let err = save_image_generation_result(
        &codex_home,
        "session-1",
        "ig_svg",
        "data:image/svg+xml,<svg/>",
    )
    .await
    .expect_err("non-base64 data url should error");
    assert!(matches!(err, CodexErr::InvalidRequest(_)));
}
