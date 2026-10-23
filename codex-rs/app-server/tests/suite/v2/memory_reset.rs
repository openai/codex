use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use chrono::Utc;
use codex_app_server_protocol::MemoryResetResponse;
use codex_features::Feature;
use codex_protocol::MemoryVersion;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use codex_state::Stage1JobClaimOutcome;
use codex_state::StateRuntime;
use codex_state::ThreadMetadataBuilder;
use codex_utils_absolute_path::test_support::PathExt;
use pretty_assertions::assert_eq;
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::time::timeout;
use uuid::Uuid;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn memory_reset_clears_memory_files_and_rows_preserves_threads() -> Result<()> {
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new("http://127.0.0.1:9")
        .with_root_config("suppress_unstable_features_warning = true")
        .enable_feature(Feature::Sqlite)
        .write(codex_home.path())?;
    let state_db = init_state_db(codex_home.path()).await?;

    let mut thread_ids = Vec::new();
    for version in [MemoryVersion::V1, MemoryVersion::V2] {
        let root = codex_home.path().join(version.directory_name());
        tokio::fs::create_dir_all(root.join("rollout_summaries")).await?;
        tokio::fs::write(root.join("memory_summary.md"), "v1\nstale memory\n").await?;
        tokio::fs::write(
            root.join("rollout_summaries/stale.md"),
            "stale rollout summary\n",
        )
        .await?;
        thread_ids.push(seed_stage1_output(&state_db, codex_home.path(), version).await?);
    }

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_raw_request("memory/reset", /*params*/ None)
        .await?;
    let _: MemoryResetResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(request_id)).await??;

    for version in [MemoryVersion::V1, MemoryVersion::V2] {
        let outputs = state_db
            .memories_for_version(version)
            .await?
            .list_stage1_outputs_for_global(/*n*/ 10)
            .await?;
        assert_eq!(outputs, Vec::new());
        let root = codex_home.path().join(version.directory_name());
        assert!(
            tokio::fs::read_dir(root)
                .await?
                .next_entry()
                .await?
                .is_none()
        );
    }
    for thread_id in thread_ids {
        assert_eq!(
            state_db.get_thread_memory_mode(thread_id).await?.as_deref(),
            Some("enabled")
        );
    }

    Ok(())
}

pub(super) async fn seed_stage1_output(
    state_db: &Arc<StateRuntime>,
    codex_home: &Path,
    version: MemoryVersion,
) -> Result<ThreadId> {
    let now = Utc::now();
    let thread_id = ThreadId::from_string(&Uuid::new_v4().to_string())?;
    let worker_id = ThreadId::from_string(&Uuid::new_v4().to_string())?;
    let mut builder = ThreadMetadataBuilder::new(
        thread_id,
        codex_home.join("sessions").join("test.jsonl"),
        now,
        SessionSource::Cli,
    );
    builder.updated_at = Some(now);
    builder.cwd = codex_home.to_path_buf();
    let metadata = builder.build("mock_provider");
    state_db.upsert_thread(&metadata).await?;

    let store = state_db.memories_for_version(version).await?;
    let claim = store
        .try_claim_stage1_job(
            thread_id,
            worker_id,
            now.timestamp(),
            /*lease_seconds*/ 3600,
            /*max_running_jobs*/ 64,
        )
        .await?;
    let Stage1JobClaimOutcome::Claimed { ownership_token } = claim else {
        anyhow::bail!("unexpected stage1 claim outcome: {claim:?}");
    };
    assert!(
        store
            .mark_stage1_job_succeeded(
                thread_id,
                ownership_token.as_str(),
                now.timestamp(),
                if version == MemoryVersion::V1 {
                    "raw memory"
                } else {
                    ""
                },
                "rollout summary",
                /*rollout_slug*/ None,
            )
            .await?,
        "stage1 success should be recorded"
    );
    store.enqueue_global_consolidation(now.timestamp()).await?;

    Ok(thread_id)
}

pub(super) async fn init_state_db(codex_home: &Path) -> Result<Arc<StateRuntime>> {
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

#[tokio::test]
async fn memory_status_requires_successful_v2_consolidation_and_resets() -> Result<()> {
    use codex_app_server_protocol::MemoryStatusResponse;
    use codex_state::Phase2JobClaimOutcome;
    let home = TempDir::new()?;
    MockResponsesConfig::new("http://127.0.0.1:9")
        .enable_feature(Feature::Sqlite)
        .write(home.path())?;
    let db = init_state_db(home.path()).await?;
    let mut server = TestAppServer::builder()
        .with_codex_home(home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;
    let params = serde_json::json!({"minConsolidatedThreads": 2});
    // Resetting a fresh profile must not create an unused V1 workspace.
    let request = server
        .send_raw_request("memory/reset", /*params*/ None)
        .await?;
    let _: MemoryResetResponse = server.read_response(request).await?;
    assert!(!home.path().join("memories").exists());
    let request = server
        .send_raw_request("memory/status", Some(params.clone()))
        .await?;
    let status: MemoryStatusResponse = server.read_response(request).await?;
    assert_eq!(
        status,
        MemoryStatusResponse {
            v2_consolidated_threads: 0,
            v2_ready: true
        }
    );
    // Even an empty V1 workspace retains the existing readiness requirements.
    tokio::fs::create_dir(home.path().join("memories")).await?;
    seed_stage1_output(&db, home.path(), MemoryVersion::V1).await?;
    let source = seed_stage1_output(&db, home.path(), MemoryVersion::V2).await?;
    seed_stage1_output(&db, home.path(), MemoryVersion::V2).await?;
    let request = server
        .send_raw_request("memory/status", Some(params.clone()))
        .await?;
    let status: MemoryStatusResponse = server.read_response(request).await?;
    assert_eq!(
        status,
        MemoryStatusResponse {
            v2_consolidated_threads: 0,
            v2_ready: false
        }
    );

    let store = db.memories_for_version(MemoryVersion::V2).await?;
    let outputs = store.list_stage1_outputs_for_global(/*n*/ 20).await?;
    let Phase2JobClaimOutcome::Claimed {
        ownership_token,
        input_watermark,
    } = store
        .try_claim_global_phase2_job(source, /*lease_seconds*/ 60)
        .await?
    else {
        panic!("claim phase 2")
    };
    assert!(
        !store
            .mark_global_phase2_job_succeeded("wrong owner", input_watermark, &outputs)
            .await?
    );
    assert_eq!(store.max_consolidated_thread_count().await?, 0);
    assert!(
        store
            .mark_global_phase2_job_succeeded(&ownership_token, input_watermark, &outputs)
            .await?
    );
    // A completed job without a usable artifact must not activate v2.
    let request = server
        .send_raw_request("memory/status", Some(params.clone()))
        .await?;
    let status: MemoryStatusResponse = server.read_response(request).await?;
    assert_eq!(
        status,
        MemoryStatusResponse {
            v2_consolidated_threads: 2,
            v2_ready: false
        }
    );
    let root = home.path().join("memories_v2");
    tokio::fs::create_dir_all(&root).await?;
    tokio::fs::write(root.join("memory_summary.md"), "v1\n## User Profile\nTest user\n## User preferences\nTest preference\n## General Tips\nTest tip\n## What's in Memory\nTest source\n").await?;
    db.delete_thread(source).await?;
    let request = server
        .send_raw_request("memory/status", Some(params.clone()))
        .await?;
    let status: MemoryStatusResponse = server.read_response(request).await?;
    assert_eq!(
        status,
        MemoryStatusResponse {
            v2_consolidated_threads: 2,
            v2_ready: true
        }
    );
    // The default threshold remains 20 even though a caller can choose a smaller cohort.
    let request = server
        .send_raw_request("memory/status", Some(serde_json::json!({})))
        .await?;
    let status: MemoryStatusResponse = server.read_response(request).await?;
    assert_eq!(
        status,
        MemoryStatusResponse {
            v2_consolidated_threads: 2,
            v2_ready: false
        }
    );
    let request = server
        .send_raw_request("memory/reset", /*params*/ None)
        .await?;
    let _: MemoryResetResponse = server.read_response(request).await?;
    let request = server
        .send_raw_request("memory/status", Some(params))
        .await?;
    let status: MemoryStatusResponse = server.read_response(request).await?;
    assert_eq!(
        status,
        MemoryStatusResponse {
            v2_consolidated_threads: 0,
            v2_ready: false
        }
    );
    Ok(())
}
