use std::collections::HashMap;
use std::time::Duration;

use codex_core::config_types::McpServerConfig;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::ReasoningSummary;
use core_test_support::responses;
use core_test_support::responses::mount_sse_once;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_with_timeout;
use escargot::CargoBuild;
use serde_json::Value;
use wiremock::matchers::any;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rmcp_tool_call_round_trip() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;

    let call_id = "call-123";
    let server_name = "rmcp";
    let tool_name = format!("{server_name}__echo");

    mount_sse_once(
        &server,
        any(),
        responses::sse(vec![
            serde_json::json!({
                "type": "response.created",
                "response": {"id": "resp-1"}
            }),
            responses::ev_function_call(call_id, &tool_name, "{\"message\":\"ping\"}"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    mount_sse_once(
        &server,
        any(),
        responses::sse(vec![
            responses::ev_assistant_message("msg-1", "rmcp echo tool completed successfully."),
            responses::ev_completed("resp-2"),
        ]),
    )
    .await;

    let expected_env_value = "propagated-env";
    let rmcp_test_server_bin = CargoBuild::new()
        .package("codex-rmcp-client")
        .bin("rmcp_test_server")
        .run()?
        .path()
        .to_string_lossy()
        .into_owned();

    let fixture = test_codex()
        .with_config(move |config| {
            config.use_experimental_use_rmcp_client = true;
            config.mcp_servers.insert(
                server_name.to_string(),
                McpServerConfig {
                    command: rmcp_test_server_bin.clone(),
                    args: Vec::new(),
                    env: Some(HashMap::from([(
                        "MCP_TEST_VALUE".to_string(),
                        expected_env_value.to_string(),
                    )])),
                    startup_timeout_sec: Some(Duration::from_secs(10)),
                    tool_timeout_sec: None,
                },
            );
        })
        .build(&server)
        .await?;
    let session_model = fixture.session_configured.model.clone();

    fixture
        .codex
        .submit(Op::UserTurn {
            items: vec![InputItem::Text {
                text: "call the rmcp echo tool".into(),
            }],
            final_output_json_schema: None,
            cwd: fixture.cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await?;

    eprintln!("waiting for mcp tool call begin event");
    let begin_event = wait_for_event_with_timeout(
        &fixture.codex,
        |ev| {
            eprintln!("ev: {ev:?}");
            matches!(ev, EventMsg::McpToolCallBegin(_))
        },
        Duration::from_secs(10),
    )
    .await;

    eprintln!("mcp tool call begin event: {begin_event:?}");
    let EventMsg::McpToolCallBegin(begin) = begin_event else {
        unreachable!("event guard guarantees McpToolCallBegin");
    };
    assert_eq!(begin.invocation.server, server_name);
    assert_eq!(begin.invocation.tool, "echo");

    let end_event = wait_for_event(&fixture.codex, |ev| {
        matches!(ev, EventMsg::McpToolCallEnd(_))
    })
    .await;
    eprintln!("end_event: {end_event:?}");
    let EventMsg::McpToolCallEnd(end) = end_event else {
        unreachable!("event guard guarantees McpToolCallEnd");
    };

    let result = end
        .result
        .as_ref()
        .expect("rmcp echo tool should return success");
    assert_eq!(result.is_error, Some(false));
    assert!(
        result.content.is_empty(),
        "content should default to an empty array"
    );

    let structured = result
        .structured_content
        .as_ref()
        .expect("structured content");
    let Value::Object(map) = structured else {
        panic!("structured content should be an object: {structured:?}");
    };
    let echo_value = map
        .get("echo")
        .and_then(Value::as_str)
        .expect("echo payload present");
    assert_eq!(echo_value, "ping");
    let env_value = map
        .get("env")
        .and_then(Value::as_str)
        .expect("env snapshot inserted");
    assert_eq!(env_value, expected_env_value);

    let task_complete_event =
        wait_for_event(&fixture.codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;
    eprintln!("task_complete_event: {task_complete_event:?}");

    server.verify().await;

    Ok(())
}
