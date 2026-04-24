use crate::agent::AgentStatus;
use crate::agent::status::is_final as is_final_agent_status;
use crate::config::Config;
use crate::memories::extensions::prune_old_extension_resources;
use crate::memories::memory_root;
use crate::memories::metrics;
use crate::memories::phase_two;
use crate::memories::prompts::build_consolidation_prompt;
use crate::memories::storage::rebuild_raw_memories_file_from_memories;
use crate::memories::storage::sync_rollout_summaries_from_memories;
use crate::memories::workspace::memory_workspace_diff;
use crate::memories::workspace::prepare_memory_workspace;
use crate::memories::workspace::reset_memory_workspace_baseline;
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
use codex_state::Stage1Output;
use codex_state::StateRuntime;
use std::collections::HashMap;
use std::path::Path;
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

    // 1. Claim the global Phase 2 lock before touching the memory workspace.
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

    // 2. Ensure the memories root has a git baseline repository.
    if let Err(err) = prepare_memory_workspace(&root).await {
        tracing::error!("failed preparing memory workspace: {err}");
        job::failed(session, db, &claim, "failed_prepare_workspace").await;
        return;
    }

    // 3. Build the locked-down config used by the consolidation agent.
    let Some(agent_config) = agent::get_config(config.as_ref()) else {
        // If we can't get the config, we can't consolidate.
        tracing::error!("failed to get agent config");
        job::failed(session, db, &claim, "failed_sandbox_policy").await;
        return;
    };

    // 4. Load current DB-backed Phase 2 inputs.
    let raw_memories = match db
        .get_phase2_input_selection(max_raw_memories, max_unused_days)
        .await
    {
        Ok(raw_memories) => raw_memories,
        Err(err) => {
            tracing::error!("failed to list stage1 outputs from global: {err}");
            job::failed(session, db, &claim, "failed_load_stage1_outputs").await;
            return;
        }
    };
    let raw_memory_count = raw_memories.len();
    let new_watermark = get_watermark(claim.watermark, &raw_memories);

    // 5. Sync the current inputs into the memory workspace.
    if let Err(err) = sync_phase2_workspace_inputs(&root, &raw_memories).await {
        tracing::error!("failed syncing phase2 workspace inputs: {err}");
        job::failed(session, db, &claim, "failed_sync_workspace_inputs").await;
        return;
    }

    // 6. Use git to decide whether the synced workspace actually changed.
    let workspace_diff = match memory_workspace_diff(&root).await {
        Ok(diff) => diff,
        Err(err) => {
            tracing::error!("failed checking memory workspace changes: {err}");
            job::failed(session, db, &claim, "failed_workspace_status").await;
            return;
        }
    };
    if !workspace_diff.has_changes() {
        tracing::error!("Phase 2 no changes");
        // We check only after sync of the file system.
        job::succeed(
            session,
            db,
            &claim,
            new_watermark,
            &raw_memories,
            "succeeded_no_workspace_changes",
        )
        .await;
        return;
    }

    // 7. Persist the diff for the consolidation agent to inspect.
    if let Err(err) = write_workspace_diff(&root, &workspace_diff).await {
        tracing::error!("failed writing memory workspace diff file: {err}");
        job::failed(session, db, &claim, "failed_workspace_diff_file").await;
        return;
    }

    // 8. Spawn the consolidation agent.
    let prompt = agent::get_prompt(&root);
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

    // 9. Hand off completion handling, heartbeats, and baseline reset.
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

    // 10. Emit dispatch metrics.
    let counters = Counters {
        input: raw_memory_count as i64,
    };
    emit_metrics(session, counters);
}

async fn sync_phase2_workspace_inputs(
    root: &Path,
    raw_memories: &[Stage1Output],
) -> std::io::Result<()> {
    let raw_memory_count = raw_memories.len();
    sync_rollout_summaries_from_memories(root, raw_memories, raw_memory_count).await?;
    rebuild_raw_memories_file_from_memories(root, raw_memories, raw_memory_count).await?;
    prune_old_extension_resources(root).await;
    Ok(())
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
            codex_state::Phase2JobClaimOutcome::SkippedRetryUnavailable => {
                return Err("skipped_retry_unavailable");
            }
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

    pub(super) fn get_config(config: &Config) -> Option<Config> {
        let root = memory_root(&config.codex_home);
        let mut agent_config = config.clone();

        agent_config.cwd = root.clone();
        // Consolidation threads must never feed back into phase-1 memory generation.
        agent_config.ephemeral = true;
        agent_config.memories.generate_memories = false;
        agent_config.memories.use_memories = false;
        agent_config.include_apps_instructions = false;
        agent_config.mcp_servers = Constrained::allow_only(HashMap::new());
        // Approval policy
        agent_config.permissions.approval_policy = Constrained::allow_only(AskForApproval::Never);
        // Consolidation runs as an internal sub-agent and must not recursively delegate.
        let _ = agent_config.features.disable(Feature::SpawnCsv);
        let _ = agent_config.features.disable(Feature::Collab);
        let _ = agent_config.features.disable(Feature::MemoryTool);
        let _ = agent_config.features.disable(Feature::Apps);
        let _ = agent_config.features.disable(Feature::Plugins);
        let _ = agent_config
            .features
            .disable(Feature::SkillMcpDependencyInstall);

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

    pub(super) fn get_prompt(root: &Path) -> Vec<UserInput> {
        let prompt = build_consolidation_prompt(root);
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
            let final_status = loop_agent(db.clone(), claim.token.clone(), thread_id, rx).await;

            if matches!(final_status, AgentStatus::Completed(_)) {
                if let Some(token_usage) = agent_control.get_total_token_usage(thread_id).await {
                    emit_token_usage_metrics(&session, &token_usage);
                }
                // Do not reset the workspace baseline if we lost the lock.
                let Ok(still_owns_lock) = db
                    .heartbeat_global_phase2_job(&claim.token, phase_two::JOB_LEASE_SECONDS)
                    .await
                    .inspect_err(|err| {
                        tracing::error!(
                            "failed confirming global memory consolidation ownership before resetting workspace baseline: {err}"
                        );
                    })
                else {
                    job::failed(&session, &db, &claim, "failed_confirm_ownership").await;
                    return;
                };
                if !still_owns_lock {
                    tracing::error!(
                        "lost global memory consolidation ownership before resetting workspace baseline"
                    );
                    return;
                }

                if let Err(err) = reset_memory_workspace_baseline(&memory_root).await {
                    tracing::error!("failed resetting memory workspace baseline: {err}");
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
                        "failed marking global memory consolidation job succeeded after resetting workspace baseline"
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
        .max(claimed_watermark)
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
