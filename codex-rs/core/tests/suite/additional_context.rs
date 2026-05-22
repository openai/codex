use anyhow::Result;
use codex_protocol::items::TurnItem;
use codex_protocol::protocol::AdditionalContextEntry;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ItemCompletedEvent;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::context_snapshot::ContextSnapshotRenderMode;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

fn untrusted_context(value: &str) -> AdditionalContextEntry {
    AdditionalContextEntry {
        value: value.to_string(),
        is_untrusted: true,
    }
}

fn trusted_context(value: &str) -> AdditionalContextEntry {
    AdditionalContextEntry {
        value: value.to_string(),
        is_untrusted: false,
    }
}

fn user_turn(text: &str, additional_context: BTreeMap<String, AdditionalContextEntry>) -> Op {
    Op::UserInput {
        environments: None,
        items: vec![UserInput::Text {
            text: text.to_string(),
            text_elements: Vec::new(),
        }],
        final_output_json_schema: None,
        responsesapi_client_metadata: None,
        additional_context,
        thread_settings: Default::default(),
    }
}

async fn wait_for_turn_complete(codex: &codex_core::CodexThread) {
    wait_for_event(codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;
}

fn additional_context_snapshot_options() -> ContextSnapshotOptions {
    ContextSnapshotOptions::default()
        .strip_capability_instructions()
        .render_mode(ContextSnapshotRenderMode::KindWithTextPrefix { max_chars: 160 })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn additional_context_is_model_visible_but_not_a_user_message_item() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let request = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let test = test_codex()
        .with_config(|config| config.include_environment_context = false)
        .build(&server)
        .await?;

    test.codex
        .submit(user_turn(
            "inspect the active tab",
            BTreeMap::from([
                ("browser_info".to_string(), untrusted_context("tab one")),
                ("automation_info".to_string(), trusted_context("run one")),
            ]),
        ))
        .await?;

    let user_item = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::ItemCompleted(ItemCompletedEvent {
            item: TurnItem::UserMessage(item),
            ..
        }) => Some(item.clone()),
        _ => None,
    })
    .await;
    assert_eq!(
        user_item.content,
        vec![UserInput::Text {
            text: "inspect the active tab".to_string(),
            text_elements: Vec::new(),
        }]
    );
    wait_for_turn_complete(&test.codex).await;

    let request = request.single_request();
    insta::assert_snapshot!(
        "additional_context_simple_input",
        context_snapshot::format_labeled_requests_snapshot(
            "additional context is inserted before the user turn input.",
            &[("Request", &request)],
            &additional_context_snapshot_options(),
        )
    );
    let developer_external_texts = request
        .message_input_texts("developer")
        .into_iter()
        .filter(|text| text.starts_with("<external_"))
        .collect::<Vec<_>>();
    assert_eq!(
        developer_external_texts,
        vec!["<external_automation_info>run one</external_automation_info>"]
    );
    assert_eq!(
        request.message_input_texts("user"),
        vec![
            "<external_browser_info>tab one</external_browser_info>",
            "inspect the active tab",
        ]
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn additional_context_trust_controls_message_role() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let request = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let test = test_codex()
        .with_config(|config| config.include_environment_context = false)
        .build(&server)
        .await?;

    test.codex
        .submit(user_turn(
            "inspect context",
            BTreeMap::from([
                ("browser_info".to_string(), untrusted_context("tab one")),
                ("automation_info".to_string(), trusted_context("run one")),
            ]),
        ))
        .await?;
    wait_for_turn_complete(&test.codex).await;

    let request = request.single_request();
    let developer_external_texts = request
        .message_input_texts("developer")
        .into_iter()
        .filter(|text| text.starts_with("<external_"))
        .collect::<Vec<_>>();
    assert_eq!(
        developer_external_texts,
        vec!["<external_automation_info>run one</external_automation_info>"]
    );
    assert_eq!(
        request.message_input_texts("user"),
        vec![
            "<external_browser_info>tab one</external_browser_info>",
            "inspect context",
        ]
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn additional_context_is_deduplicated_between_turns_while_retained() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let first_request = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let second_request = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
    )
    .await;
    let test = test_codex()
        .with_config(|config| config.include_environment_context = false)
        .build(&server)
        .await?;
    let additional_context =
        BTreeMap::from([("browser_info".to_string(), untrusted_context("same tab"))]);

    test.codex
        .submit(user_turn("first turn", additional_context.clone()))
        .await?;
    wait_for_turn_complete(&test.codex).await;

    test.codex
        .submit(user_turn("second turn", additional_context))
        .await?;
    wait_for_turn_complete(&test.codex).await;

    assert_eq!(
        first_request.single_request().message_input_texts("user"),
        vec![
            "<external_browser_info>same tab</external_browser_info>",
            "first turn",
        ]
    );
    assert_eq!(
        second_request.single_request().message_input_texts("user"),
        vec![
            "<external_browser_info>same tab</external_browser_info>",
            "first turn",
            "second turn",
        ]
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn additional_context_removes_one_value_while_adding_another() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let first_request = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let second_request = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
    )
    .await;
    let third_request = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-3"), ev_completed("resp-3")]),
    )
    .await;
    let test = test_codex()
        .with_config(|config| config.include_environment_context = false)
        .build(&server)
        .await?;

    test.codex
        .submit(user_turn(
            "first turn",
            BTreeMap::from([
                ("automation_info".to_string(), untrusted_context("run one")),
                ("browser_info".to_string(), untrusted_context("tab one")),
            ]),
        ))
        .await?;
    wait_for_turn_complete(&test.codex).await;

    test.codex
        .submit(user_turn(
            "second turn",
            BTreeMap::from([
                ("automation_info".to_string(), untrusted_context("run one")),
                ("terminal_info".to_string(), untrusted_context("pty one")),
            ]),
        ))
        .await?;
    wait_for_turn_complete(&test.codex).await;

    test.codex
        .submit(user_turn(
            "third turn",
            BTreeMap::from([
                ("automation_info".to_string(), untrusted_context("run one")),
                ("browser_info".to_string(), untrusted_context("tab one")),
                ("terminal_info".to_string(), untrusted_context("pty one")),
            ]),
        ))
        .await?;
    wait_for_turn_complete(&test.codex).await;

    assert_eq!(
        first_request.single_request().message_input_texts("user"),
        vec![
            "<external_automation_info>run one</external_automation_info>",
            "<external_browser_info>tab one</external_browser_info>",
            "first turn",
        ]
    );
    assert_eq!(
        second_request.single_request().message_input_texts("user"),
        vec![
            "<external_automation_info>run one</external_automation_info>",
            "<external_browser_info>tab one</external_browser_info>",
            "first turn",
            "<external_terminal_info>pty one</external_terminal_info>",
            "second turn",
        ]
    );
    assert_eq!(
        third_request.single_request().message_input_texts("user"),
        vec![
            "<external_automation_info>run one</external_automation_info>",
            "<external_browser_info>tab one</external_browser_info>",
            "first turn",
            "<external_terminal_info>pty one</external_terminal_info>",
            "second turn",
            "<external_browser_info>tab one</external_browser_info>",
            "third turn",
        ]
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn additional_context_value_is_truncated_before_model_input() -> Result<()> {
    skip_if_no_network!(Ok(()));

    const MAX_EXPECTED_EXTERNAL_CONTEXT_TEXT_BYTES: usize = 5 * 1024;

    let server = start_mock_server().await;
    let request = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let test = test_codex()
        .with_config(|config| config.include_environment_context = false)
        .build(&server)
        .await?;
    let long_value = format!("{}tail", "a".repeat(40_000));
    let untruncated_fragment =
        format!("<external_browser_info>{long_value}</external_browser_info>");

    test.codex
        .submit(user_turn(
            "summarize context",
            BTreeMap::from([("browser_info".to_string(), untrusted_context(&long_value))]),
        ))
        .await?;
    wait_for_turn_complete(&test.codex).await;

    let user_texts = request.single_request().message_input_texts("user");
    let [external_text, user_text] = user_texts.as_slice() else {
        panic!("expected external context plus user input, got {user_texts:?}");
    };
    assert_eq!(user_text, "summarize context");
    assert!(external_text.starts_with(&format!("<external_browser_info>{}", "a".repeat(1024))));
    assert!(external_text.contains("tokens truncated"));
    assert!(external_text.ends_with("tail</external_browser_info>"));
    assert!(external_text.len() < untruncated_fragment.len());
    assert!(
        external_text.len() <= MAX_EXPECTED_EXTERNAL_CONTEXT_TEXT_BYTES,
        "external context was not capped before model input: {} bytes",
        external_text.len()
    );

    Ok(())
}
