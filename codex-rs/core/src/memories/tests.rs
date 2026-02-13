use super::storage::rebuild_raw_memories_file_from_memories;
use super::storage::sync_rollout_summaries_from_memories;
use crate::memories::ensure_layout;
use crate::memories::memory_root;
use crate::memories::raw_memories_file;
use crate::memories::rollout_summaries_dir;
use chrono::TimeZone;
use chrono::Utc;
use codex_protocol::ThreadId;
use codex_state::Stage1Output;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn memory_root_uses_shared_global_path() {
    let dir = tempdir().expect("tempdir");
    let codex_home = dir.path().join("codex");
    assert_eq!(memory_root(&codex_home), codex_home.join("memories"));
}

#[test]
fn stage_one_output_schema_requires_all_declared_properties() {
    let schema = crate::memories::phase1::output_schema();
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .expect("properties object");
    let required = schema
        .get("required")
        .and_then(Value::as_array)
        .expect("required array");

    let mut property_keys = properties.keys().map(String::as_str).collect::<Vec<_>>();
    property_keys.sort_unstable();

    let mut required_keys = required
        .iter()
        .map(|key| key.as_str().expect("required key string"))
        .collect::<Vec<_>>();
    required_keys.sort_unstable();

    assert_eq!(required_keys, property_keys);
}

#[tokio::test]
async fn sync_rollout_summaries_and_raw_memories_file_keeps_latest_memories_only() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path().join("memory");
    ensure_layout(&root).await.expect("ensure layout");

    let keep_id = ThreadId::default().to_string();
    let drop_id = ThreadId::default().to_string();
    let keep_path = rollout_summaries_dir(&root).join(format!("{keep_id}.md"));
    let drop_path = rollout_summaries_dir(&root).join(format!("{drop_id}.md"));
    tokio::fs::write(&keep_path, "keep")
        .await
        .expect("write keep");
    tokio::fs::write(&drop_path, "drop")
        .await
        .expect("write drop");

    let memories = vec![Stage1Output {
        thread_id: ThreadId::try_from(keep_id.clone()).expect("thread id"),
        source_updated_at: Utc.timestamp_opt(100, 0).single().expect("timestamp"),
        raw_memory: "raw memory".to_string(),
        rollout_summary: "short summary".to_string(),
        cwd: PathBuf::from("/tmp/workspace"),
        generated_at: Utc.timestamp_opt(101, 0).single().expect("timestamp"),
    }];

    sync_rollout_summaries_from_memories(&root, &memories)
        .await
        .expect("sync rollout summaries");
    rebuild_raw_memories_file_from_memories(&root, &memories)
        .await
        .expect("rebuild raw memories");

    assert!(keep_path.is_file());
    assert!(!drop_path.exists());

    let raw_memories = tokio::fs::read_to_string(raw_memories_file(&root))
        .await
        .expect("read raw memories");
    assert!(raw_memories.contains("raw memory"));
    assert!(raw_memories.contains(&keep_id));
    assert!(raw_memories.contains("cwd: /tmp/workspace"));
}

mod phase2 {
    use crate::CodexAuth;
    use crate::ThreadManager;
    use crate::agent::AgentControl;
    use crate::codex::Session;
    use crate::codex::make_session_and_context;
    use crate::config::Config;
    use crate::config::test_config;
    use crate::memories::memory_root;
    use crate::memories::phase2;
    use crate::memories::raw_memories_file;
    use crate::memories::rollout_summaries_dir;
    use chrono::Utc;
    use codex_config::Constrained;
    use codex_protocol::ThreadId;
    use codex_protocol::protocol::AskForApproval;
    use codex_protocol::protocol::Op;
    use codex_protocol::protocol::SandboxPolicy;
    use codex_protocol::protocol::SessionSource;
    use codex_state::Phase2JobClaimOutcome;
    use codex_state::Stage1Output;
    use codex_state::ThreadMetadataBuilder;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn stage1_output_with_source_updated_at(source_updated_at: i64) -> Stage1Output {
        Stage1Output {
            thread_id: ThreadId::new(),
            source_updated_at: chrono::DateTime::<Utc>::from_timestamp(source_updated_at, 0)
                .expect("valid source_updated_at timestamp"),
            raw_memory: "raw memory".to_string(),
            rollout_summary: "rollout summary".to_string(),
            cwd: PathBuf::from("/tmp/workspace"),
            generated_at: chrono::DateTime::<Utc>::from_timestamp(source_updated_at + 1, 0)
                .expect("valid generated_at timestamp"),
        }
    }

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
        let stage1_output = stage1_output_with_source_updated_at(123);

        let completion = phase2::get_watermark(1_000, &[stage1_output]);
        pretty_assertions::assert_eq!(completion, 1_000);
    }

    #[test]
    fn completion_watermark_uses_claimed_watermark_when_there_are_no_memories() {
        let completion = phase2::get_watermark(777, &[]);
        pretty_assertions::assert_eq!(completion, 777);
    }

    #[test]
    fn completion_watermark_uses_latest_memory_timestamp_when_it_is_newer() {
        let older = stage1_output_with_source_updated_at(123);
        let newer = stage1_output_with_source_updated_at(456);

        let completion = phase2::get_watermark(200, &[older, newer]);
        pretty_assertions::assert_eq!(completion, 456);
    }

    #[tokio::test]
    async fn dispatch_skips_when_global_job_is_not_dirty() {
        let harness = DispatchHarness::new().await;

        phase2::run(&harness.session, Arc::clone(&harness.config)).await;

        pretty_assertions::assert_eq!(harness.user_input_ops_count(), 0);
        let thread_ids = harness.manager.list_thread_ids().await;
        pretty_assertions::assert_eq!(thread_ids.len(), 0);
    }

    #[tokio::test]
    async fn dispatch_skips_when_global_job_is_already_running() {
        let harness = DispatchHarness::new().await;
        harness
            .state_db
            .enqueue_global_consolidation(123)
            .await
            .expect("enqueue global consolidation");
        let claimed = harness
            .state_db
            .try_claim_global_phase2_job(ThreadId::new(), 3_600)
            .await
            .expect("claim running global lock");
        assert!(
            matches!(claimed, Phase2JobClaimOutcome::Claimed { .. }),
            "precondition should claim the running lock"
        );

        phase2::run(&harness.session, Arc::clone(&harness.config)).await;

        let running_claim = harness
            .state_db
            .try_claim_global_phase2_job(ThreadId::new(), 3_600)
            .await
            .expect("claim while lock is still running");
        pretty_assertions::assert_eq!(running_claim, Phase2JobClaimOutcome::SkippedRunning);
        pretty_assertions::assert_eq!(harness.user_input_ops_count(), 0);
        let thread_ids = harness.manager.list_thread_ids().await;
        pretty_assertions::assert_eq!(thread_ids.len(), 0);
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

        phase2::run(&harness.session, Arc::clone(&harness.config)).await;

        let running_claim = harness
            .state_db
            .try_claim_global_phase2_job(ThreadId::new(), 3_600)
            .await
            .expect("claim while running");
        pretty_assertions::assert_eq!(running_claim, Phase2JobClaimOutcome::SkippedRunning);

        let user_input_ops = harness.user_input_ops_count();
        pretty_assertions::assert_eq!(user_input_ops, 1);
        let thread_ids = harness.manager.list_thread_ids().await;
        pretty_assertions::assert_eq!(thread_ids.len(), 1);
        let subagent = harness
            .manager
            .get_thread(thread_ids[0])
            .await
            .expect("get consolidation thread");
        let config_snapshot = subagent.config_snapshot().await;
        pretty_assertions::assert_eq!(config_snapshot.approval_policy, AskForApproval::Never);
        pretty_assertions::assert_eq!(config_snapshot.cwd, memory_root(&harness.config.codex_home));
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

        phase2::run(&harness.session, Arc::clone(&harness.config)).await;

        assert!(
            !tokio::fs::try_exists(&stale_summary_path)
                .await
                .expect("check stale summary existence"),
            "empty consolidation should prune stale rollout summary files"
        );
        let raw_memories = tokio::fs::read_to_string(&raw_memories_path)
            .await
            .expect("read rebuilt raw memories");
        pretty_assertions::assert_eq!(raw_memories, "# Raw Memories\n\nNo raw memories yet.\n");
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
        let next_claim = harness
            .state_db
            .try_claim_global_phase2_job(ThreadId::new(), 3_600)
            .await
            .expect("claim global job after empty consolidation success");
        pretty_assertions::assert_eq!(next_claim, Phase2JobClaimOutcome::SkippedNotDirty);
        pretty_assertions::assert_eq!(harness.user_input_ops_count(), 0);
        let thread_ids = harness.manager.list_thread_ids().await;
        pretty_assertions::assert_eq!(thread_ids.len(), 0);

        harness.shutdown_threads().await;
    }

    #[tokio::test]
    async fn dispatch_marks_job_for_retry_when_sandbox_policy_cannot_be_overridden() {
        let harness = DispatchHarness::new().await;
        harness
            .state_db
            .enqueue_global_consolidation(99)
            .await
            .expect("enqueue global consolidation");
        let mut constrained_config = harness.config.as_ref().clone();
        constrained_config.permissions.sandbox_policy =
            Constrained::allow_only(SandboxPolicy::DangerFullAccess);

        phase2::run(&harness.session, Arc::new(constrained_config)).await;

        let retry_claim = harness
            .state_db
            .try_claim_global_phase2_job(ThreadId::new(), 3_600)
            .await
            .expect("claim global job after sandbox policy failure");
        pretty_assertions::assert_eq!(retry_claim, Phase2JobClaimOutcome::SkippedNotDirty);
        pretty_assertions::assert_eq!(harness.user_input_ops_count(), 0);
        let thread_ids = harness.manager.list_thread_ids().await;
        pretty_assertions::assert_eq!(thread_ids.len(), 0);
    }

    #[tokio::test]
    async fn dispatch_marks_job_for_retry_when_syncing_artifacts_fails() {
        let harness = DispatchHarness::new().await;
        harness.seed_stage1_output(100).await;
        let root = memory_root(&harness.config.codex_home);
        tokio::fs::write(&root, "not a directory")
            .await
            .expect("create file at memory root");

        phase2::run(&harness.session, Arc::clone(&harness.config)).await;

        let retry_claim = harness
            .state_db
            .try_claim_global_phase2_job(ThreadId::new(), 3_600)
            .await
            .expect("claim global job after sync failure");
        pretty_assertions::assert_eq!(retry_claim, Phase2JobClaimOutcome::SkippedNotDirty);
        pretty_assertions::assert_eq!(harness.user_input_ops_count(), 0);
        let thread_ids = harness.manager.list_thread_ids().await;
        pretty_assertions::assert_eq!(thread_ids.len(), 0);
    }

    #[tokio::test]
    async fn dispatch_marks_job_for_retry_when_rebuilding_raw_memories_fails() {
        let harness = DispatchHarness::new().await;
        harness.seed_stage1_output(100).await;
        let root = memory_root(&harness.config.codex_home);
        tokio::fs::create_dir_all(raw_memories_file(&root))
            .await
            .expect("create raw_memories.md as a directory");

        phase2::run(&harness.session, Arc::clone(&harness.config)).await;

        let retry_claim = harness
            .state_db
            .try_claim_global_phase2_job(ThreadId::new(), 3_600)
            .await
            .expect("claim global job after rebuild failure");
        pretty_assertions::assert_eq!(retry_claim, Phase2JobClaimOutcome::SkippedNotDirty);
        pretty_assertions::assert_eq!(harness.user_input_ops_count(), 0);
        let thread_ids = harness.manager.list_thread_ids().await;
        pretty_assertions::assert_eq!(thread_ids.len(), 0);
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

        phase2::run(&session, Arc::clone(&config)).await;

        let retry_claim = state_db
            .try_claim_global_phase2_job(ThreadId::new(), 3_600)
            .await
            .expect("claim global job after spawn failure");
        pretty_assertions::assert_eq!(
            retry_claim,
            Phase2JobClaimOutcome::SkippedNotDirty,
            "spawn failures should leave the job in retry backoff instead of running"
        );
    }
}
