use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadStartedNotification;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_core::config::set_project_trust_level;
use codex_protocol::config_types::TrustLevel;
use codex_protocol::openai_models::ReasoningEffort;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use std::path::Path;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn thread_start_creates_thread_and_emits_started() -> Result<()> {
    // Provide a mock server and config so model wiring is valid.
    let server = create_mock_responses_server_repeating_assistant("Done").await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    // Start server and initialize.
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    // Start a v2 thread with an explicit model override.
    let req_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("gpt-5.1".to_string()),
            ..Default::default()
        })
        .await?;

    // Expect a proper JSON-RPC response with a thread id.
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(req_id)),
    )
    .await??;
    let ThreadStartResponse {
        thread,
        model_provider,
        ..
    } = to_response::<ThreadStartResponse>(resp)?;
    assert!(!thread.id.is_empty(), "thread id should not be empty");
    assert!(
        thread.preview.is_empty(),
        "new threads should start with an empty preview"
    );
    assert_eq!(model_provider, "mock_provider");
    assert!(
        thread.created_at > 0,
        "created_at should be a positive UNIX timestamp"
    );

    // A corresponding thread/started notification should arrive.
    let notif: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("thread/started"),
    )
    .await??;
    let started: ThreadStartedNotification =
        serde_json::from_value(notif.params.expect("params must be present"))?;
    assert_eq!(started.thread, thread);

    Ok(())
}

#[tokio::test]
async fn thread_start_respects_project_config_from_cwd() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let workspace = TempDir::new()?;
    let project_config_dir = workspace.path().join(".codex");
    std::fs::create_dir_all(&project_config_dir)?;
    std::fs::write(
        project_config_dir.join("config.toml"),
        r#"
model_reasoning_effort = "high"
"#,
    )?;
    set_project_trust_level(codex_home.path(), workspace.path(), TrustLevel::Trusted)?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let req_id = mcp
        .send_thread_start_request(ThreadStartParams {
            cwd: Some(workspace.path().to_string_lossy().into_owned()),
            ..Default::default()
        })
        .await?;

    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(req_id)),
    )
    .await??;
    let ThreadStartResponse {
        reasoning_effort, ..
    } = to_response::<ThreadStartResponse>(resp)?;

    assert_eq!(reasoning_effort, Some(ReasoningEffort::High));
    Ok(())
}

#[tokio::test]
async fn thread_start_additional_skills_roots_apply_to_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_assistant_message("msg-1", "Done"),
        responses::ev_completed("resp-1"),
    ]);
    let response_mock = responses::mount_sse_once(&server, body).await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let additional_root = TempDir::new()?;
    let skill_body = "extra skill body";
    let skill_path = write_skill_at(
        additional_root.path(),
        "desktop-extra",
        "desktop-extra-skill",
        "desktop extra skill",
        skill_body,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_req = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            additional_skills_roots: Some(vec![additional_root.path().to_path_buf()]),
            ..Default::default()
        })
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(thread_resp)?;

    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "please run $desktop-extra-skill".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req)),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let request = response_mock.single_request();
    let user_texts = request.message_input_texts("user");
    let skill_path = std::fs::canonicalize(skill_path)?;
    let skill_path_str = skill_path.to_string_lossy();
    assert!(
        user_texts.iter().any(|text| {
            text.contains("<skill>\n<name>desktop-extra-skill</name>")
                && text.contains(skill_body)
                && text.contains(skill_path_str.as_ref())
        }),
        "expected skill instructions from additional skills roots in user input, got {user_texts:?}"
    );

    Ok(())
}

// Helper to create a config.toml pointing at the mock model server.
fn create_config_toml(codex_home: &Path, server_uri: &str) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "read-only"

model_provider = "mock_provider"

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

fn write_skill_at(
    root: &Path,
    dir: &str,
    name: &str,
    description: &str,
    body: &str,
) -> std::io::Result<std::path::PathBuf> {
    let skill_dir = root.join(dir);
    std::fs::create_dir_all(&skill_dir)?;
    let contents = format!("---\nname: {name}\ndescription: {description}\n---\n\n{body}\n");
    let skill_path = skill_dir.join("SKILL.md");
    std::fs::write(&skill_path, contents)?;
    Ok(skill_path)
}
