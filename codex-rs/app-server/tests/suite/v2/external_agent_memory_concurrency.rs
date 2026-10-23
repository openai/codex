//! Exercises import exclusion against an actual in-flight consolidation agent.

use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use codex_app_server_protocol::ExternalAgentConfigImportCompletedNotification;
use codex_app_server_protocol::ExternalAgentConfigImportResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput;
use codex_features::Feature;
use codex_protocol::MemoryVersion;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::sync::oneshot;

#[tokio::test]
async fn memory_import_retries_after_consolidation_without_partial_dual_write() -> Result<()> {
    // The foreground turn and background consolidation may request in either order.
    let (release_first, first_gate) = oneshot::channel();
    let (release_second, second_gate) = oneshot::channel();
    let (server, _) = start_streaming_sse_server(
        [first_gate, second_gate]
            .into_iter()
            .map(|gate| {
                vec![StreamingSseChunk {
                    gate: Some(gate),
                    body: sse(vec![
                        ev_response_created("response"),
                        ev_completed("response"),
                    ]),
                }]
            })
            .collect(),
    )
    .await;
    let home = TempDir::new()?;
    MockResponsesConfig::new(server.uri())
        .enable_feature(Feature::Sqlite)
        .enable_feature(Feature::MemoryTool)
        .enable_feature(Feature::ExternalAgentMemoryImport)
        .with_extra_config("[memories]\nversion = \"v2\"\ngenerate_memories = false\n")
        .write(home.path())?;

    let db = super::memory_reset::init_state_db(home.path()).await?;
    super::memory_reset::seed_stage1_output(&db, home.path(), MemoryVersion::V2).await?;
    let legacy_root = home.path().join("memories");
    let memory_root = home.path().join("memories_v2");
    std::fs::create_dir_all(&legacy_root)?;
    std::fs::create_dir_all(&memory_root)?;
    std::fs::write(
        memory_root.join("memory_summary.md"),
        "v1\n\n## User Profile\n\n## User preferences\n\n## General Tips\n\n## What's in Memory\n",
    )?;
    let source_project = home
        .path()
        .join(concat!(".", "cla", "ude", "/projects/project-a"));
    std::fs::create_dir_all(source_project.join("memory"))?;
    std::fs::write(source_project.join("memory/MEMORY.md"), "imported memory")?;
    std::fs::write(
        source_project.join("session.jsonl"),
        json!({
            "type": "user",
            "cwd": std::fs::canonicalize(home.path())?,
            "timestamp": "2026-07-13T00:00:00Z",
            "message": { "content": "remember this" },
        })
        .to_string(),
    )?;
    let home_dir = home.path().display().to_string();
    let mut app = TestAppServer::builder()
        .with_codex_home(home.path())
        .with_env_overrides(&[("HOME", Some(home_dir.as_str()))])
        .build_initialized()
        .await?;
    let request_id = app
        .send_thread_start_request_with_auto_env(ThreadStartParams::default())
        .await?;
    let response: ThreadStartResponse = app.read_response(request_id).await?;
    let request_id = app
        .send_turn_start_request(TurnStartParams {
            thread_id: response.thread.id,
            input: vec![UserInput::Text {
                text: "Hello".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _: TurnStartResponse = app.read_response(request_id).await?;

    // The request is sent only after the consolidation snapshot has been written.
    server.wait_for_request_count(/*count*/ 2).await;
    let diff_path = memory_root.join("phase2_workspace_diff.md");
    let consolidation_diff = std::fs::read(&diff_path)?;
    let params = json!({
        "migrationItems": [{
            "itemType": "MEMORY",
            "description": "Import memory",
            "cwd": null,
            "details": { "memory": ["project-a"] },
        }],
    });
    let request_id = app
        .send_raw_request("externalAgentConfig/import", Some(params.clone()))
        .await?;
    let response: ExternalAgentConfigImportResponse = app.read_response(request_id).await?;
    let completed: ExternalAgentConfigImportCompletedNotification = app
        .read_notification("externalAgentConfig/import/completed")
        .await?;
    assert_eq!(completed.import_id, response.import_id);
    assert_eq!(completed.item_type_results.len(), 1);
    let result = &completed.item_type_results[0];
    assert!(result.successes.is_empty());
    assert_eq!(result.failures.len(), 1);
    assert_eq!(
        result.failures[0].message,
        "memory workspace is busy; retry after consolidation or another import finishes"
    );
    let resource = "extensions/external_agent_import/resources/project-a/MEMORY.md";
    assert!(!legacy_root.join(resource).exists());
    assert!(!memory_root.join(resource).exists());
    assert_eq!(std::fs::read(&diff_path)?, consolidation_diff);

    release_first.send(()).expect("release first response");
    release_second.send(()).expect("release second response");
    // Wait on the cross-process lock instead of guessing when agent shutdown and
    // baseline finalization finish. This also catches releasing it too early.
    let lock_path = memory_root.with_extension("lock");
    tokio::task::spawn_blocking(move || -> std::io::Result<()> {
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(lock_path)?;
        lock.lock()?;
        Ok(())
    })
    .await??;
    assert_eq!(
        db.memories_for_version(MemoryVersion::V2)
            .await?
            .max_consolidated_thread_count()
            .await?,
        1
    );
    assert!(
        !codex_git_utils::diff_since_latest_init(&memory_root)
            .await?
            .has_changes()
    );

    let request_id = app
        .send_raw_request("externalAgentConfig/import", Some(params))
        .await?;
    let response: ExternalAgentConfigImportResponse = app.read_response(request_id).await?;
    let completed: ExternalAgentConfigImportCompletedNotification = app
        .read_notification("externalAgentConfig/import/completed")
        .await?;
    assert_eq!(completed.import_id, response.import_id);
    assert_eq!(completed.item_type_results.len(), 1);
    assert!(completed.item_type_results[0].failures.is_empty());
    assert_eq!(completed.item_type_results[0].successes.len(), 1);
    for root in [&legacy_root, &memory_root] {
        assert_eq!(
            std::fs::read_to_string(root.join(resource))?,
            "imported memory"
        );
        assert!(
            codex_git_utils::diff_since_latest_init(root)
                .await?
                .changes
                .iter()
                .any(|change| change.path == resource),
            "the next consolidation must still see the imported resource"
        );
    }
    app.shutdown_gracefully().await?;
    server.shutdown().await;
    Ok(())
}
