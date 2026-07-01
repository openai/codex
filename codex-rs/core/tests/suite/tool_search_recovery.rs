use anyhow::Result;
use codex_protocol::models::InternalChatMessageMetadataPassthrough;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::Map;
use serde_json::Value;
use serde_json::json;
use std::fs;
use std::path::Path;
use std::time::Duration;
use tokio::time::timeout;

const CALL_ID: &str = "malformed-search-call";
const VALID_CALL_ID: &str = "valid-search-call";
const INVALID_TOOL_SEARCH_QUERY: &str = "[invalid tool_search arguments omitted]";

fn tool_search_item(
    id: &str,
    call_id: Option<&str>,
    execution: &str,
    arguments: Value,
) -> ResponseItem {
    ResponseItem::ToolSearchCall {
        id: Some(id.to_string()),
        call_id: call_id.map(str::to_string),
        status: Some("completed".to_string()),
        execution: execution.to_string(),
        arguments,
        internal_chat_message_metadata_passthrough: Some(InternalChatMessageMetadataPassthrough {
            turn_id: Some("source-turn".to_string()),
        }),
    }
}

fn ev_response_item_done(item: &ResponseItem) -> Value {
    json!({"type": "response.output_item.done", "item": item})
}

fn without_item_id(mut item: Value) -> Value {
    item.as_object_mut()
        .expect("response item should serialize as an object")
        .remove("id");
    item
}

fn rollout_response_items(path: &Path) -> Result<Vec<ResponseItem>> {
    Ok(fs::read_to_string(path)?
        .lines()
        .map(serde_json::from_str::<RolloutLine>)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter_map(|line| match line.item {
            RolloutItem::ResponseItem(item) => Some(item),
            _ => None,
        })
        .collect())
}

fn tool_search_call(
    request: &core_test_support::responses::ResponsesRequest,
    call_id: &str,
) -> Value {
    request
        .inputs_of_type("tool_search_call")
        .into_iter()
        .find(|item| item.get("call_id").and_then(Value::as_str) == Some(call_id))
        .expect("tool_search_call should be present")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn malformed_tool_search_is_repaired_before_persistence_and_resume() -> Result<()> {
    let oversized_key = "multilingual-generated-gibberish-世界-".repeat(24);
    let original_arguments = Value::Object(Map::from_iter([(oversized_key.clone(), json!([]))]));
    let malformed_item = tool_search_item(
        "malformed-item",
        Some(CALL_ID),
        "client",
        original_arguments,
    );
    let valid_item = tool_search_item(
        "valid-item",
        Some(VALID_CALL_ID),
        "client",
        json!({"query": "calendar", "limit": 3, "unknown": oversized_key.clone()}),
    );
    let missing_id_item = tool_search_item(
        "missing-call-id-item",
        /*call_id*/ None,
        "client",
        json!({"query": "missing"}),
    );
    let empty_id_item = tool_search_item(
        "empty-call-id-item",
        Some(""),
        "client",
        json!({"query": "empty"}),
    );
    let server_item = tool_search_item(
        "server-item",
        /*call_id*/ None,
        "server",
        json!({"serverGenerated": {"nested": true}}),
    );
    let canonical_malformed_item = tool_search_item(
        "malformed-item",
        Some(CALL_ID),
        "client",
        json!({"query": INVALID_TOOL_SEARCH_QUERY}),
    );
    let canonical_valid_item = tool_search_item(
        "valid-item",
        Some(VALID_CALL_ID),
        "client",
        json!({"query": "calendar", "limit": 3}),
    );
    let canonical_malformed_request =
        without_item_id(serde_json::to_value(&canonical_malformed_item)?);
    let canonical_valid_request = without_item_id(serde_json::to_value(&canonical_valid_item)?);
    let server_request = without_item_id(serde_json::to_value(&server_item)?);
    let original_items = vec![
        malformed_item.clone(),
        valid_item,
        missing_id_item,
        empty_id_item,
        server_item.clone(),
    ];
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut first_response = vec![ev_response_created("resp-1")];
    first_response.extend(original_items.iter().map(ev_response_item_done));
    first_response.push(ev_completed("resp-1"));
    let turn_mock = mount_sse_sequence(
        &server,
        vec![
            sse(first_response),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "recovered"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex();
    let test = builder.build_with_auto_env(&server).await?;
    let rollout_path = test
        .session_configured
        .rollout_path
        .clone()
        .expect("rollout path");
    let home = test.home.clone();

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "trigger malformed tool search".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;

    let mut raw_tool_search_items = Vec::new();
    loop {
        let event = timeout(Duration::from_secs(30), test.codex.next_event())
            .await
            .expect("timeout waiting for turn event")
            .expect("event stream ended unexpectedly");
        match event.msg {
            EventMsg::RawResponseItem(raw)
                if matches!(&raw.item, ResponseItem::ToolSearchCall { .. }) =>
            {
                raw_tool_search_items.push(raw.item);
            }
            EventMsg::TurnComplete(_) => break,
            EventMsg::Error(error) => panic!("turn failed: {}", error.message),
            _ => {}
        }
    }
    assert_eq!(raw_tool_search_items, original_items);

    let requests = turn_mock.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        tool_search_call(&requests[1], CALL_ID),
        canonical_malformed_request
    );
    assert_eq!(
        tool_search_call(&requests[1], VALID_CALL_ID),
        canonical_valid_request
    );
    let tool_search_output = requests[1].tool_search_output(CALL_ID);
    assert_eq!(
        tool_search_output.get("status").and_then(Value::as_str),
        Some("completed")
    );
    assert_eq!(
        tool_search_output.get("execution").and_then(Value::as_str),
        Some("client")
    );
    assert_eq!(tool_search_output.get("tools"), Some(&json!([])));
    assert_eq!(
        requests[1].tool_search_output(VALID_CALL_ID).get("tools"),
        Some(&json!([]))
    );
    let persisted_calls = requests[1].inputs_of_type("tool_search_call");
    assert_eq!(persisted_calls.len(), 3);
    assert!(persisted_calls.contains(&server_request));
    assert_eq!(
        persisted_calls
            .iter()
            .filter_map(|item| item.get("call_id").and_then(Value::as_str))
            .collect::<Vec<_>>(),
        vec![CALL_ID, VALID_CALL_ID]
    );
    assert!(
        requests[1]
            .inputs_of_type("function_call_output")
            .iter()
            .all(|item| item.get("call_id").and_then(Value::as_str) != Some(""))
    );

    let rollout_text = fs::read_to_string(&rollout_path)?;
    assert!(!rollout_text.contains(&oversized_key));
    let rollout_items = rollout_response_items(&rollout_path)?;
    assert!(rollout_items.contains(&canonical_malformed_item));
    assert!(rollout_items.contains(&canonical_valid_item));
    assert!(rollout_items.contains(&server_item));
    assert!(rollout_items.iter().all(|item| {
        !matches!(
            item,
            ResponseItem::ToolSearchCall {
                id: Some(id),
                ..
            } if id == "missing-call-id-item" || id == "empty-call-id-item"
        )
    }));
    assert!(rollout_items.iter().any(|item| {
        matches!(
            item,
            ResponseItem::ToolSearchOutput {
                call_id: Some(call_id),
                tools,
                ..
            } if call_id == CALL_ID && tools.is_empty()
        )
    }));

    drop(test);

    let resume_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-3"),
            ev_assistant_message("msg-2", "resumed"),
            ev_completed("resp-3"),
        ]),
    )
    .await;
    let mut resume_builder = test_codex();
    let resumed = resume_builder.resume(&server, home, rollout_path).await?;

    resumed.submit_turn("continue").await?;

    let resumed_request = resume_mock.single_request();
    assert!(
        resumed_request
            .inputs_of_type("tool_search_call")
            .contains(&server_request)
    );
    assert_eq!(
        tool_search_call(&resumed_request, CALL_ID),
        canonical_malformed_request
    );
    assert_eq!(
        tool_search_call(&resumed_request, VALID_CALL_ID),
        canonical_valid_request
    );
    assert_eq!(
        resumed_request.tool_search_output(CALL_ID).get("tools"),
        Some(&json!([]))
    );

    Ok(())
}
