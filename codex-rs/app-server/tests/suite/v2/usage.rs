use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::UsageRange;
use codex_app_server_protocol::UsageReadParams;
use codex_app_server_protocol::UsageReadResponse;
use codex_app_server_protocol::UserInput;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use std::path::Path;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn usage_read_returns_metrics_recorded_by_a_real_turn() -> Result<()> {
    let server = responses::start_mock_server().await;
    responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-usage"),
            responses::ev_assistant_message("msg-usage", "Done"),
            responses::ev_completed_with_tokens("resp-usage", /*total_tokens*/ 500),
        ]),
    )
    .await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;
    let skill_path = codex_home
        .path()
        .join(".agents/skills/usage-rpc-e2e/SKILL.md");
    std::fs::create_dir_all(skill_path.parent().expect("skill path should have parent"))?;
    std::fs::write(
        &skill_path,
        "---\nname: usage-rpc-e2e\ndescription: RPC usage test\n---\n\nRecord this skill.\n",
    )?;
    let skill_path = skill_path.canonicalize()?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let empty_request = mcp
        .send_usage_read_request(UsageReadParams {
            range: UsageRange::Day,
        })
        .await?;
    let empty_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(empty_request)),
    )
    .await??;
    let empty_report = to_response::<UsageReadResponse>(empty_response)?.report;
    assert_eq!(empty_report.total_tokens, 0);
    assert!(empty_report.skills.is_empty());

    let thread_request = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_request)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(thread_response)?;

    let turn_request = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id,
            input: vec![
                UserInput::Text {
                    text: "Use $usage-rpc-e2e".to_string(),
                    text_elements: Vec::new(),
                },
                UserInput::Skill {
                    name: "usage-rpc-e2e".to_string(),
                    path: skill_path.clone(),
                },
            ],
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_request)),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let daily = read_usage(&mut mcp, UsageRange::Day).await?;
    assert_eq!(daily.report.range, UsageRange::Day);
    assert_eq!(daily.report.total_tokens, 500);
    assert!(daily.report.tracked_from.is_some());
    assert_eq!(daily.report.skills.len(), 1);
    assert_eq!(daily.report.skills[0].label, "usage-rpc-e2e");
    assert!(daily.report.skills[0].attributed_tokens > 0);

    let weekly = read_usage(&mut mcp, UsageRange::Week).await?;
    assert_eq!(weekly.report.range, UsageRange::Week);
    assert_eq!(weekly.report.total_tokens, 500);
    assert_eq!(weekly.report.skills, daily.report.skills);

    Ok(())
}

async fn read_usage(mcp: &mut McpProcess, range: UsageRange) -> Result<UsageReadResponse> {
    let request = mcp
        .send_usage_read_request(UsageReadParams { range })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request)),
    )
    .await??;
    to_response::<UsageReadResponse>(response)
}

fn create_config_toml(codex_home: &Path, server_uri: &str) -> std::io::Result<()> {
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "read-only"
model_provider = "mock_provider"

[features]
sqlite = true

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#
        ),
    )
}
