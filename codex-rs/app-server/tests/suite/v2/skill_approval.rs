use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence;
use app_test_support::to_response;
use app_test_support::write_mock_responses_config_toml;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_core::features::Feature;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

fn write_skill_with_script(
    home: &Path,
    name: &str,
    script_body: &str,
) -> Result<std::path::PathBuf> {
    let skill_dir = home.join("skills").join(name);
    let scripts_dir = skill_dir.join("scripts");
    fs::create_dir_all(&scripts_dir)?;
    fs::write(
        skill_dir.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {name} skill\n---\n"),
    )?;
    let script_path = scripts_dir.join("run.py");
    fs::write(&script_path, script_body)?;
    Ok(script_path)
}

fn shell_command_response(tool_call_id: &str, command: &str) -> Result<String> {
    let arguments = serde_json::to_string(&json!({
        "command": command,
        "timeout_ms": 500,
    }))?;
    Ok(responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_function_call(tool_call_id, "shell_command", &arguments),
        responses::ev_completed("resp-1"),
    ]))
}

fn command_for_script(script_path: &Path) -> Result<String> {
    let runner = if cfg!(windows) { "python" } else { "python3" };
    let script_path = script_path.to_string_lossy().into_owned();
    Ok(shlex::try_join([runner, script_path.as_str()])?)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn skill_request_approval_round_trip_on_shell_command_skill_script_exec() -> Result<()> {
    let codex_home = tempfile::TempDir::new()?;
    let script_path = write_skill_with_script(codex_home.path(), "demo", "print('hello')")?;
    let tool_call_id = "skill-call";
    let command = command_for_script(&script_path)?;
    let server = create_mock_responses_server_sequence(vec![
        shell_command_response(tool_call_id, &command)?,
        create_final_assistant_message_sse_response("done")?,
    ])
    .await;
    write_mock_responses_config_toml(
        codex_home.path(),
        &server.uri(),
        &BTreeMap::from([(Feature::SkillApproval, true)]),
        8192,
        Some(false),
        "mock_provider",
        "compact",
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_start_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response(thread_start_resp)?;

    let turn_start_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "ask something".to_string(),
                text_elements: Vec::new(),
            }],
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let turn_start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_start_id)),
    )
    .await??;
    let TurnStartResponse { .. } = to_response::<TurnStartResponse>(turn_start_resp)?;

    let server_req = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;
    let ServerRequest::SkillRequestApproval { request_id, params } = server_req else {
        panic!("expected SkillRequestApproval request, got: {server_req:?}");
    };

    assert_eq!(params.item_id, tool_call_id);
    assert_eq!(params.skill_name, "demo");

    mcp.send_response(request_id, serde_json::json!({ "decision": "approve" }))
        .await?;

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    Ok(())
}
