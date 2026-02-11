use crate::codex::Session;
use crate::config::Config;
use crate::memories::memory_root;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::user_input::UserInput;
use std::sync::Arc;
use tracing::debug;
use tracing::info;
use tracing::warn;

use super::super::MAX_RAW_MEMORIES_FOR_GLOBAL;
use super::super::MEMORY_CONSOLIDATION_SUBAGENT_LABEL;
use super::super::PHASE_TWO_JOB_LEASE_SECONDS;
use super::super::PHASE_TWO_JOB_RETRY_DELAY_SECONDS;
use super::super::prompts::build_consolidation_prompt;
use super::super::storage::rebuild_raw_memories_file_from_memories;
use super::super::storage::sync_rollout_summaries_from_memories;
use super::super::storage::wipe_consolidation_outputs;
use super::phase2::spawn_phase2_completion_task;

pub(super) async fn run_global_memory_consolidation(
    session: &Arc<Session>,
    config: Arc<Config>,
) -> bool {
    let Some(state_db) = session.services.state_db.as_deref() else {
        warn!("state db unavailable; skipping global memory consolidation");
        return false;
    };

    let claim = match state_db
        .try_claim_global_phase2_job(session.conversation_id, PHASE_TWO_JOB_LEASE_SECONDS)
        .await
    {
        Ok(claim) => claim,
        Err(err) => {
            warn!("state db try_claim_global_phase2_job failed during memories startup: {err}");
            return false;
        }
    };
    let (ownership_token, claimed_watermark) = match claim {
        codex_state::Phase2JobClaimOutcome::Claimed {
            ownership_token,
            input_watermark,
        } => (ownership_token, input_watermark),
        codex_state::Phase2JobClaimOutcome::SkippedNotDirty => {
            debug!("memory phase-2 global lock is up-to-date; skipping consolidation");
            return false;
        }
        codex_state::Phase2JobClaimOutcome::SkippedRunning => {
            debug!("memory phase-2 global consolidation already running; skipping");
            return false;
        }
    };

    let latest_memories = match state_db
        .list_stage1_outputs_for_global(MAX_RAW_MEMORIES_FOR_GLOBAL)
        .await
    {
        Ok(memories) => memories,
        Err(err) => {
            warn!("state db list_stage1_outputs_for_global failed during consolidation: {err}");
            let _ = state_db
                .mark_global_phase2_job_failed(
                    &ownership_token,
                    "failed to read stage-1 outputs before global consolidation",
                    PHASE_TWO_JOB_RETRY_DELAY_SECONDS,
                )
                .await;
            return false;
        }
    };
    if latest_memories.is_empty() {
        debug!("memory phase-2 has no stage-1 outputs; skipping global consolidation");
        let _ = state_db
            .mark_global_phase2_job_succeeded(&ownership_token, claimed_watermark)
            .await;
        return false;
    };

    let root = memory_root(&config.codex_home);
    let materialized_watermark = latest_memories
        .iter()
        .map(|memory| memory.source_updated_at.timestamp())
        .max()
        .unwrap_or(claimed_watermark);

    if let Err(err) = sync_rollout_summaries_from_memories(&root, &latest_memories).await {
        warn!("failed syncing phase-1 rollout summaries for global consolidation: {err}");
        let _ = state_db
            .mark_global_phase2_job_failed(
                &ownership_token,
                "failed syncing phase-1 rollout summaries",
                PHASE_TWO_JOB_RETRY_DELAY_SECONDS,
            )
            .await;
        return false;
    }

    if let Err(err) = rebuild_raw_memories_file_from_memories(&root, &latest_memories).await {
        warn!("failed rebuilding raw memories aggregate for global consolidation: {err}");
        let _ = state_db
            .mark_global_phase2_job_failed(
                &ownership_token,
                "failed rebuilding raw memories aggregate",
                PHASE_TWO_JOB_RETRY_DELAY_SECONDS,
            )
            .await;
        return false;
    }

    if let Err(err) = wipe_consolidation_outputs(&root).await {
        warn!("failed to wipe previous global consolidation outputs: {err}");
        let _ = state_db
            .mark_global_phase2_job_failed(
                &ownership_token,
                "failed to wipe previous consolidation outputs",
                PHASE_TWO_JOB_RETRY_DELAY_SECONDS,
            )
            .await;
        return false;
    }

    let prompt = build_consolidation_prompt(&root);
    let input = vec![UserInput::Text {
        text: prompt,
        text_elements: vec![],
    }];
    let mut consolidation_config = config.as_ref().clone();
    consolidation_config.cwd = root.clone();
    let source = SessionSource::SubAgent(SubAgentSource::Other(
        MEMORY_CONSOLIDATION_SUBAGENT_LABEL.to_string(),
    ));

    match session
        .services
        .agent_control
        .spawn_agent(consolidation_config, input, Some(source))
        .await
    {
        Ok(consolidation_agent_id) => {
            info!(
                "memory phase-2 global consolidation agent started: agent_id={consolidation_agent_id}"
            );
            spawn_phase2_completion_task(
                session.as_ref(),
                ownership_token,
                materialized_watermark,
                consolidation_agent_id,
            );
            true
        }
        Err(err) => {
            warn!("failed to spawn global memory consolidation agent: {err}");
            let _ = state_db
                .mark_global_phase2_job_failed(
                    &ownership_token,
                    "failed to spawn consolidation agent",
                    PHASE_TWO_JOB_RETRY_DELAY_SECONDS,
                )
                .await;
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::run_global_memory_consolidation;
    use crate::CodexAuth;
    use crate::ThreadManager;
    use crate::agent::control::AgentControl;
    use crate::codex::Session;
    use crate::codex::make_session_and_context;
    use crate::config::Config;
    use crate::config::test_config;
    use chrono::Utc;
    use codex_protocol::ThreadId;
    use codex_protocol::protocol::Op;
    use codex_protocol::protocol::SessionSource;
    use codex_state::Phase2JobClaimOutcome;
    use codex_state::ThreadMetadataBuilder;
    use pretty_assertions::assert_eq;
    use std::sync::Arc;
    use tempfile::TempDir;

    struct DispatchHarness {
        _codex_home: TempDir,
        config: Arc<Config>,
        session: Arc<Session>,
        manager: ThreadManager,
        state_db: Arc<codex_state::StateRuntime>,
    }

    impl DispatchHarness {
        async fn new() -> Self {
            let codex_home = tempfile::tempdir().expect("create temp codex home");
            let mut config = test_config();
            config.codex_home = codex_home.path().to_path_buf();
            config.cwd = config.codex_home.clone();
            let config = Arc::new(config);

            let state_db = codex_state::StateRuntime::init(
                config.codex_home.clone(),
                config.model_provider_id.clone(),
                None,
            )
            .await
            .expect("initialize state db");

            let manager = ThreadManager::with_models_provider_and_home(
                CodexAuth::from_api_key("dummy"),
                config.model_provider.clone(),
                config.codex_home.clone(),
            );
            let (mut session, _turn_context) = make_session_and_context().await;
            session.services.state_db = Some(Arc::clone(&state_db));
            session.services.agent_control = manager.agent_control();

            Self {
                _codex_home: codex_home,
                config,
                session: Arc::new(session),
                manager,
                state_db,
            }
        }

        async fn seed_stage1_output(&self, source_updated_at: i64) {
            let thread_id = ThreadId::new();
            let mut metadata_builder = ThreadMetadataBuilder::new(
                thread_id,
                self.config
                    .codex_home
                    .join(format!("rollout-{thread_id}.jsonl")),
                Utc::now(),
                SessionSource::Cli,
            );
            metadata_builder.cwd = self.config.cwd.clone();
            metadata_builder.model_provider = Some(self.config.model_provider_id.clone());
            let metadata = metadata_builder.build(&self.config.model_provider_id);

            self.state_db
                .upsert_thread(&metadata)
                .await
                .expect("upsert thread metadata");

            let claim = self
                .state_db
                .try_claim_stage1_job(
                    thread_id,
                    self.session.conversation_id,
                    source_updated_at,
                    3_600,
                    64,
                )
                .await
                .expect("claim stage-1 job");
            let ownership_token = match claim {
                codex_state::Stage1JobClaimOutcome::Claimed { ownership_token } => ownership_token,
                other => panic!("unexpected stage-1 claim outcome: {other:?}"),
            };
            assert!(
                self.state_db
                    .mark_stage1_job_succeeded(
                        thread_id,
                        &ownership_token,
                        source_updated_at,
                        "raw memory",
                        "rollout summary",
                    )
                    .await
                    .expect("mark stage-1 success"),
                "stage-1 success should enqueue global consolidation"
            );
        }

        async fn shutdown_threads(&self) {
            self.manager
                .remove_and_close_all_threads()
                .await
                .expect("shutdown spawned threads");
        }

        fn user_input_ops_count(&self) -> usize {
            self.manager
                .captured_ops()
                .into_iter()
                .filter(|(_, op)| matches!(op, Op::UserInput { .. }))
                .count()
        }
    }

    #[tokio::test]
    async fn dispatch_reclaims_stale_global_lock_and_starts_consolidation() {
        let harness = DispatchHarness::new().await;
        harness.seed_stage1_output(100).await;

        let stale_claim = harness
            .state_db
            .try_claim_global_phase2_job(ThreadId::new(), 0)
            .await
            .expect("claim stale global lock");
        assert!(
            matches!(stale_claim, Phase2JobClaimOutcome::Claimed { .. }),
            "stale lock precondition should be claimed"
        );

        let scheduled =
            run_global_memory_consolidation(&harness.session, Arc::clone(&harness.config)).await;
        assert!(
            scheduled,
            "dispatch should reclaim stale lock and spawn one agent"
        );

        let running_claim = harness
            .state_db
            .try_claim_global_phase2_job(ThreadId::new(), 3_600)
            .await
            .expect("claim while running");
        assert_eq!(running_claim, Phase2JobClaimOutcome::SkippedRunning);

        let user_input_ops = harness.user_input_ops_count();
        assert_eq!(user_input_ops, 1);

        harness.shutdown_threads().await;
    }

    #[tokio::test]
    async fn dispatch_schedules_only_one_agent_while_lock_is_running() {
        let harness = DispatchHarness::new().await;
        harness.seed_stage1_output(200).await;

        let first_run =
            run_global_memory_consolidation(&harness.session, Arc::clone(&harness.config)).await;
        let second_run =
            run_global_memory_consolidation(&harness.session, Arc::clone(&harness.config)).await;

        assert!(first_run, "first dispatch should schedule consolidation");
        assert!(
            !second_run,
            "second dispatch should skip while the global lock is running"
        );

        let user_input_ops = harness.user_input_ops_count();
        assert_eq!(user_input_ops, 1);

        harness.shutdown_threads().await;
    }

    #[tokio::test]
    async fn dispatch_with_dirty_job_and_no_stage1_outputs_skips_spawn_and_clears_dirty_flag() {
        let harness = DispatchHarness::new().await;
        harness
            .state_db
            .enqueue_global_consolidation(999)
            .await
            .expect("enqueue global consolidation");

        let scheduled =
            run_global_memory_consolidation(&harness.session, Arc::clone(&harness.config)).await;
        assert!(
            !scheduled,
            "dispatch should not spawn when no stage-1 outputs are available"
        );
        assert_eq!(harness.user_input_ops_count(), 0);

        let claim = harness
            .state_db
            .try_claim_global_phase2_job(ThreadId::new(), 3_600)
            .await
            .expect("claim global job after empty dispatch");
        assert_eq!(
            claim,
            Phase2JobClaimOutcome::SkippedNotDirty,
            "empty dispatch should finalize global job as up-to-date"
        );

        harness.shutdown_threads().await;
    }

    #[tokio::test]
    async fn dispatch_marks_job_for_retry_when_spawn_agent_fails() {
        let codex_home = tempfile::tempdir().expect("create temp codex home");
        let mut config = test_config();
        config.codex_home = codex_home.path().to_path_buf();
        config.cwd = config.codex_home.clone();
        let config = Arc::new(config);

        let state_db = codex_state::StateRuntime::init(
            config.codex_home.clone(),
            config.model_provider_id.clone(),
            None,
        )
        .await
        .expect("initialize state db");

        let (mut session, _turn_context) = make_session_and_context().await;
        session.services.state_db = Some(Arc::clone(&state_db));
        session.services.agent_control = AgentControl::default();
        let session = Arc::new(session);

        let thread_id = ThreadId::new();
        let mut metadata_builder = ThreadMetadataBuilder::new(
            thread_id,
            config.codex_home.join(format!("rollout-{thread_id}.jsonl")),
            Utc::now(),
            SessionSource::Cli,
        );
        metadata_builder.cwd = config.cwd.clone();
        metadata_builder.model_provider = Some(config.model_provider_id.clone());
        let metadata = metadata_builder.build(&config.model_provider_id);
        state_db
            .upsert_thread(&metadata)
            .await
            .expect("upsert thread metadata");

        let claim = state_db
            .try_claim_stage1_job(thread_id, session.conversation_id, 100, 3_600, 64)
            .await
            .expect("claim stage-1 job");
        let ownership_token = match claim {
            codex_state::Stage1JobClaimOutcome::Claimed { ownership_token } => ownership_token,
            other => panic!("unexpected stage-1 claim outcome: {other:?}"),
        };
        assert!(
            state_db
                .mark_stage1_job_succeeded(
                    thread_id,
                    &ownership_token,
                    100,
                    "raw memory",
                    "rollout summary",
                )
                .await
                .expect("mark stage-1 success"),
            "stage-1 success should enqueue global consolidation"
        );

        let scheduled = run_global_memory_consolidation(&session, Arc::clone(&config)).await;
        assert!(
            !scheduled,
            "dispatch should return false when consolidation subagent cannot be spawned"
        );

        let retry_claim = state_db
            .try_claim_global_phase2_job(ThreadId::new(), 3_600)
            .await
            .expect("claim global job after spawn failure");
        assert_eq!(
            retry_claim,
            Phase2JobClaimOutcome::SkippedNotDirty,
            "spawn failures should leave the job in retry backoff instead of running"
        );
    }
}
