use anyhow::Context;
use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use codex_app_server_protocol::AdditionalContextEntry;
use codex_app_server_protocol::AdditionalContextKind;
use codex_app_server_protocol::ThreadInjectItemsParams;
use codex_app_server_protocol::ThreadInjectItemsResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_core::RolloutRecorder;
use codex_features::Feature;
use codex_protocol::ThreadId;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_rollout::InitialHistory;
use codex_rollout::RolloutItem;
use codex_state::StateRuntime;
use codex_utils_absolute_path::test_support::PathExt;
use core_test_support::responses;
use core_test_support::responses::strip_response_item_id;
use core_test_support::responses::strip_response_item_ids_from_json;
use serde_json::Value;
use serde_json::json;
use std::collections::HashMap;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn thread_inject_items_adds_raw_response_items_to_thread_history() -> Result<()> {
    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_assistant_message("msg-1", "Done"),
        responses::ev_completed("resp-1"),
    ]);
    let response_mock = responses::mount_sse_once(&server, body).await;

    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .enable_feature(Feature::Sqlite)
        .enable_feature(Feature::RetainClientDeveloperMessages)
        .with_extra_config("[memories]\ndisable_on_external_context = true")
        .write(codex_home.path())?;
    let state_db = StateRuntime::init(
        codex_state::SqliteConfig::new_for_testing(codex_home.path().abs()),
        "mock_provider".into(),
    )
    .await?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;

    let thread_req = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(thread_req)).await??;

    let injected_text = "Injected assistant context";
    let injected_item = ResponseItem::Message {
        id: None,
        role: "assistant".to_string(),
        content: vec![ContentItem::OutputText {
            text: injected_text.to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    };
    let developer_item = |text: &str| ResponseItem::Message {
        id: None,
        role: "developer".to_string(),
        content: vec![ContentItem::InputText {
            text: text.to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    };
    let injected_developer_item = developer_item("Injected developer context");
    let marker_shaped_developer_item =
        developer_item("<image_resize_notice>\nclient message\n</image_resize_notice>");
    let named_tool_output = json!({
        "type": "function_call_output",
        "name": "send_message_to_thread",
        "namespace": "codex_app",
        "output": "Another agent delegated this task.",
    });
    let named_tool_item: ResponseItem = serde_json::from_value(named_tool_output.clone())?;

    let inject_req = mcp
        .send_thread_inject_items_request(ThreadInjectItemsParams {
            thread_id: thread.id.clone(),
            items: vec![
                serde_json::to_value(&injected_item)?,
                serde_json::to_value(&injected_developer_item)?,
                serde_json::to_value(&marker_shaped_developer_item)?,
                named_tool_output.clone(),
            ],
        })
        .await?;
    let _response: ThreadInjectItemsResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(inject_req)).await??;
    assert_eq!(
        state_db
            .get_thread_memory_mode(ThreadId::from_string(&thread.id)?)
            .await?
            .as_deref(),
        Some("polluted")
    );

    let rollout_path = thread.path.as_ref().context("thread path missing")?;
    let history = RolloutRecorder::get_rollout_history(rollout_path).await?;
    let InitialHistory::Resumed(resumed_history) = history else {
        panic!("expected resumed rollout history");
    };
    let persisted_injected_items = resumed_history
        .history
        .iter()
        .filter_map(|item| match item {
            RolloutItem::ResponseItem(envelope) => Some((
                strip_response_item_id(responses::strip_metadata(envelope.item.clone())),
                envelope
                    .metadata
                    .as_ref()
                    .map(|metadata| metadata.client_authored),
            )),
            _ => None,
        })
        .filter(|(item, _)| {
            item == &injected_item
                || item == &injected_developer_item
                || item == &marker_shaped_developer_item
                || item == &named_tool_item
        })
        .collect::<Vec<_>>();
    assert_eq!(
        persisted_injected_items,
        vec![
            (injected_item.clone(), None),
            (injected_developer_item.clone(), Some(true)),
            (marker_shaped_developer_item.clone(), Some(true)),
            (named_tool_item, None),
        ]
    );

    let application_context_text = "Application developer context";
    let untrusted_context_text = "Untrusted client context";
    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
            input: vec![V2UserInput::Text {
                text: "Hello".to_string(),
                text_elements: Vec::new(),
            }],
            additional_context: Some(HashMap::from([
                (
                    "application_context".to_string(),
                    AdditionalContextEntry {
                        value: application_context_text.to_string(),
                        kind: AdditionalContextKind::Application,
                    },
                ),
                (
                    "untrusted_context".to_string(),
                    AdditionalContextEntry {
                        value: untrusted_context_text.to_string(),
                        kind: AdditionalContextKind::Untrusted,
                    },
                ),
            ])),
            ..Default::default()
        })
        .await?;
    let _: TurnStartResponse = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(turn_req)).await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let InitialHistory::Resumed(resumed_history) =
        RolloutRecorder::get_rollout_history(rollout_path).await?
    else {
        panic!("expected resumed rollout history");
    };
    let persisted_additional_context = resumed_history
        .history
        .iter()
        .filter_map(|item| match item {
            RolloutItem::ResponseItem(envelope) => {
                let ResponseItem::Message { role, content, .. } = &envelope.item else {
                    return None;
                };
                content.iter().find_map(|item| {
                    let ContentItem::InputText { text } = item else {
                        return None;
                    };
                    (text.contains(application_context_text)
                        || text.contains(untrusted_context_text))
                    .then(|| {
                        (
                            role.as_str(),
                            envelope
                                .metadata
                                .as_ref()
                                .map(|metadata| metadata.client_authored),
                        )
                    })
                })
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        persisted_additional_context,
        vec![("developer", Some(true)), ("user", None)]
    );

    let injected_value = serde_json::to_value(&injected_item)?;
    let model_input: Vec<Value> = response_mock
        .single_request()
        .input()
        .into_iter()
        .map(strip_response_item_ids_from_json)
        .collect();
    assert!(
        model_input
            .iter()
            .all(|item| item.get("metadata").is_none() && item.get("client_authored").is_none()),
        "private harness metadata must never enter the provider request"
    );
    assert!(
        response_item_text_position(&model_input, application_context_text).is_some(),
        "application-provided developer context should reach the model"
    );
    let environment_context_index =
        response_item_text_position(&model_input, "<environment_context>")
            .expect("environment context should be injected before the first user turn");
    let injected_index = model_input
        .iter()
        .position(|item| item == &injected_value)
        .expect("injected item should be sent in the next model request");
    let user_prompt_index = response_item_text_position(&model_input, "Hello")
        .expect("user prompt should be sent in the next model request");
    assert!(
        environment_context_index < injected_index,
        "standard initial context should be sent before injected items"
    );
    assert!(
        injected_index < user_prompt_index,
        "injected items should be sent before the user prompt"
    );
    assert!(
        model_input.contains(&named_tool_output),
        "named unpaired tool output should be sent in the next model request"
    );

    Ok(())
}

#[tokio::test]
async fn thread_inject_items_adds_raw_response_items_after_a_turn() -> Result<()> {
    let server = responses::start_mock_server().await;
    let first_body = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_assistant_message("msg-1", "First done"),
        responses::ev_completed("resp-1"),
    ]);
    let second_body = responses::sse(vec![
        responses::ev_response_created("resp-2"),
        responses::ev_assistant_message("msg-2", "Second done"),
        responses::ev_completed("resp-2"),
    ]);
    let response_mock = responses::mount_sse_sequence(&server, vec![first_body, second_body]).await;

    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;

    let thread_req = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(thread_req)).await??;

    let first_turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
            input: vec![V2UserInput::Text {
                text: "First turn".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _: TurnStartResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(first_turn_req)).await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let injected_item = ResponseItem::Message {
        id: None,
        role: "developer".to_string(),
        content: vec![ContentItem::InputText {
            text: "Injected after first turn".to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    };
    let injected_value = serde_json::to_value(&injected_item)?;

    let inject_req = mcp
        .send_thread_inject_items_request(ThreadInjectItemsParams {
            thread_id: thread.id.clone(),
            items: vec![injected_value.clone()],
        })
        .await?;
    let _response: ThreadInjectItemsResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(inject_req)).await??;

    let rollout_path = thread.path.as_ref().context("thread path missing")?;
    let InitialHistory::Resumed(resumed_history) =
        RolloutRecorder::get_rollout_history(rollout_path).await?
    else {
        panic!("expected resumed rollout history");
    };
    let persisted_developer_item = resumed_history
        .history
        .iter()
        .find_map(|item| match item {
            RolloutItem::ResponseItem(envelope)
                if strip_response_item_id(responses::strip_metadata(envelope.item.clone()))
                    == injected_item =>
            {
                Some(envelope)
            }
            _ => None,
        })
        .context("injected developer item should be persisted")?;
    assert_eq!(persisted_developer_item.metadata, None);

    let second_turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
            input: vec![V2UserInput::Text {
                text: "Second turn".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _: TurnStartResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(second_turn_req)).await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 2);
    assert!(
        !requests[0]
            .input()
            .into_iter()
            .map(strip_response_item_ids_from_json)
            .any(|item| item == injected_value),
        "injected item should not be sent before it is injected"
    );
    assert!(
        requests[1]
            .input()
            .into_iter()
            .map(strip_response_item_ids_from_json)
            .any(|item| item == injected_value),
        "injected item should be sent after being injected into existing history"
    );

    Ok(())
}

fn response_item_text_position(items: &[Value], needle: &str) -> Option<usize> {
    items.iter().position(|item| {
        item.get("content")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .any(|content| {
                content
                    .get("text")
                    .and_then(Value::as_str)
                    .is_some_and(|text| text.contains(needle))
            })
    })
}
