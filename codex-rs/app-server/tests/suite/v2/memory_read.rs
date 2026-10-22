//! Exercises version selection through app-server and the model request boundary.

use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_features::Feature;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;

#[tokio::test]
async fn memory_read_version_selects_only_its_own_context() -> Result<()> {
    for (version, has_v1_workspace, expected, excluded) in [
        (Some("v1"), true, "legacy-only-marker", "v2-only-marker"),
        (Some("v2"), true, "v2-only-marker", "legacy-only-marker"),
        (None, true, "legacy-only-marker", "v2-only-marker"),
        (None, false, "v2-only-marker", "legacy-only-marker"),
    ] {
        let server = start_mock_server().await;
        let home = TempDir::new()?;
        MockResponsesConfig::new(&server.uri())
            .enable_feature(Feature::MemoryTool)
            .write(home.path())?;
        for (directory, marker) in [
            ("memories", "legacy-only-marker"),
            ("memories_v2", "v2-only-marker"),
        ] {
            if directory == "memories" && !has_v1_workspace {
                continue;
            }
            tokio::fs::create_dir_all(home.path().join(directory)).await?;
            tokio::fs::write(
                home.path().join(directory).join("memory_summary.md"),
                format!("v1\n{marker}\n"),
            )
            .await?;
        }
        let mock = mount_sse_once(
            &server,
            sse(vec![
                ev_response_created("response"),
                ev_assistant_message("message", "Done"),
                ev_completed("response"),
            ]),
        )
        .await;
        let mut app = TestAppServer::builder()
            .with_codex_home(home.path())
            .build_initialized()
            .await?;
        let mut config = std::collections::HashMap::from([(
            "memories.generate_memories".to_string(),
            json!(false),
        )]);
        if let Some(version) = version {
            config.insert("memories.version".to_string(), json!(version));
        }
        let start = app
            .send_thread_start_request_with_auto_env(ThreadStartParams {
                // Skip background consolidation so it cannot consume the read test's response.
                ephemeral: Some(true),
                config: Some(config),
                ..Default::default()
            })
            .await?;
        let response: ThreadStartResponse = app.read_response(start).await?;
        app.send_turn_start_request(TurnStartParams {
            thread_id: response.thread.id,
            input: vec![UserInput::Text {
                text: "Use my memory preferences".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
        let completed: TurnCompletedNotification = app.read_notification("turn/completed").await?;
        assert_eq!(completed.turn.status, TurnStatus::Completed);
        let request = mock.single_request();
        assert!(request.body_contains_text(expected));
        assert!(!request.body_contains_text(excluded));
    }
    Ok(())
}
