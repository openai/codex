use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::SkillDependenciesApprovalDecision;
use codex_app_server_protocol::SkillDependenciesRequestApprovalResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::openai_models::ReasoningEffort;
use pretty_assertions::assert_eq;
use std::path::Path;
use std::path::PathBuf;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn skill_dependencies_request_approval_round_trip() -> Result<()> {
    let codex_home = tempfile::TempDir::new()?;
    let responses = vec![create_final_assistant_message_sse_response("done")?];
    let server = create_mock_responses_server_sequence(responses).await;
    create_config_toml(codex_home.path(), &server.uri())?;
    let skill_path = write_skill_files(codex_home.path())?;
    let skill_path = std::fs::canonicalize(&skill_path).unwrap_or(skill_path);

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
            input: vec![
                V2UserInput::Text {
                    text: "use the skill".to_string(),
                    text_elements: Vec::new(),
                },
                V2UserInput::Skill {
                    name: "dep-skill".to_string(),
                    path: skill_path,
                },
            ],
            model: Some("mock-model".to_string()),
            effort: Some(ReasoningEffort::Medium),
            collaboration_mode: Some(CollaborationMode {
                mode: ModeKind::Plan,
                settings: Settings {
                    model: "mock-model".to_string(),
                    reasoning_effort: Some(ReasoningEffort::Medium),
                    developer_instructions: None,
                },
            }),
            ..Default::default()
        })
        .await?;
    let turn_start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_start_id)),
    )
    .await??;
    let TurnStartResponse { turn, .. } = to_response(turn_start_resp)?;

    let server_req = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;
    let ServerRequest::SkillDependenciesRequestApproval { request_id, params } = server_req else {
        panic!("expected SkillDependenciesRequestApproval request, got: {server_req:?}");
    };

    assert_eq!(params.thread_id, thread.id);
    assert_eq!(params.turn_id, turn.id);
    assert_eq!(params.item_id, "skill-dep-skill-mcp-deps");
    assert_eq!(params.header, "Missing MCP dependency");
    assert_eq!(
        params.question,
        "The \"dep-skill\" skill depends on MCP server(s) that are not loaded: example-mcp. What would you like to do?"
    );
    assert_eq!(params.run_anyway.label, "Run anyway");
    assert_eq!(
        params.run_anyway.description,
        "Proceed without installing. The skill may not work as expected."
    );
    assert_eq!(params.install.label, "Install example-mcp");
    assert_eq!(
        params.install.description,
        "Install and configure the example-mcp MCP server."
    );

    mcp.send_response(
        request_id,
        serde_json::to_value(SkillDependenciesRequestApprovalResponse {
            decision: SkillDependenciesApprovalDecision::RunAnyway,
        })?,
    )
    .await?;

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("codex/event/task_complete"),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    Ok(())
}

fn create_config_toml(codex_home: &Path, server_uri: &str) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "mock-model"
approval_policy = "untrusted"
sandbox_mode = "read-only"

model_provider = "mock_provider"

[features]
collaboration_modes = true

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

fn write_skill_files(codex_home: &Path) -> std::io::Result<PathBuf> {
    let skill_dir = codex_home.join("skills").join("dep-skill");
    std::fs::create_dir_all(&skill_dir)?;

    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: dep-skill\ndescription: Skill dependency test\n---\n\nTest skill.\n",
    )?;

    std::fs::write(
        skill_dir.join("SKILL.toml"),
        "[dependencies]\n[[dependencies.tools]]\ntype = \"mcp\"\nvalue = \"example-mcp\"\ndescription = \"Example MCP\"\ntransport = \"streamable_http\"\nurl = \"https://example.com\"\n",
    )?;

    Ok(skill_dir.join("SKILL.md"))
}
