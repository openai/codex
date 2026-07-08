use anyhow::Result;
use codex_features::Feature;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::SEARCH_CALENDAR_CREATE_TOOL;
use core_test_support::apps_test_server::SEARCH_CALENDAR_NAMESPACE;
use core_test_support::apps_test_server::search_capable_apps_builder;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event;
use core_test_support::wait_for_mcp_server;
use serde_json::json;
use std::time::Duration;

#[derive(Debug, Eq, PartialEq)]
enum McpCallEvent {
    Begin(String),
    End(String),
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn codex_apps_calls_run_serially_by_default() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let events = run_codex_apps_calls(/*parallel_tool_calls*/ false).await?;

    assert_calls_do_not_overlap(&events, "apps-call-1", "apps-call-2");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn codex_apps_calls_run_concurrently_with_feature() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let events = run_codex_apps_calls(/*parallel_tool_calls*/ true).await?;

    assert_calls_overlap(&events, "apps-call-1", "apps-call-2");
    Ok(())
}

async fn run_codex_apps_calls(parallel_tool_calls: bool) -> Result<Vec<McpCallEvent>> {
    let server = responses::start_mock_server().await;
    let apps_server =
        AppsTestServer::mount_with_tool_call_delay(&server, Duration::from_millis(150)).await?;
    let search_call_id = "apps-search";
    let first_call_id = "apps-call-1";
    let second_call_id = "apps-call-2";
    let first_args = json!({
        "title": "First event",
        "starts_at": "2026-07-08T10:00:00Z",
    })
    .to_string();
    let second_args = json!({
        "title": "Second event",
        "starts_at": "2026-07-08T11:00:00Z",
    })
    .to_string();
    responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_tool_search_call(
                    search_call_id,
                    &json!({"query": "create calendar event"}),
                ),
                responses::ev_completed("resp-1"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("resp-2"),
                responses::ev_function_call_with_namespace(
                    first_call_id,
                    SEARCH_CALENDAR_NAMESPACE,
                    SEARCH_CALENDAR_CREATE_TOOL,
                    &first_args,
                ),
                responses::ev_function_call_with_namespace(
                    second_call_id,
                    SEARCH_CALENDAR_NAMESPACE,
                    SEARCH_CALENDAR_CREATE_TOOL,
                    &second_args,
                ),
                responses::ev_completed("resp-2"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("resp-3"),
                responses::ev_assistant_message("msg-1", "done"),
                responses::ev_completed("resp-3"),
            ]),
        ],
    )
    .await;

    let mut builder =
        search_capable_apps_builder(apps_server.chatgpt_base_url).with_config(move |config| {
            config
                .features
                .set_enabled(Feature::CodexAppsParallelToolCalls, parallel_tool_calls)
                .expect("test config should allow feature update");
        });
    let fixture = builder.build_with_auto_env(&server).await?;
    wait_for_mcp_server(&fixture.codex, CODEX_APPS_MCP_SERVER_NAME).await?;
    submit_auto_approved_turn(&fixture).await?;

    let mut events = Vec::new();
    while events.len() < 4 {
        match wait_for_event(&fixture.codex, |event| {
            matches!(
                event,
                EventMsg::McpToolCallBegin(_) | EventMsg::McpToolCallEnd(_)
            )
        })
        .await
        {
            EventMsg::McpToolCallBegin(event) => {
                events.push(McpCallEvent::Begin(event.call_id));
            }
            EventMsg::McpToolCallEnd(event) => {
                events.push(McpCallEvent::End(event.call_id));
            }
            _ => unreachable!("event guard guarantees MCP call events"),
        }
    }
    wait_for_event(&fixture.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    Ok(events)
}

async fn submit_auto_approved_turn(fixture: &TestCodex) -> Result<()> {
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, fixture.config.cwd.as_path());
    fixture
        .codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "Create two calendar events.".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: codex_protocol::protocol::ThreadSettingsOverrides {
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                ..Default::default()
            },
        })
        .await?;
    Ok(())
}

fn assert_calls_overlap(events: &[McpCallEvent], first_call_id: &str, second_call_id: &str) {
    let first_begin = event_index(events, McpCallEvent::Begin(first_call_id.to_string()));
    let first_end = event_index(events, McpCallEvent::End(first_call_id.to_string()));
    let second_begin = event_index(events, McpCallEvent::Begin(second_call_id.to_string()));
    let second_end = event_index(events, McpCallEvent::End(second_call_id.to_string()));
    assert!(
        first_begin < second_end && second_begin < first_end,
        "Codex Apps calls should overlap; saw events: {events:?}"
    );
}

fn assert_calls_do_not_overlap(events: &[McpCallEvent], first_call_id: &str, second_call_id: &str) {
    let first_begin = event_index(events, McpCallEvent::Begin(first_call_id.to_string()));
    let first_end = event_index(events, McpCallEvent::End(first_call_id.to_string()));
    let second_begin = event_index(events, McpCallEvent::Begin(second_call_id.to_string()));
    let second_end = event_index(events, McpCallEvent::End(second_call_id.to_string()));
    assert!(
        first_end < second_begin || second_end < first_begin,
        "Codex Apps calls should run serially; saw events: {events:?}"
    );
}

fn event_index(events: &[McpCallEvent], needle: McpCallEvent) -> usize {
    events
        .iter()
        .position(|event| event == &needle)
        .expect("expected MCP call event")
}
