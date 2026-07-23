use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_fake_rollout;
use app_test_support::create_mock_responses_server_repeating_assistant;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ThreadMemoryMode;
use codex_app_server_protocol::ThreadMemoryModeSetParams;
use codex_app_server_protocol::ThreadMemoryModeSetResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_features::Feature;
use codex_protocol::ThreadId;
use codex_state::StateRuntime;
use codex_utils_absolute_path::test_support::PathExt;
use pretty_assertions::assert_eq;
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;

#[tokio::test]
async fn thread_memory_mode_set_updates_loaded_thread_state() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .with_root_config("suppress_unstable_features_warning = true")
        .enable_feature(Feature::Sqlite)
        .write(codex_home.path())?;
    let state_db = init_state_db(codex_home.path()).await?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    let ThreadStartResponse { thread, .. } = mcp
        .start_thread(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_uuid = ThreadId::from_string(&thread.id)?;

    let _: ThreadMemoryModeSetResponse = mcp
        .request(|request_id| ClientRequest::ThreadMemoryModeSet {
            request_id,
            params: ThreadMemoryModeSetParams {
                thread_id: thread.id,
                mode: ThreadMemoryMode::Disabled,
            },
        })
        .await?;

    let memory_mode = state_db.get_thread_memory_mode(thread_uuid).await?;
    assert_eq!(memory_mode.as_deref(), Some("disabled"));
    Ok(())
}

#[tokio::test]
async fn thread_memory_mode_set_updates_stored_thread_state() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .with_root_config("suppress_unstable_features_warning = true")
        .enable_feature(Feature::Sqlite)
        .write(codex_home.path())?;
    let state_db = init_state_db(codex_home.path()).await?;

    let thread_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-06T08-30-00",
        "2025-01-06T08:30:00Z",
        "Stored thread preview",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;
    let thread_uuid = ThreadId::from_string(&thread_id)?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;

    for mode in [ThreadMemoryMode::Disabled, ThreadMemoryMode::Enabled] {
        let _: ThreadMemoryModeSetResponse = mcp
            .request(|request_id| ClientRequest::ThreadMemoryModeSet {
                request_id,
                params: ThreadMemoryModeSetParams {
                    thread_id: thread_id.clone(),
                    mode,
                },
            })
            .await?;
    }

    let memory_mode = state_db.get_thread_memory_mode(thread_uuid).await?;
    assert_eq!(memory_mode.as_deref(), Some("enabled"));
    Ok(())
}

async fn init_state_db(codex_home: &Path) -> Result<Arc<StateRuntime>> {
    let state_db = StateRuntime::init(
        codex_state::SqliteConfig::new_for_testing(codex_home.abs()),
        "mock_provider".into(),
    )
    .await?;
    state_db
        .mark_backfill_complete(/*last_watermark*/ None)
        .await?;
    Ok(state_db)
}
