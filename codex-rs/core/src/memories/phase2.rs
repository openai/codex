use crate::agent::AgentStatus;
use crate::agent::status::is_final as is_final_agent_status;
use crate::config::Config;
use crate::memories::extensions::find_old_extension_resources;
use crate::memories::extensions::remove_extension_resources;
use crate::memories::memory_root;
use crate::memories::metrics;
use crate::memories::phase_two;
use crate::memories::prompts::build_consolidation_prompt;
use crate::memories::storage::EmptyArtifactCleanup;
use crate::memories::storage::rebuild_raw_memories_file_from_memories;
use crate::memories::storage::sync_rollout_summaries_from_memories;
use crate::memories::workspace::commit_all;
use crate::memories::workspace::has_changes;
use crate::memories::workspace::prepare_git_repo;
use crate::memories::workspace::remove_workspace_diff;
use crate::memories::workspace::write_workspace_diff;
use crate::session::emit_subagent_session_started;
use crate::session::session::Session;
use codex_config::Constrained;
use codex_features::Feature;
use codex_protocol::ThreadId;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::TokenUsage;
use codex_protocol::user_input::UserInput;
use codex_state::StateRuntime;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tracing::warn;

#[derive(Debug, Clone, Default)]
struct Claim {
    token: String,
    watermark: i64,
}

#[derive(Debug, Clone, Default)]
struct Counters {
    input: i64,
}

/// Runs memory phase 2 (aka consolidation) in strict order. The method represents the linear
/// flow of the consolidation phase.
pub(super) async fn run(session: &Arc<Session>, config: Arc<Config>) {
    let phase_two_e2e_timer = session
        .services
        .session_telemetry
        .start_timer(metrics::MEMORY_PHASE_TWO_E2E_MS, &[])
        .ok();

    let Some(db) = session.services.state_db.as_deref() else {
        // This should not happen.
        return;
    };
    let root = memory_root(&config.codex_home);
    let max_raw_memories = config.memories.max_raw_memories_for_consolidation;
    let max_unused_days = config.memories.max_unused_days;

    // 1. Claim the job.
    let claim = match job::claim(session, db).await {
        Ok(claim) => claim,
        Err(e) => {
            session.services.session_telemetry.counter(
                metrics::MEMORY_PHASE_TWO_JOBS,
                /*inc*/ 1,
                &[("status", e)],
            );
            return;
        }
    };

    if let Err(err) = prepare_git_repo(&root).await {
        tracing::error!("failed preparing memory workspace: {err}");
        job::failed(session, db, &claim, "failed_prepare_workspace").await;
        return;
    }

    // 2. Get the config for the agent
    let Some(agent_config) = agent::get_config(config.clone()) else {
        // If we can't get the config, we can't consolidate.
        tracing::error!("failed to get agent config");
        job::failed(session, db, &claim, "failed_sandbox_policy").await;
        return;
    };

    // 3. Query the memories
    let selection = match db
        .get_phase2_input_selection(max_raw_memories, max_unused_days)
        .await
    {
        Ok(selection) => selection,
        Err(err) => {
            tracing::error!("failed to list stage1 outputs from global: {}", err);
            job::failed(session, db, &claim, "failed_load_stage1_outputs").await;
            return;
        }
    };
    let raw_memories = selection.selected.to_vec();
    let artifact_memories = selection.selected.clone();
    let empty_artifact_cleanup = if selection.removed.is_empty() {
        EmptyArtifactCleanup::RemoveConsolidated
    } else {
        EmptyArtifactCleanup::PreserveConsolidated
    };
    let new_watermark = get_watermark(claim.watermark, &raw_memories);

    // 4. Update the file system by syncing the raw memories with the one extracted from DB at
    //    step 3
    // [`rollout_summaries/`]
    if let Err(err) = sync_rollout_summaries_from_memories(
        &root,
        &artifact_memories,
        artifact_memories.len(),
        empty_artifact_cleanup,
    )
    .await
    {
        tracing::error!("failed syncing local memory artifacts for global consolidation: {err}");
        job::failed(session, db, &claim, "failed_sync_artifacts").await;
        return;
    }
    // [`raw_memories.md`]
    if let Err(err) =
        rebuild_raw_memories_file_from_memories(&root, &artifact_memories, artifact_memories.len())
            .await
    {
        tracing::error!("failed syncing local memory artifacts for global consolidation: {err}");
        job::failed(session, db, &claim, "failed_rebuild_raw_memories").await;
        return;
    }
    let pending_extension_resource_removals = find_old_extension_resources(&root).await;
    remove_extension_resources(&pending_extension_resource_removals).await;

    let workspace_has_changes = match has_changes(&root).await {
        Ok(has_changes) => has_changes,
        Err(err) => {
            tracing::error!("failed checking memory workspace changes: {err}");
            job::failed(session, db, &claim, "failed_workspace_status").await;
            return;
        }
    };
    if !workspace_has_changes {
        if let Err(err) = remove_workspace_diff(&root).await {
            tracing::warn!("failed removing stale memory workspace diff file: {err}");
        }
        // We check only after sync of the file system.
        job::succeed(
            session,
            db,
            &claim,
            new_watermark,
            &raw_memories,
            "succeeded_no_input",
        )
        .await;
        return;
    }
    if let Err(err) = write_workspace_diff(&root).await {
        tracing::error!("failed writing memory workspace diff file: {err}");
        job::failed(session, db, &claim, "failed_workspace_diff_file").await;
        return;
    }

    // 5. Spawn the agent
    let prompt = agent::get_prompt(config);
    let source = SessionSource::SubAgent(SubAgentSource::MemoryConsolidation);
    let agent_control = session.services.agent_control.detached_registry();
    let thread_id = match agent_control
        .spawn_agent(agent_config, prompt.into(), Some(source))
        .await
    {
        Ok(thread_id) => thread_id,
        Err(err) => {
            tracing::error!("failed to spawn global memory consolidation agent: {err}");
            job::failed(session, db, &claim, "failed_spawn_agent").await;
            return;
        }
    };

    if let Some(thread_config) = session
        .services
        .agent_control
        .get_agent_config_snapshot(thread_id)
        .await
    {
        if session.enabled(Feature::GeneralAnalytics) {
            let client_metadata = session.app_server_client_metadata().await;
            emit_subagent_session_started(
                &session.services.analytics_events_client,
                client_metadata,
                thread_id,
                /*parent_thread_id*/ None,
                thread_config,
                SubAgentSource::MemoryConsolidation,
            );
        }
    } else {
        warn!("failed to load memory consolidation thread config for analytics: {thread_id}");
    }

    // 6. Spawn the agent handler.
    agent::handle(
        session,
        claim,
        new_watermark,
        raw_memories.clone(),
        root,
        thread_id,
        agent_control,
        phase_two_e2e_timer,
    );

    // 7. Metrics and logs.
    let counters = Counters {
        input: raw_memories.len() as i64,
    };
    emit_metrics(session, counters);
}

mod job {
    use super::*;

    pub(super) async fn claim(
        session: &Arc<Session>,
        db: &StateRuntime,
    ) -> Result<Claim, &'static str> {
        let session_telemetry = &session.services.session_telemetry;
        let claim = db
            .try_claim_global_phase2_job(session.conversation_id, phase_two::JOB_LEASE_SECONDS)
            .await
            .map_err(|e| {
                tracing::error!("failed to claim job: {}", e);
                "failed_claim"
            })?;
        let (token, watermark) = match claim {
            codex_state::Phase2JobClaimOutcome::Claimed {
                ownership_token,
                input_watermark,
            } => {
                session_telemetry.counter(
                    metrics::MEMORY_PHASE_TWO_JOBS,
                    /*inc*/ 1,
                    &[("status", "claimed")],
                );
                (ownership_token, input_watermark)
            }
            codex_state::Phase2JobClaimOutcome::SkippedNotDirty => return Err("skipped_not_dirty"),
            codex_state::Phase2JobClaimOutcome::SkippedRunning => return Err("skipped_running"),
        };

        Ok(Claim { token, watermark })
    }

    pub(super) async fn failed(
        session: &Arc<Session>,
        db: &StateRuntime,
        claim: &Claim,
        reason: &'static str,
    ) {
        session.services.session_telemetry.counter(
            metrics::MEMORY_PHASE_TWO_JOBS,
            /*inc*/ 1,
            &[("status", reason)],
        );
        if matches!(
            db.mark_global_phase2_job_failed(
                &claim.token,
                reason,
                phase_two::JOB_RETRY_DELAY_SECONDS,
            )
            .await,
            Ok(false)
        ) {
            let _ = db
                .mark_global_phase2_job_failed_if_unowned(
                    &claim.token,
                    reason,
                    phase_two::JOB_RETRY_DELAY_SECONDS,
                )
                .await;
        }
    }

    pub(super) async fn succeed(
        session: &Arc<Session>,
        db: &StateRuntime,
        claim: &Claim,
        completion_watermark: i64,
        selected_outputs: &[codex_state::Stage1Output],
        reason: &'static str,
    ) -> bool {
        session.services.session_telemetry.counter(
            metrics::MEMORY_PHASE_TWO_JOBS,
            /*inc*/ 1,
            &[("status", reason)],
        );
        db.mark_global_phase2_job_succeeded(&claim.token, completion_watermark, selected_outputs)
            .await
            .unwrap_or(false)
    }
}

mod agent {
    use super::*;

    pub(super) fn get_config(config: Arc<Config>) -> Option<Config> {
        let root = memory_root(&config.codex_home);
        let mut agent_config = config.as_ref().clone();

        agent_config.cwd = root.clone();
        // Consolidation threads must never feed back into phase-1 memory generation.
        agent_config.ephemeral = true;
        agent_config.memories.generate_memories = false;
        agent_config.memories.use_memories = false;
        // Approval policy
        agent_config.permissions.approval_policy = Constrained::allow_only(AskForApproval::Never);
        // Consolidation runs as an internal sub-agent and must not recursively delegate.
        let _ = agent_config.features.disable(Feature::SpawnCsv);
        let _ = agent_config.features.disable(Feature::Collab);
        let _ = agent_config.features.disable(Feature::MemoryTool);

        // Sandbox policy
        let writable_roots = vec![root];
        // The consolidation agent only needs local memory-root write access and no network.
        let consolidation_sandbox_policy = SandboxPolicy::WorkspaceWrite {
            writable_roots,
            read_only_access: Default::default(),
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };
        let consolidation_file_system_sandbox_policy =
            FileSystemSandboxPolicy::from_legacy_sandbox_policy(
                &consolidation_sandbox_policy,
                agent_config.cwd.as_path(),
            );
        let consolidation_network_sandbox_policy =
            NetworkSandboxPolicy::from(&consolidation_sandbox_policy);
        agent_config
            .permissions
            .sandbox_policy
            .set(consolidation_sandbox_policy)
            .ok()?;
        agent_config.permissions.file_system_sandbox_policy =
            consolidation_file_system_sandbox_policy;
        agent_config.permissions.network_sandbox_policy = consolidation_network_sandbox_policy;

        agent_config.model = Some(
            config
                .memories
                .consolidation_model
                .clone()
                .unwrap_or(phase_two::MODEL.to_string()),
        );
        agent_config.model_reasoning_effort = Some(phase_two::REASONING_EFFORT);

        Some(agent_config)
    }

    pub(super) fn get_prompt(config: Arc<Config>) -> Vec<UserInput> {
        let root = memory_root(&config.codex_home);
        let prompt = build_consolidation_prompt(&root);
        vec![UserInput::Text {
            text: prompt,
            text_elements: vec![],
        }]
    }

    /// Handle the agent while it is running.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn handle(
        session: &Arc<Session>,
        claim: Claim,
        new_watermark: i64,
        selected_outputs: Vec<codex_state::Stage1Output>,
        memory_root: codex_utils_absolute_path::AbsolutePathBuf,
        thread_id: ThreadId,
        agent_control: crate::agent::AgentControl,
        phase_two_e2e_timer: Option<codex_otel::Timer>,
    ) {
        let Some(db) = session.services.state_db.clone() else {
            return;
        };
        let session = session.clone();

        tokio::spawn(async move {
            let _phase_two_e2e_timer = phase_two_e2e_timer;

            // TODO(jif) we might have a very small race here.
            let rx = match agent_control.subscribe_status(thread_id).await {
                Ok(rx) => rx,
                Err(err) => {
                    tracing::error!("agent_control.subscribe_status failed: {err:?}");
                    job::failed(&session, &db, &claim, "failed_subscribe_status").await;
                    return;
                }
            };

            // Loop the agent until we have the final status.
            let final_status = loop_agent(
                db.clone(),
                claim.token.clone(),
                new_watermark,
                thread_id,
                rx,
            )
            .await;

            if matches!(final_status, AgentStatus::Completed(_)) {
                if let Some(token_usage) = agent_control.get_total_token_usage(thread_id).await {
                    emit_token_usage_metrics(&session, &token_usage);
                }
                // TODO jif simlpify
                match db
                    .heartbeat_global_phase2_job(&claim.token, phase_two::JOB_LEASE_SECONDS)
                    .await
                {
                    Ok(true) => {}
                    Ok(false) => {
                        tracing::error!(
                            "lost global memory consolidation ownership before committing workspace"
                        );
                        return;
                    }
                    Err(err) => {
                        tracing::error!(
                            "failed confirming global memory consolidation ownership before committing workspace: {err}"
                        );
                        job::failed(&session, &db, &claim, "failed_confirm_ownership").await;
                        return;
                    }
                }
                if let Err(err) = commit_all(&memory_root).await {
                    tracing::error!("failed committing memory workspace: {err}");
                    job::failed(&session, &db, &claim, "failed_workspace_commit").await;
                    return;
                }
                if !job::succeed(
                    &session,
                    &db,
                    &claim,
                    new_watermark,
                    &selected_outputs,
                    "succeeded",
                )
                .await
                {
                    tracing::error!(
                        "failed marking global memory consolidation job succeeded after committing workspace"
                    );
                }
            } else {
                job::failed(&session, &db, &claim, "failed_agent").await;
            }

            // Fire and forget close of the agent.
            if !matches!(final_status, AgentStatus::Shutdown | AgentStatus::NotFound) {
                tokio::spawn(async move {
                    if let Err(err) = agent_control.shutdown_live_agent(thread_id).await {
                        warn!(
                            "failed to auto-close global memory consolidation agent {thread_id}: {err}"
                        );
                    }
                });
            } else {
                tracing::warn!("The agent was already gone");
            }
        });
    }

    async fn loop_agent(
        db: Arc<StateRuntime>,
        token: String,
        _new_watermark: i64,
        thread_id: ThreadId,
        mut rx: watch::Receiver<AgentStatus>,
    ) -> AgentStatus {
        let mut heartbeat_interval =
            tokio::time::interval(Duration::from_secs(phase_two::JOB_HEARTBEAT_SECONDS));
        heartbeat_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            let status = rx.borrow().clone();
            if is_final_agent_status(&status) {
                break status;
            }

            tokio::select! {
                update = rx.changed() => {
                    if update.is_err() {
                        tracing::warn!(
                            "lost status updates for global memory consolidation agent {thread_id}"
                        );
                        break status;
                    }
                }
                _ = heartbeat_interval.tick() => {
                    match db
                        .heartbeat_global_phase2_job(
                            &token,
                            phase_two::JOB_LEASE_SECONDS,
                        )
                        .await
                    {
                        Ok(true) => {}
                        Ok(false) => {
                            break AgentStatus::Errored(
                                "lost global phase-2 ownership during heartbeat".to_string(),
                            );
                        }
                        Err(err) => {
                            break AgentStatus::Errored(format!(
                                "phase-2 heartbeat update failed: {err}"
                            ));
                        }
                    }
                }
            }
        }
    }
}

pub(super) fn get_watermark(
    claimed_watermark: i64,
    latest_memories: &[codex_state::Stage1Output],
) -> i64 {
    latest_memories
        .iter()
        .map(|memory| memory.source_updated_at.timestamp())
        .max()
        .unwrap_or(claimed_watermark)
        .max(claimed_watermark) // todo double check the claimed here.
}

fn emit_metrics(session: &Arc<Session>, counters: Counters) {
    let otel = session.services.session_telemetry.clone();
    if counters.input > 0 {
        otel.counter(metrics::MEMORY_PHASE_TWO_INPUT, counters.input, &[]);
    }

    otel.counter(
        metrics::MEMORY_PHASE_TWO_JOBS,
        /*inc*/ 1,
        &[("status", "agent_spawned")],
    );
}

fn emit_token_usage_metrics(session: &Arc<Session>, token_usage: &TokenUsage) {
    let otel = session.services.session_telemetry.clone();
    otel.histogram(
        metrics::MEMORY_PHASE_TWO_TOKEN_USAGE,
        token_usage.total_tokens.max(0),
        &[("token_type", "total")],
    );
    otel.histogram(
        metrics::MEMORY_PHASE_TWO_TOKEN_USAGE,
        token_usage.input_tokens.max(0),
        &[("token_type", "input")],
    );
    otel.histogram(
        metrics::MEMORY_PHASE_TWO_TOKEN_USAGE,
        token_usage.cached_input(),
        &[("token_type", "cached_input")],
    );
    otel.histogram(
        metrics::MEMORY_PHASE_TWO_TOKEN_USAGE,
        token_usage.output_tokens.max(0),
        &[("token_type", "output")],
    );
    otel.histogram(
        metrics::MEMORY_PHASE_TWO_TOKEN_USAGE,
        token_usage.reasoning_output_tokens.max(0),
        &[("token_type", "reasoning_output")],
    );
}
