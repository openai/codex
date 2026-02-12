use crate::codex::Session;
use crate::config::Config;
use crate::config::Constrained;
use crate::memories::memory_root;
use crate::memories::metrics;
use crate::memories::phase_two;
use crate::memories::phase2::spawn_phase2_completion_task;
use crate::memories::prompts::build_consolidation_prompt;
use crate::memories::storage::rebuild_raw_memories_file_from_memories;
use crate::memories::storage::sync_rollout_summaries_from_memories;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::user_input::UserInput;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::sync::Arc;
use tracing::debug;
use tracing::info;
use tracing::warn;

//TODO(jif) clean.

fn completion_watermark(
    claimed_watermark: i64,
    latest_memories: &[codex_state::Stage1Output],
) -> i64 {
    latest_memories
        .iter()
        .map(|memory| memory.source_updated_at.timestamp())
        .max()
        .unwrap_or(claimed_watermark)
        .max(claimed_watermark)
}

pub(in crate::memories) async fn run_global_memory_consolidation(
    session: &Arc<Session>,
    config: Arc<Config>,
) -> bool {
    let otel_manager = &session.services.otel_manager;
    let Some(state_db) = session.services.state_db.as_deref() else {
        warn!("state db unavailable; skipping global memory consolidation");
        otel_manager.counter(
            metrics::MEMORY_PHASE_TWO_JOBS,
            1,
            &[("status", "skipped_state_db_unavailable")],
        );
        return false;
    };

    let claim = match state_db
        .try_claim_global_phase2_job(session.conversation_id, phase_two::JOB_LEASE_SECONDS)
        .await
    {
        Ok(claim) => claim,
        Err(err) => {
            warn!("state db try_claim_global_phase2_job failed during memories startup: {err}");
            otel_manager.counter(
                metrics::MEMORY_PHASE_TWO_JOBS,
                1,
                &[("status", "failed_claim")],
            );
            return false;
        }
    };
    let (ownership_token, claimed_watermark) = match claim {
        codex_state::Phase2JobClaimOutcome::Claimed {
            ownership_token,
            input_watermark,
        } => {
            otel_manager.counter(metrics::MEMORY_PHASE_TWO_JOBS, 1, &[("status", "claimed")]);
            (ownership_token, input_watermark)
        }
        codex_state::Phase2JobClaimOutcome::SkippedNotDirty => {
            debug!("memory phase-2 global lock is up-to-date; skipping consolidation");
            otel_manager.counter(
                metrics::MEMORY_PHASE_TWO_JOBS,
                1,
                &[("status", "skipped_not_dirty")],
            );
            return false;
        }
        codex_state::Phase2JobClaimOutcome::SkippedRunning => {
            debug!("memory phase-2 global consolidation already running; skipping");
            otel_manager.counter(
                metrics::MEMORY_PHASE_TWO_JOBS,
                1,
                &[("status", "skipped_running")],
            );
            return false;
        }
    };

    let root = memory_root(&config.codex_home);
    let consolidation_config = {
        let mut consolidation_config = config.as_ref().clone();
        consolidation_config.cwd = root.clone();
        consolidation_config.approval_policy = Constrained::allow_only(AskForApproval::Never);
        let mut writable_roots = Vec::new();
        match AbsolutePathBuf::from_absolute_path(consolidation_config.codex_home.clone()) {
            Ok(codex_home) => writable_roots.push(codex_home),
            Err(err) => warn!(
                "memory phase-2 consolidation could not add codex_home writable root {}: {err}",
                consolidation_config.codex_home.display()
            ),
        }
        let consolidation_sandbox_policy = SandboxPolicy::WorkspaceWrite {
            writable_roots,
            read_only_access: Default::default(),
            network_access: false,
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
        };
        if let Err(err) = consolidation_config
            .sandbox_policy
            .set(consolidation_sandbox_policy)
        {
            warn!("memory phase-2 consolidation sandbox policy was rejected by constraints: {err}");
            otel_manager.counter(
                metrics::MEMORY_PHASE_TWO_JOBS,
                1,
                &[("status", "failed_sandbox_policy")],
            );
            let _ = state_db
                .mark_global_phase2_job_failed(
                    &ownership_token,
                    "consolidation sandbox policy was rejected by constraints",
                    phase_two::JOB_RETRY_DELAY_SECONDS,
                )
                .await;
            return false;
        }
        consolidation_config
    };

    let latest_memories = match state_db
        .list_stage1_outputs_for_global(phase_two::MAX_RAW_MEMORIES_FOR_GLOBAL)
        .await
    {
        Ok(memories) => memories,
        Err(err) => {
            warn!("state db list_stage1_outputs_for_global failed during consolidation: {err}");
            otel_manager.counter(
                metrics::MEMORY_PHASE_TWO_JOBS,
                1,
                &[("status", "failed_load_stage1_outputs")],
            );
            let _ = state_db
                .mark_global_phase2_job_failed(
                    &ownership_token,
                    "failed to read stage-1 outputs before global consolidation",
                    phase_two::JOB_RETRY_DELAY_SECONDS,
                )
                .await;
            return false;
        }
    };
    if !latest_memories.is_empty() {
        otel_manager.counter(
            metrics::MEMORY_PHASE_TWO_INPUT,
            latest_memories.len() as i64,
            &[],
        );
    }
    let completion_watermark = completion_watermark(claimed_watermark, &latest_memories);
    if let Err(err) = sync_rollout_summaries_from_memories(&root, &latest_memories).await {
        warn!("failed syncing local memory artifacts for global consolidation: {err}");
        otel_manager.counter(
            metrics::MEMORY_PHASE_TWO_JOBS,
            1,
            &[("status", "failed_sync_artifacts")],
        );
        let _ = state_db
            .mark_global_phase2_job_failed(
                &ownership_token,
                "failed syncing local memory artifacts",
                phase_two::JOB_RETRY_DELAY_SECONDS,
            )
            .await;
        return false;
    }

    if let Err(err) = rebuild_raw_memories_file_from_memories(&root, &latest_memories).await {
        warn!("failed rebuilding raw memories aggregate for global consolidation: {err}");
        otel_manager.counter(
            metrics::MEMORY_PHASE_TWO_JOBS,
            1,
            &[("status", "failed_rebuild_raw_memories")],
        );
        let _ = state_db
            .mark_global_phase2_job_failed(
                &ownership_token,
                "failed rebuilding raw memories aggregate",
                phase_two::JOB_RETRY_DELAY_SECONDS,
            )
            .await;
        return false;
    }
    if latest_memories.is_empty() {
        debug!("memory phase-2 has no stage-1 outputs; finalized local memory artifacts");
        let _ = state_db
            .mark_global_phase2_job_succeeded(&ownership_token, completion_watermark)
            .await;
        otel_manager.counter(
            metrics::MEMORY_PHASE_TWO_JOBS,
            1,
            &[("status", "succeeded_no_input")],
        );
        return false;
    }

    let prompt = build_consolidation_prompt(&root);
    let input = vec![UserInput::Text {
        text: prompt,
        text_elements: vec![],
    }];
    let source = SessionSource::SubAgent(SubAgentSource::Other(
        phase_two::MEMORY_CONSOLIDATION_SUBAGENT_LABEL.to_string(),
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
            otel_manager.counter(
                metrics::MEMORY_PHASE_TWO_JOBS,
                1,
                &[("status", "agent_spawned")],
            );
            spawn_phase2_completion_task(
                session.as_ref(),
                ownership_token,
                completion_watermark,
                consolidation_agent_id,
            );
            true
        }
        Err(err) => {
            warn!("failed to spawn global memory consolidation agent: {err}");
            otel_manager.counter(
                metrics::MEMORY_PHASE_TWO_JOBS,
                1,
                &[("status", "failed_spawn_agent")],
            );
            let _ = state_db
                .mark_global_phase2_job_failed(
                    &ownership_token,
                    "failed to spawn consolidation agent",
                    phase_two::JOB_RETRY_DELAY_SECONDS,
                )
                .await;
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::completion_watermark;
    use super::run_global_memory_consolidation;
    use crate::CodexAuth;
    use crate::ThreadManager;
    use crate::agent::control::AgentControl;
    use crate::codex::Session;
    use crate::codex::make_session_and_context;
    use crate::config::Config;
    use crate::config::test_config;
    use crate::memories::memory_root;
    use crate::memories::raw_memories_file;
    use crate::memories::rollout_summaries_dir;
    use chrono::Utc;
    use codex_protocol::ThreadId;
    use codex_protocol::protocol::AskForApproval;
    use codex_protocol::protocol::Op;
    use codex_protocol::protocol::SandboxPolicy;
    use codex_protocol::protocol::SessionSource;
    use codex_state::Phase2JobClaimOutcome;
    use codex_state::Stage1Output;
    use codex_state::ThreadMetadataBuilder;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;
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

            let manager = ThreadManager::with_models_provider_and_home_for_tests(
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

    #[test]
    fn completion_watermark_never_regresses_below_claimed_input_watermark() {
        let stage1_output = Stage1Output {
            thread_id: ThreadId::new(),
            source_updated_at: chrono::DateTime::<Utc>::from_timestamp(123, 0)
                .expect("valid source_updated_at timestamp"),
            raw_memory: "raw memory".to_string(),
            rollout_summary: "rollout summary".to_string(),
            cwd: PathBuf::from("/tmp/workspace"),
            generated_at: chrono::DateTime::<Utc>::from_timestamp(124, 0)
                .expect("valid generated_at timestamp"),
        };

        let completion = completion_watermark(1_000, &[stage1_output]);
        assert_eq!(completion, 1_000);
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
        let thread_ids = harness.manager.list_thread_ids().await;
        assert_eq!(thread_ids.len(), 1);
        let subagent = harness
            .manager
            .get_thread(thread_ids[0])
            .await
            .expect("get consolidation thread");
        let config_snapshot = subagent.config_snapshot().await;
        assert_eq!(config_snapshot.approval_policy, AskForApproval::Never);
        assert_eq!(config_snapshot.cwd, memory_root(&harness.config.codex_home));
        match config_snapshot.sandbox_policy {
            SandboxPolicy::WorkspaceWrite { writable_roots, .. } => {
                assert!(
                    writable_roots
                        .iter()
                        .any(|root| root.as_path() == harness.config.codex_home.as_path()),
                    "consolidation subagent should have codex_home as writable root"
                );
            }
            other => panic!("unexpected sandbox policy: {other:?}"),
        }

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
    async fn dispatch_with_empty_stage1_outputs_rebuilds_local_artifacts() {
        let harness = DispatchHarness::new().await;
        let root = memory_root(&harness.config.codex_home);
        let summaries_dir = rollout_summaries_dir(&root);
        tokio::fs::create_dir_all(&summaries_dir)
            .await
            .expect("create rollout summaries dir");

        let stale_summary_path = summaries_dir.join(format!("{}.md", ThreadId::new()));
        tokio::fs::write(&stale_summary_path, "stale summary\n")
            .await
            .expect("write stale rollout summary");
        let raw_memories_path = raw_memories_file(&root);
        tokio::fs::write(&raw_memories_path, "stale raw memories\n")
            .await
            .expect("write stale raw memories");
        let memory_index_path = root.join("MEMORY.md");
        tokio::fs::write(&memory_index_path, "stale memory index\n")
            .await
            .expect("write stale memory index");
        let memory_summary_path = root.join("memory_summary.md");
        tokio::fs::write(&memory_summary_path, "stale memory summary\n")
            .await
            .expect("write stale memory summary");
        let stale_skill_file = root.join("skills/demo/SKILL.md");
        tokio::fs::create_dir_all(
            stale_skill_file
                .parent()
                .expect("skills subdirectory parent should exist"),
        )
        .await
        .expect("create stale skills dir");
        tokio::fs::write(&stale_skill_file, "stale skill\n")
            .await
            .expect("write stale skill");

        harness
            .state_db
            .enqueue_global_consolidation(999)
            .await
            .expect("enqueue global consolidation");

        let scheduled =
            run_global_memory_consolidation(&harness.session, Arc::clone(&harness.config)).await;
        assert!(
            !scheduled,
            "dispatch should skip subagent spawn when no stage-1 outputs are available"
        );

        assert!(
            !tokio::fs::try_exists(&stale_summary_path)
                .await
                .expect("check stale summary existence"),
            "empty consolidation should prune stale rollout summary files"
        );
        let raw_memories = tokio::fs::read_to_string(&raw_memories_path)
            .await
            .expect("read rebuilt raw memories");
        assert_eq!(raw_memories, "# Raw Memories\n\nNo raw memories yet.\n");
        assert!(
            !tokio::fs::try_exists(&memory_index_path)
                .await
                .expect("check memory index existence"),
            "empty consolidation should remove stale MEMORY.md"
        );
        assert!(
            !tokio::fs::try_exists(&memory_summary_path)
                .await
                .expect("check memory summary existence"),
            "empty consolidation should remove stale memory_summary.md"
        );
        assert!(
            !tokio::fs::try_exists(&stale_skill_file)
                .await
                .expect("check stale skill existence"),
            "empty consolidation should remove stale skills artifacts"
        );
        assert!(
            !tokio::fs::try_exists(root.join("skills"))
                .await
                .expect("check skills dir existence"),
            "empty consolidation should remove stale skills directory"
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
