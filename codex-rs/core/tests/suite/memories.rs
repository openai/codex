use anyhow::Result;
use chrono::Duration as ChronoDuration;
use chrono::Utc;
use codex_core::features::Feature;
use codex_protocol::ThreadId;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SessionSource;
use core_test_support::responses::ResponseMock;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::time::Duration;
use tokio::time::Instant;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn memories_startup_phase2_tracks_added_and_removed_inputs_across_runs() -> Result<()> {
    let server = start_mock_server().await;
    let home = Arc::new(TempDir::new()?);
    let db = init_state_db(&home).await?;

    let now = Utc::now();
    let thread_a = seed_stage1_output(
        db.as_ref(),
        home.path(),
        now - ChronoDuration::hours(2),
        "raw memory A",
        "rollout summary A",
        "rollout-a",
    )
    .await?;

    let first_phase2 = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-phase2-1"),
            ev_assistant_message("msg-phase2-1", "phase2 complete"),
            ev_completed("resp-phase2-1"),
        ]),
    )
    .await;

    let first = build_test_codex(&server, home.clone()).await?;
    let first_request = wait_for_single_request(&first_phase2).await;
    let first_prompt = phase2_prompt_text(&first_request);
    assert!(
        first_prompt.contains("- selected inputs this run: 1"),
        "expected selected count in first prompt: {first_prompt}"
    );
    assert!(
        first_prompt.contains("- newly added since the last successful Phase 2 run: 1"),
        "expected added count in first prompt: {first_prompt}"
    );
    assert!(
        first_prompt.contains("- removed from the last successful Phase 2 run: 0"),
        "expected removed count in first prompt: {first_prompt}"
    );
    assert!(
        first_prompt.contains(&format!("- [added] thread_id={thread_a},")),
        "expected thread A to be marked added: {first_prompt}"
    );
    assert!(
        first_prompt.contains("Removed from the last successful Phase 2 selection:\n- none"),
        "expected no removed items in first prompt: {first_prompt}"
    );

    wait_for_phase2_success(db.as_ref(), thread_a).await?;
    let memory_root = home.path().join("memories");
    let raw_memories = tokio::fs::read_to_string(memory_root.join("raw_memories.md")).await?;
    assert!(raw_memories.contains("raw memory A"));
    assert!(!raw_memories.contains("raw memory B"));
    let rollout_summaries = read_rollout_summary_bodies(&memory_root).await?;
    assert_eq!(rollout_summaries.len(), 1);
    assert!(rollout_summaries[0].contains("rollout summary A"));

    shutdown_test_codex(&first).await?;

    let thread_b = seed_stage1_output(
        db.as_ref(),
        home.path(),
        now - ChronoDuration::hours(1),
        "raw memory B",
        "rollout summary B",
        "rollout-b",
    )
    .await?;

    let second_phase2 = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-phase2-2"),
            ev_assistant_message("msg-phase2-2", "phase2 complete"),
            ev_completed("resp-phase2-2"),
        ]),
    )
    .await;

    let second = build_test_codex(&server, home.clone()).await?;
    let second_request = wait_for_single_request(&second_phase2).await;
    let second_prompt = phase2_prompt_text(&second_request);
    assert!(
        second_prompt.contains("- selected inputs this run: 1"),
        "expected selected count in second prompt: {second_prompt}"
    );
    assert!(
        second_prompt.contains("- newly added since the last successful Phase 2 run: 1"),
        "expected added count in second prompt: {second_prompt}"
    );
    assert!(
        second_prompt.contains("- removed from the last successful Phase 2 run: 1"),
        "expected removed count in second prompt: {second_prompt}"
    );
    assert!(
        second_prompt.contains(&format!("- [added] thread_id={thread_b},")),
        "expected thread B to be marked added: {second_prompt}"
    );
    assert!(
        second_prompt.contains(&format!("- thread_id={thread_a},")),
        "expected thread A to be marked removed: {second_prompt}"
    );

    wait_for_phase2_success(db.as_ref(), thread_b).await?;
    let raw_memories = tokio::fs::read_to_string(memory_root.join("raw_memories.md")).await?;
    assert!(raw_memories.contains("raw memory B"));
    assert!(raw_memories.contains("raw memory A"));
    let rollout_summaries = read_rollout_summary_bodies(&memory_root).await?;
    assert_eq!(rollout_summaries.len(), 2);
    assert!(
        rollout_summaries
            .iter()
            .any(|summary| summary.contains("rollout summary B"))
    );
    assert!(
        rollout_summaries
            .iter()
            .any(|summary| summary.contains("rollout summary A"))
    );

    shutdown_test_codex(&second).await?;
    Ok(())
}

async fn build_test_codex(server: &wiremock::MockServer, home: Arc<TempDir>) -> Result<TestCodex> {
    let mut builder = test_codex().with_home(home).with_config(|config| {
        config.features.enable(Feature::Sqlite);
        config.features.enable(Feature::MemoryTool);
        config.memories.max_raw_memories_for_global = 1;
    });
    builder.build(server).await
}

async fn init_state_db(home: &Arc<TempDir>) -> Result<Arc<codex_state::StateRuntime>> {
    let db =
        codex_state::StateRuntime::init(home.path().to_path_buf(), "test-provider".into(), None)
            .await?;
    db.mark_backfill_complete(None).await?;
    Ok(db)
}

async fn seed_stage1_output(
    db: &codex_state::StateRuntime,
    codex_home: &Path,
    updated_at: chrono::DateTime<Utc>,
    raw_memory: &str,
    rollout_summary: &str,
    rollout_slug: &str,
) -> Result<ThreadId> {
    let thread_id = ThreadId::new();
    let mut metadata_builder = codex_state::ThreadMetadataBuilder::new(
        thread_id,
        codex_home.join(format!("rollout-{thread_id}.jsonl")),
        updated_at,
        SessionSource::Cli,
    );
    metadata_builder.cwd = codex_home.join(format!("workspace-{rollout_slug}"));
    metadata_builder.model_provider = Some("test-provider".to_string());
    let metadata = metadata_builder.build("test-provider");
    db.upsert_thread(&metadata).await?;

    let claim = db
        .try_claim_stage1_job(
            thread_id,
            ThreadId::new(),
            updated_at.timestamp(),
            3_600,
            64,
        )
        .await?;
    let ownership_token = match claim {
        codex_state::Stage1JobClaimOutcome::Claimed { ownership_token } => ownership_token,
        other => panic!("unexpected stage-1 claim outcome: {other:?}"),
    };

    assert!(
        db.mark_stage1_job_succeeded(
            thread_id,
            &ownership_token,
            updated_at.timestamp(),
            raw_memory,
            rollout_summary,
            Some(rollout_slug),
        )
        .await?,
        "stage-1 success should enqueue global consolidation"
    );

    Ok(thread_id)
}

async fn wait_for_single_request(mock: &ResponseMock) -> ResponsesRequest {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let requests = mock.requests();
        if let Some(request) = requests.into_iter().next() {
            return request;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for phase2 request"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[allow(clippy::expect_used)]
fn phase2_prompt_text(request: &ResponsesRequest) -> String {
    request
        .message_input_texts("user")
        .into_iter()
        .find(|text| text.contains("Current selected Phase 1 inputs:"))
        .expect("phase2 prompt text")
}

async fn wait_for_phase2_success(
    db: &codex_state::StateRuntime,
    expected_thread_id: ThreadId,
) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let selection = db.get_phase2_input_selection(1).await?;
        if selection.selected.len() == 1
            && selection.selected[0].thread_id == expected_thread_id
            && selection.retained_thread_ids == vec![expected_thread_id]
            && selection.removed.is_empty()
        {
            return Ok(());
        }

        assert!(
            Instant::now() < deadline,
            "timed out waiting for phase2 success for {expected_thread_id}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn read_rollout_summary_bodies(memory_root: &Path) -> Result<Vec<String>> {
    let mut dir = tokio::fs::read_dir(memory_root.join("rollout_summaries")).await?;
    let mut summaries = Vec::new();
    while let Some(entry) = dir.next_entry().await? {
        summaries.push(tokio::fs::read_to_string(entry.path()).await?);
    }
    summaries.sort();
    Ok(summaries)
}

async fn shutdown_test_codex(test: &TestCodex) -> Result<()> {
    test.codex.submit(Op::Shutdown {}).await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::ShutdownComplete)).await;
    Ok(())
}
