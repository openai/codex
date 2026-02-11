use crate::agent::AgentStatus;
use crate::agent::status::is_final as is_final_agent_status;
use crate::codex::Session;
use crate::config::Config;
use crate::memories::memory_root;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::ThreadId;
use codex_protocol::user_input::UserInput;
use codex_state::Phase2JobClaimOutcome;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tracing::debug;
use tracing::info;
use tracing::warn;

use super::super::PHASE_TWO_JOB_HEARTBEAT_SECONDS;
use super::super::PHASE_TWO_JOB_LEASE_SECONDS;
use super::super::PHASE_TWO_JOB_RETRY_DELAY_SECONDS;
use super::super::MAX_RAW_MEMORIES_FOR_GLOBAL;
use super::super::MEMORY_CONSOLIDATION_SUBAGENT_LABEL;
use super::super::prompts::build_consolidation_prompt;
use super::super::storage::rebuild_raw_memories_file_from_memories;
use super::super::storage::sync_rollout_summaries_from_memories;

#[derive(Clone)]
pub(super) struct ClaimedPhase2Job {
    state_db: Arc<codex_state::StateRuntime>,
    ownership_token: String,
    input_watermark: i64,
}

pub(super) enum GlobalPhase2Claim {
    Claimed(ClaimedPhase2Job),
    SkippedNotDirty,
    SkippedRunning,
}

pub(super) async fn try_claim_global_phase2_job(
    state_db: Arc<codex_state::StateRuntime>,
    worker_id: ThreadId,
) -> anyhow::Result<GlobalPhase2Claim> {
    let claim = state_db
        .try_claim_global_phase2_job(worker_id, PHASE_TWO_JOB_LEASE_SECONDS)
        .await?;
    Ok(match claim {
        Phase2JobClaimOutcome::Claimed {
            ownership_token,
            input_watermark,
        } => GlobalPhase2Claim::Claimed(ClaimedPhase2Job::new(
            state_db,
            ownership_token,
            input_watermark,
        )),
        Phase2JobClaimOutcome::SkippedNotDirty => GlobalPhase2Claim::SkippedNotDirty,
        Phase2JobClaimOutcome::SkippedRunning => GlobalPhase2Claim::SkippedRunning,
    })
}

impl ClaimedPhase2Job {
    fn new(
        state_db: Arc<codex_state::StateRuntime>,
        ownership_token: String,
        input_watermark: i64,
    ) -> Self {
        Self {
            state_db,
            ownership_token,
            input_watermark,
        }
    }

    pub(super) fn input_watermark(&self) -> i64 {
        self.input_watermark
    }

    pub(super) async fn heartbeat(&self) -> anyhow::Result<bool> {
        self.state_db
            .heartbeat_global_phase2_job(&self.ownership_token, PHASE_TWO_JOB_LEASE_SECONDS)
            .await
    }

    pub(super) async fn mark_succeeded(&self, completion_watermark: i64) {
        match self
            .state_db
            .mark_global_phase2_job_succeeded(&self.ownership_token, completion_watermark)
            .await
        {
            Ok(true) => {}
            Ok(false) => {
                debug!("memory phase-2 success finalization skipped after global ownership changed");
            }
            Err(err) => {
                warn!("state db mark_global_phase2_job_succeeded failed during memories startup: {err}");
            }
        }
    }

    pub(super) async fn mark_failed_with_recovery(&self, failure_reason: &str) {
        mark_phase2_failed_with_recovery(
            self.state_db.as_ref(),
            self.ownership_token.as_str(),
            failure_reason,
        )
        .await;
    }
}

fn completion_watermark(claimed_watermark: i64, latest_memories: &[codex_state::Stage1Output]) -> i64 {
    latest_memories
        .iter()
        .map(|memory| memory.source_updated_at.timestamp())
        .max()
        .unwrap_or(claimed_watermark)
        .max(claimed_watermark)
}

pub(super) async fn run_global_memory_consolidation(
    session: &Arc<Session>,
    config: Arc<Config>,
) -> bool {
    // 1) Get a lock.
    let Some(state_db) = session.services.state_db.clone() else {
        warn!("state db unavailable; skipping global memory consolidation");
        return false;
    };
    let claim = match try_claim_global_phase2_job(Arc::clone(&state_db), session.conversation_id).await
    {
        Ok(claim) => claim,
        Err(err) => {
            warn!("state db try_claim_global_phase2_job failed during memories startup: {err}");
            return false;
        }
    };
    let phase2_job = match claim {
        GlobalPhase2Claim::Claimed(phase2_job) => phase2_job,
        GlobalPhase2Claim::SkippedNotDirty => {
            debug!("memory phase-2 global lock is up-to-date; skipping consolidation");
            return false;
        }
        GlobalPhase2Claim::SkippedRunning => {
            debug!("memory phase-2 global consolidation already running; skipping");
            return false;
        }
    };

    // 2) Get the rollouts.
    let latest_memories = match state_db
        .list_stage1_outputs_for_global(MAX_RAW_MEMORIES_FOR_GLOBAL)
        .await
    {
        Ok(memories) => memories,
        Err(err) => {
            warn!("state db list_stage1_outputs_for_global failed during consolidation: {err}");
            phase2_job
                .mark_failed_with_recovery("failed to read stage-1 outputs before global consolidation")
                .await;
            return false;
        }
    };

    // 3) Persist the files.
    let root = memory_root(&config.codex_home);
    let completion_watermark = completion_watermark(phase2_job.input_watermark(), &latest_memories);
    if let Err(err) = sync_rollout_summaries_from_memories(&root, &latest_memories).await {
        warn!("failed syncing local memory artifacts for global consolidation: {err}");
        phase2_job
            .mark_failed_with_recovery("failed syncing local memory artifacts")
            .await;
        return false;
    }
    if let Err(err) = rebuild_raw_memories_file_from_memories(&root, &latest_memories).await {
        warn!("failed rebuilding raw memories aggregate for global consolidation: {err}");
        phase2_job
            .mark_failed_with_recovery("failed rebuilding raw memories aggregate")
            .await;
        return false;
    }
    if latest_memories.is_empty() {
        phase2_job.mark_succeeded(completion_watermark).await;
        return false;
    }

    // 4) Run the worker.
    let prompt = build_consolidation_prompt(&root);
    let input = vec![UserInput::Text {
        text: prompt,
        text_elements: vec![],
    }];
    let mut consolidation_config = config.as_ref().clone();
    consolidation_config.cwd = root;
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
                phase2_job,
                completion_watermark,
                consolidation_agent_id,
            );
            true
        }
        Err(err) => {
            warn!("failed to spawn global memory consolidation agent: {err}");
            phase2_job
                .mark_failed_with_recovery("failed to spawn consolidation agent")
                .await;
            false
        }
    }
}

pub(super) fn spawn_phase2_completion_task(
    session: &Session,
    phase2_job: ClaimedPhase2Job,
    completion_watermark: i64,
    consolidation_agent_id: ThreadId,
) {
    let agent_control = session.services.agent_control.clone();

    tokio::spawn(async move {
        let status_rx = match agent_control.subscribe_status(consolidation_agent_id).await {
            Ok(status_rx) => status_rx,
            Err(err) => {
                warn!(
                    "failed to subscribe to global memory consolidation agent {consolidation_agent_id}: {err}"
                );
                phase2_job
                    .mark_failed_with_recovery("failed to subscribe to consolidation agent status")
                    .await;
                return;
            }
        };

        let final_status = run_phase2_completion_task(
            phase2_job,
            completion_watermark,
            consolidation_agent_id,
            status_rx,
        )
        .await;
        if matches!(final_status, AgentStatus::Shutdown | AgentStatus::NotFound) {
            return;
        }

        tokio::spawn(async move {
            if let Err(err) = agent_control.shutdown_agent(consolidation_agent_id).await {
                warn!(
                    "failed to auto-close global memory consolidation agent {consolidation_agent_id}: {err}"
                );
            }
        });
    });
}

async fn run_phase2_completion_task(
    phase2_job: ClaimedPhase2Job,
    completion_watermark: i64,
    consolidation_agent_id: ThreadId,
    mut status_rx: watch::Receiver<AgentStatus>,
) -> AgentStatus {
    let final_status = {
        let mut heartbeat_interval =
            tokio::time::interval(Duration::from_secs(PHASE_TWO_JOB_HEARTBEAT_SECONDS));
        heartbeat_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            let status = status_rx.borrow().clone();
            if is_final_agent_status(&status) {
                break status;
            }

            tokio::select! {
                changed = status_rx.changed() => {
                    if changed.is_err() {
                        warn!(
                            "lost status updates for global memory consolidation agent {consolidation_agent_id}"
                        );
                        break status;
                    }
                }
                _ = heartbeat_interval.tick() => {
                    match phase2_job.heartbeat().await {
                        Ok(true) => {}
                        Ok(false) => {
                            warn!(
                                "memory phase-2 heartbeat lost global ownership; finalizing as failure"
                            );
                            break AgentStatus::Errored(
                                "lost global phase-2 ownership during heartbeat".to_string(),
                            );
                        }
                        Err(err) => {
                            warn!(
                                "state db heartbeat_global_phase2_job failed during memories startup: {err}"
                            );
                            break AgentStatus::Errored(format!(
                                "phase-2 heartbeat update failed: {err}"
                            ));
                        }
                    }
                }
            }
        }
    };

    let phase2_success = is_phase2_success(&final_status);
    info!(
        "memory phase-2 global consolidation complete: agent_id={consolidation_agent_id} success={phase2_success} final_status={final_status:?}"
    );

    if phase2_success {
        phase2_job.mark_succeeded(completion_watermark).await;
        return final_status;
    }

    let failure_reason = phase2_failure_reason(&final_status);
    phase2_job.mark_failed_with_recovery(&failure_reason).await;
    warn!(
        "memory phase-2 global consolidation agent finished with non-success status: agent_id={consolidation_agent_id} final_status={final_status:?}"
    );
    final_status
}

async fn mark_phase2_failed_with_recovery(
    state_db: &codex_state::StateRuntime,
    ownership_token: &str,
    failure_reason: &str,
) {
    match state_db
        .mark_global_phase2_job_failed(
            ownership_token,
            failure_reason,
            PHASE_TWO_JOB_RETRY_DELAY_SECONDS,
        )
        .await
    {
        Ok(true) => {}
        Ok(false) => match state_db
            .mark_global_phase2_job_failed_if_unowned(
                ownership_token,
                failure_reason,
                PHASE_TWO_JOB_RETRY_DELAY_SECONDS,
            )
            .await
        {
            Ok(true) => {
                debug!(
                    "memory phase-2 failure finalization applied fallback update for unowned running job"
                );
            }
            Ok(false) => {
                debug!(
                    "memory phase-2 failure finalization skipped after global ownership changed"
                );
            }
            Err(err) => {
                warn!(
                    "state db mark_global_phase2_job_failed_if_unowned failed during memories startup: {err}"
                );
            }
        },
        Err(err) => {
            warn!("state db mark_global_phase2_job_failed failed during memories startup: {err}");
        }
    }
}

fn is_phase2_success(final_status: &AgentStatus) -> bool {
    matches!(final_status, AgentStatus::Completed(_))
}

fn phase2_failure_reason(final_status: &AgentStatus) -> String {
    format!("consolidation agent finished with status {final_status:?}")
}

#[cfg(test)]
mod tests {
    use super::ClaimedPhase2Job;
    use super::is_phase2_success;
    use super::phase2_failure_reason;
    use super::run_phase2_completion_task;
    use crate::agent::AgentStatus;
    use codex_protocol::ThreadId;
    use codex_state::Phase2JobClaimOutcome;
    use pretty_assertions::assert_eq;
    use std::sync::Arc;

    #[test]
    fn phase2_success_only_for_completed_status() {
        assert!(is_phase2_success(&AgentStatus::Completed(None)));
        assert!(!is_phase2_success(&AgentStatus::Running));
        assert!(!is_phase2_success(&AgentStatus::Errored(
            "oops".to_string()
        )));
    }

    #[test]
    fn phase2_failure_reason_includes_status() {
        let status = AgentStatus::Errored("boom".to_string());
        let reason = phase2_failure_reason(&status);
        assert!(reason.contains("consolidation agent finished with status"));
        assert!(reason.contains("boom"));
    }

    #[tokio::test]
    async fn phase2_completion_marks_succeeded_for_completed_status() {
        let codex_home = tempfile::tempdir().expect("create temp codex home");
        let state_db = Arc::new(
            codex_state::StateRuntime::init(
                codex_home.path().to_path_buf(),
                "test-provider".to_string(),
                None,
            )
            .await
            .expect("initialize state runtime"),
        );
        let owner = ThreadId::new();
        state_db
            .enqueue_global_consolidation(123)
            .await
            .expect("enqueue global consolidation");
        let claim = state_db
            .try_claim_global_phase2_job(owner, 3_600)
            .await
            .expect("claim global phase-2 job");
        let phase2_job = match claim {
            Phase2JobClaimOutcome::Claimed {
                ownership_token,
                input_watermark,
            } => ClaimedPhase2Job::new(Arc::clone(&state_db), ownership_token, input_watermark),
            other => panic!("unexpected phase-2 claim outcome: {other:?}"),
        };

        let (_status_tx, status_rx) = tokio::sync::watch::channel(AgentStatus::Completed(None));
        run_phase2_completion_task(
            phase2_job,
            123,
            ThreadId::new(),
            status_rx,
        )
        .await;

        let up_to_date_claim = state_db
            .try_claim_global_phase2_job(ThreadId::new(), 3_600)
            .await
            .expect("claim up-to-date global job");
        assert_eq!(up_to_date_claim, Phase2JobClaimOutcome::SkippedNotDirty);

        state_db
            .enqueue_global_consolidation(124)
            .await
            .expect("enqueue advanced consolidation watermark");
        let rerun_claim = state_db
            .try_claim_global_phase2_job(ThreadId::new(), 3_600)
            .await
            .expect("claim rerun global job");
        assert!(
            matches!(rerun_claim, Phase2JobClaimOutcome::Claimed { .. }),
            "advanced watermark should be claimable after success finalization"
        );
    }

    #[tokio::test]
    async fn phase2_completion_marks_failed_when_status_updates_are_lost() {
        let codex_home = tempfile::tempdir().expect("create temp codex home");
        let state_db = Arc::new(
            codex_state::StateRuntime::init(
                codex_home.path().to_path_buf(),
                "test-provider".to_string(),
                None,
            )
            .await
            .expect("initialize state runtime"),
        );
        state_db
            .enqueue_global_consolidation(456)
            .await
            .expect("enqueue global consolidation");
        let claim = state_db
            .try_claim_global_phase2_job(ThreadId::new(), 3_600)
            .await
            .expect("claim global phase-2 job");
        let phase2_job = match claim {
            Phase2JobClaimOutcome::Claimed {
                ownership_token,
                input_watermark,
            } => ClaimedPhase2Job::new(Arc::clone(&state_db), ownership_token, input_watermark),
            other => panic!("unexpected phase-2 claim outcome: {other:?}"),
        };

        let (status_tx, status_rx) = tokio::sync::watch::channel(AgentStatus::Running);
        drop(status_tx);
        run_phase2_completion_task(
            phase2_job,
            456,
            ThreadId::new(),
            status_rx,
        )
        .await;

        let claim = state_db
            .try_claim_global_phase2_job(ThreadId::new(), 3_600)
            .await
            .expect("claim after failure finalization");
        assert_eq!(
            claim,
            Phase2JobClaimOutcome::SkippedNotDirty,
            "failure finalization should leave global job in retry-backoff, not running ownership"
        );
    }

    #[tokio::test]
    async fn phase2_completion_heartbeat_loss_does_not_steal_active_other_owner() {
        let codex_home = tempfile::tempdir().expect("create temp codex home");
        let state_db = Arc::new(
            codex_state::StateRuntime::init(
                codex_home.path().to_path_buf(),
                "test-provider".to_string(),
                None,
            )
            .await
            .expect("initialize state runtime"),
        );
        state_db
            .enqueue_global_consolidation(789)
            .await
            .expect("enqueue global consolidation");
        let claim = state_db
            .try_claim_global_phase2_job(ThreadId::new(), 3_600)
            .await
            .expect("claim global phase-2 job");
        let claimed_token = match claim {
            Phase2JobClaimOutcome::Claimed {
                ownership_token, ..
            } => ownership_token,
            other => panic!("unexpected phase-2 claim outcome: {other:?}"),
        };

        let (_status_tx, status_rx) = tokio::sync::watch::channel(AgentStatus::Running);
        let phase2_job = ClaimedPhase2Job::new(Arc::clone(&state_db), "non-owner-token".to_string(), 789);
        run_phase2_completion_task(phase2_job, 789, ThreadId::new(), status_rx).await;

        let claim = state_db
            .try_claim_global_phase2_job(ThreadId::new(), 3_600)
            .await
            .expect("claim after heartbeat ownership loss");
        assert_eq!(
            claim,
            Phase2JobClaimOutcome::SkippedRunning,
            "heartbeat ownership-loss handling should not steal a live owner lease"
        );
        assert_eq!(
            state_db
                .mark_global_phase2_job_succeeded(claimed_token.as_str(), 789)
                .await
                .expect("mark original owner success"),
            true,
            "the original owner should still be able to finalize"
        );
    }
}

#[cfg(test)]
mod dispatch_tests {
    use super::completion_watermark;
    use super::memory_root;
    use super::run_global_memory_consolidation;
    use crate::CodexAuth;
    use crate::ThreadManager;
    use crate::agent::control::AgentControl;
    use crate::codex::Session;
    use crate::codex::make_session_and_context;
    use crate::config::Config;
    use crate::config::test_config;
    use crate::memories::raw_memories_file;
    use crate::memories::rollout_summaries_dir;
    use chrono::Utc;
    use codex_protocol::ThreadId;
    use codex_protocol::protocol::Op;
    use codex_protocol::protocol::SessionSource;
    use codex_state::Phase2JobClaimOutcome;
    use codex_state::Stage1Output;
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
                self.config.codex_home.join(format!("rollout-{thread_id}.jsonl")),
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
