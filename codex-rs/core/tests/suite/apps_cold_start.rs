use std::time::Duration;

use anyhow::Result;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::SEARCH_CALENDAR_CREATE_TOOL;
use core_test_support::apps_test_server::SEARCH_CALENDAR_NAMESPACE;
use core_test_support::apps_test_server::recorded_apps_tool_call_by_call_id;
use core_test_support::apps_test_server::search_capable_apps_builder;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::ev_tool_search_call;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::namespace_child_tool;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::wait_for_event_with_timeout;
use serde_json::Value;
use serde_json::json;
use wiremock::MockServer;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cold_apps_inventory_eventually_searches_and_calls_tool() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let responses_server = MockServer::start().await;
    let apps_host = MockServer::start().await;
    let (apps_server, inventory_gate) =
        AppsTestServer::mount_with_tools_list_gate(&apps_host).await?;
    let search_call_id = "cold-apps-search";
    let tool_call_id = "cold-apps-tool";
    let response_mock = mount_sse_sequence(
        &responses_server,
        vec![
            sse(vec![
                ev_response_created("cold-resp-1"),
                ev_assistant_message("cold-message-1", "inventory pending"),
                ev_completed("cold-resp-1"),
            ]),
            sse(vec![
                ev_response_created("cold-resp-2"),
                ev_assistant_message("cold-message-2", "inventory adopted"),
                ev_completed("cold-resp-2"),
            ]),
            sse(vec![
                ev_response_created("cold-resp-3"),
                ev_tool_search_call(
                    search_call_id,
                    &json!({
                        "query": "create calendar event",
                        "limit": 1,
                    }),
                ),
                ev_completed("cold-resp-3"),
            ]),
            sse(vec![
                ev_response_created("cold-resp-4"),
                ev_function_call_with_namespace(
                    tool_call_id,
                    SEARCH_CALENDAR_NAMESPACE,
                    SEARCH_CALENDAR_CREATE_TOOL,
                    &serde_json::to_string(&json!({
                        "title": "Lunch",
                        "starts_at": "2026-03-10T12:00:00Z",
                    }))?,
                ),
                ev_completed("cold-resp-4"),
            ]),
            sse(vec![
                ev_response_created("cold-resp-5"),
                ev_assistant_message("cold-message-5", "done"),
                ev_completed("cold-resp-5"),
            ]),
        ],
    )
    .await;

    let mut builder = search_capable_apps_builder(apps_server.chatgpt_base_url);
    let test =
        tokio::time::timeout(Duration::from_secs(10), builder.build(&responses_server)).await??;
    tokio::time::timeout(Duration::from_secs(10), inventory_gate.wait_until_entered()).await?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "Find and call the calendar create tool".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    wait_for_event_with_timeout(
        &test.codex,
        |event| matches!(event, EventMsg::TurnStarted(_)),
        Duration::from_secs(10),
    )
    .await;

    inventory_gate.release();
    wait_for_event_with_timeout(
        &test.codex,
        |event| matches!(event, EventMsg::TurnComplete(_)),
        Duration::from_secs(10),
    )
    .await;
    test.submit_turn("adopt the published Apps inventory")
        .await?;
    test.submit_turn("find and call the calendar create tool")
        .await?;

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 5);
    let search_output = requests[3].tool_search_output(search_call_id);
    assert!(
        namespace_child_tool(
            &search_output,
            SEARCH_CALENDAR_NAMESPACE,
            SEARCH_CALENDAR_CREATE_TOOL,
        )
        .is_some(),
        "tool_search should return the Calendar create tool"
    );
    let upstream_call = recorded_apps_tool_call_by_call_id(&apps_host, tool_call_id).await;
    assert_eq!(
        upstream_call
            .pointer("/params/name")
            .and_then(Value::as_str),
        Some("calendar_create_event")
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cold_apps_inventory_recovers_after_startup_failures_on_later_boundaries() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let responses_server = MockServer::start().await;
    let apps_host = MockServer::start().await;
    let (apps_server, startup_control) =
        AppsTestServer::mount_searchable_with_startup_control(&apps_host).await?;
    startup_control.fail_next_initialize_attempts(/*attempts*/ 2);
    let search_call_id = "recovered-apps-search";
    let tool_call_id = "recovered-apps-tool";
    let response_mock = mount_sse_sequence(
        &responses_server,
        vec![
            sse(vec![
                ev_response_created("recovery-resp-1"),
                ev_assistant_message("recovery-message-1", "recovery started"),
                ev_completed("recovery-resp-1"),
            ]),
            sse(vec![
                ev_response_created("recovery-resp-2"),
                ev_assistant_message("recovery-message-2", "recovery published"),
                ev_completed("recovery-resp-2"),
            ]),
            sse(vec![
                ev_response_created("recovery-resp-3"),
                ev_tool_search_call(
                    search_call_id,
                    &json!({
                        "query": "create calendar event",
                        "limit": 1,
                    }),
                ),
                ev_completed("recovery-resp-3"),
            ]),
            sse(vec![
                ev_response_created("recovery-resp-4"),
                ev_function_call_with_namespace(
                    tool_call_id,
                    SEARCH_CALENDAR_NAMESPACE,
                    SEARCH_CALENDAR_CREATE_TOOL,
                    &serde_json::to_string(&json!({
                        "title": "Lunch",
                        "starts_at": "2026-03-10T12:00:00Z",
                    }))?,
                ),
                ev_completed("recovery-resp-4"),
            ]),
            sse(vec![
                ev_response_created("recovery-resp-5"),
                ev_assistant_message("recovery-message-5", "done"),
                ev_completed("recovery-resp-5"),
            ]),
        ],
    )
    .await;

    let mut builder = search_capable_apps_builder(apps_server.chatgpt_base_url);
    let test =
        tokio::time::timeout(Duration::from_secs(10), builder.build(&responses_server)).await??;
    tokio::time::timeout(Duration::from_secs(10), async {
        while startup_control.initialize_attempts() < 2 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("initial Apps startup and immediate background retry should fail");
    assert_eq!(startup_control.initialize_attempts(), 2);
    assert_eq!(startup_control.tools_list_attempts(), 0);

    tokio::time::sleep(Duration::from_millis(1_100)).await;
    test.submit_turn("continue while Apps recovers in the background")
        .await?;
    tokio::time::timeout(Duration::from_secs(10), async {
        while startup_control.initialize_attempts() < 3 || startup_control.tools_list_attempts() < 1
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("eligible Apps retry should reconnect and fetch inventory");
    assert_eq!(startup_control.initialize_attempts(), 3);
    assert_eq!(startup_control.tools_list_attempts(), 1);

    test.submit_turn("adopt the recovered Apps inventory")
        .await?;
    test.submit_turn("find and call the recovered calendar create tool")
        .await?;

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 5);
    let search_output = requests[3].tool_search_output(search_call_id);
    assert!(
        namespace_child_tool(
            &search_output,
            SEARCH_CALENDAR_NAMESPACE,
            SEARCH_CALENDAR_CREATE_TOOL,
        )
        .is_some(),
        "tool_search should return the recovered Calendar create tool"
    );
    let upstream_call = recorded_apps_tool_call_by_call_id(&apps_host, tool_call_id).await;
    assert_eq!(
        upstream_call
            .pointer("/params/name")
            .and_then(Value::as_str),
        Some("calendar_create_event")
    );
    Ok(())
}
