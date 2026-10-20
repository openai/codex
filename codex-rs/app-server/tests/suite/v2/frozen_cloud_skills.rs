//! Frozen cloud sources use the native catalog and read handler without an MCP server.

use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::UserInput;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::json;

#[tokio::test]
async fn frozen_cloud_catalog_and_read_use_native_skills_interface() -> Result<()> {
    let server = responses::start_mock_server().await;
    let responses = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("read-skill"),
                responses::ev_function_call_with_namespace(
                    "read",
                    "skills",
                    "read",
                    &json!({"package": "c0/work"}).to_string(),
                ),
                responses::ev_completed("read-skill"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("done"),
                responses::ev_assistant_message("answer", "Read it."),
                responses::ev_completed("done"),
            ]),
        ],
    )
    .await;
    let home = tempfile::tempdir()?;
    let snapshot = home.path().join("cloud skills.json");
    std::fs::write(
        &snapshot,
        json!({"skills": [{
            "package": "skill://demo/work", "name": "demo:work",
            "description": "Do the work.", "alias_root": "skill://demo",
            "main_resource": "skill://demo/work/SKILL.md",
            "resources": {"skill://demo/work/SKILL.md": "# Work\nExact frozen contents.\n"}
        }]})
        .to_string(),
    )?;
    std::fs::write(
        home.path().join("config.toml"),
        format!(
            r#"
model = "mock-model"
model_provider = "mock_provider"
approval_policy = "never"
sandbox_mode = "read-only"
[skills]
include_instructions = true
[skills.bundled]
enabled = false
[cloud.skills]
enabled = true
[model_providers.mock_provider]
name = "Mock provider"
base_url = "{}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#,
            server.uri(),
        ),
    )?;
    let mut app = TestAppServer::builder()
        .with_codex_home(home.path())
        // Cloud parents have no local executor; local threads intentionally hide cloud skills.
        .without_auto_env()
        .with_args(&[
            "--cloud-skills-snapshot",
            snapshot.to_str().expect("snapshot path must be UTF-8"),
        ])
        .build()
        .await?;
    app.initialize().await?;
    // The process owns a frozen snapshot even if its source file later disappears.
    std::fs::remove_file(snapshot)?;
    let id = app
        .send_thread_start_request(ThreadStartParams {
            environments: Some(Vec::new()),
            ..Default::default()
        })
        .await?;
    let response = app
        .read_stream_until_response_message(RequestId::Integer(id))
        .await?;
    let ThreadStartResponse { thread, .. } = to_response(response)?;
    app.start_turn_and_wait_for_completion(TurnStartParams {
        thread_id: thread.id,
        input: vec![UserInput::Text {
            text: "Read the work skill.".into(),
            text_elements: vec![],
        }],
        ..Default::default()
    })
    .await?;
    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    let catalog = requests[0].message_input_texts("developer").join("\n");
    assert!(catalog.contains("- `c0` = `skill://demo`"));
    assert!(catalog.contains("(cloud package: c0/work)"));
    let result: serde_json::Value = serde_json::from_str(
        &requests[1]
            .function_call_output_text("read")
            .expect("native read output"),
    )?;
    assert_eq!(
        result,
        json!({
            "resource": "skill://demo/work/SKILL.md",
            "contents": "# Work\nExact frozen contents.\n",
            "next_cursor": null,
        })
    );
    Ok(())
}

#[test_case::test_case(None; "missing snapshot")]
#[test_case::test_case(Some("not JSON"); "invalid snapshot")]
#[tokio::test]
async fn frozen_cloud_snapshot_errors_fail_startup(contents: Option<&str>) -> Result<()> {
    let home = tempfile::tempdir()?;
    let snapshot = home.path().join("cloud-skills.json");
    if let Some(contents) = contents {
        std::fs::write(&snapshot, contents)?;
    }
    let mut app = TestAppServer::builder()
        .with_codex_home(home.path())
        .without_auto_env()
        .with_args(&[
            "--cloud-skills-snapshot",
            snapshot.to_str().expect("snapshot path must be UTF-8"),
        ])
        .build()
        .await?;
    assert!(app.initialize().await.is_err());
    Ok(())
}
