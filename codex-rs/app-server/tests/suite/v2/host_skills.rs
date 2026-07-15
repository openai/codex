use std::time::Duration;

use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SkillsExtraRootsSetParams;
use codex_app_server_protocol::SkillsExtraRootsSetResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::UserInput;
use codex_utils_absolute_path::AbsolutePathBuf;
use core_test_support::responses;
use core_test_support::skip_if_remote;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const READ_TIMEOUT: Duration = Duration::from_secs(30);
const INITIAL_SKILL_DESCRIPTION: &str = "INITIAL_HOST_SKILL_DESCRIPTION";
const RUNTIME_SKILL_DESCRIPTION: &str = "RUNTIME_HOST_SKILL_DESCRIPTION";

#[tokio::test]
async fn host_skill_catalog_refreshes_once_when_skills_change() -> Result<()> {
    skip_if_remote!(
        Ok(()),
        "host-local skill changes are not visible to remote executors"
    );

    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_sequence(
        &server,
        (1..=3)
            .map(|index| {
                let response_id = format!("resp-{index}");
                let message_id = format!("msg-{index}");
                responses::sse(vec![
                    responses::ev_response_created(&response_id),
                    responses::ev_assistant_message(&message_id, "Done"),
                    responses::ev_completed(&response_id),
                ])
            })
            .collect(),
    )
    .await;

    let codex_home = TempDir::new()?;
    let extra_root = TempDir::new()?;
    let extra_skills_root = extra_root.path().join("skills");
    write_skill(
        &extra_skills_root,
        "initial-host-skill",
        INITIAL_SKILL_DESCRIPTION,
    )?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "read-only"
model_provider = "mock_provider"

[skills]
include_instructions = true

[skills.bundled]
enabled = false

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#,
            server.uri()
        ),
    )?;

    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build()
        .await?;
    timeout(READ_TIMEOUT, app_server.initialize()).await??;

    let request_id = app_server
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response(response)?;

    set_extra_roots(&mut app_server, &extra_skills_root).await?;
    run_turn(&mut app_server, &thread.id, "Initial catalog").await?;

    write_skill(
        &extra_skills_root,
        "runtime-host-skill",
        RUNTIME_SKILL_DESCRIPTION,
    )?;
    set_extra_roots(&mut app_server, &extra_skills_root).await?;
    run_turn(&mut app_server, &thread.id, "After install").await?;
    run_turn(&mut app_server, &thread.id, "Unchanged follow-up").await?;

    let requests = response_mock.requests();
    assert_eq!(3, requests.len());
    let marker_counts = |marker| {
        requests
            .iter()
            .map(|request| {
                request
                    .message_input_texts("developer")
                    .iter()
                    .map(|text| text.matches(marker).count())
                    .sum::<usize>()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(vec![1, 2, 2], marker_counts(INITIAL_SKILL_DESCRIPTION));
    assert_eq!(vec![0, 1, 1], marker_counts(RUNTIME_SKILL_DESCRIPTION));

    Ok(())
}

fn write_skill(root: &std::path::Path, name: &str, description: &str) -> Result<()> {
    let skill_dir = root.join(name);
    std::fs::create_dir_all(&skill_dir)?;
    std::fs::write(
        skill_dir.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {description}\n---\n\n# {name}\n"),
    )?;
    Ok(())
}

async fn set_extra_roots(app_server: &mut TestAppServer, root: &std::path::Path) -> Result<()> {
    let request_id = app_server
        .send_skills_extra_roots_set_request(SkillsExtraRootsSetParams {
            extra_roots: vec![AbsolutePathBuf::from_absolute_path(root)?],
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let _: SkillsExtraRootsSetResponse = to_response(response)?;
    timeout(
        READ_TIMEOUT,
        app_server.read_stream_until_notification_message("skills/changed"),
    )
    .await??;
    Ok(())
}

async fn run_turn(app_server: &mut TestAppServer, thread_id: &str, prompt: &str) -> Result<()> {
    let request_id = app_server
        .send_turn_start_request(TurnStartParams {
            thread_id: thread_id.to_string(),
            input: vec![UserInput::Text {
                text: prompt.to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    timeout(
        READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    timeout(
        READ_TIMEOUT,
        app_server.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    Ok(())
}
