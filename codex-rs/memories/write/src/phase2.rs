use crate::build_consolidation_prompt;
use crate::memory_root;
use crate::metrics::MEMORY_PHASE_TWO_E2E_MS;
use crate::metrics::MEMORY_PHASE_TWO_INPUT;
use crate::metrics::MEMORY_PHASE_TWO_JOBS;
use crate::metrics::MEMORY_PHASE_TWO_TOKEN_USAGE;
use crate::prune_old_extension_resources;
use crate::rebuild_raw_memories_file_from_memories;
use crate::runtime::MemoryStartupContext;
use crate::runtime::SpawnedConsolidationAgent;
use crate::sync_rollout_summaries_from_memories;
use crate::workspace::memory_workspace_diff;
use crate::workspace::prepare_memory_workspace;
use crate::workspace::reset_memory_workspace_baseline;
use crate::workspace::write_workspace_diff;
use codex_config::Constrained;
use codex_core::config::Config;
use codex_features::Feature;
use codex_protocol::ThreadId;
use codex_protocol::protocol::AgentStatus;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::TokenUsage;
use codex_protocol::user_input::UserInput;
use codex_state::Stage1Output;
use codex_state::StateRuntime;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

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
pub async fn run(context: Arc<MemoryStartupContext>, config: Arc<Config>) {
    let phase_two_e2e_timer = context.start_timer(MEMORY_PHASE_TWO_E2E_MS);

    let Some(db) = context.state_db() else {
        // This should not happen.
        return;
    };
    let root = memory_root(&config.codex_home);
    let max_raw_memories = config.memories.max_raw_memories_for_consolidation;
    let max_unused_days = config.memories.max_unused_days;

    // 1. Claim the global Phase 2 lock before touching the memory workspace.
    let claim = match job::claim(context.as_ref(), db.as_ref()).await {
        Ok(claim) => claim,
        Err(e) => {
            context.counter(MEMORY_PHASE_TWO_JOBS, /*inc*/ 1, &[("status", e)]);
            return;
        }
    };

    // 2. Ensure the memories root has a git baseline repository.
    if let Err(err) = prepare_memory_workspace(&root).await {
        tracing::error!("failed preparing memory workspace: {err}");
        job::failed(
            context.as_ref(),
            db.as_ref(),
            &claim,
            "failed_prepare_workspace",
        )
        .await;
        return;
    }

    // 3. Build the locked-down config used by the consolidation agent.
    let Some(agent_config) = agent::get_config(config.as_ref()) else {
        // If we can't get the config, we can't consolidate.
        tracing::error!("failed to get agent config");
        job::failed(
            context.as_ref(),
            db.as_ref(),
            &claim,
            "failed_sandbox_policy",
        )
        .await;
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
            job::failed(
                context.as_ref(),
                db.as_ref(),
                &claim,
                "failed_load_stage1_outputs",
            )
            .await;
            return;
        }
    };
    let raw_memory_count = raw_memories.len();
    let new_watermark = get_watermark(claim.watermark, &raw_memories);

    // 5. Sync the current inputs into the memory workspace.
    if let Err(err) = sync_phase2_workspace_inputs(&root, &raw_memories).await {
        tracing::error!("failed syncing phase2 workspace inputs: {err}");
        job::failed(
            context.as_ref(),
            db.as_ref(),
            &claim,
            "failed_sync_workspace_inputs",
        )
        .await;
        return;
    }

    // 6. Use git to decide whether the synced workspace actually changed.
    let workspace_diff = match memory_workspace_diff(&root).await {
        Ok(diff) => diff,
        Err(err) => {
            tracing::error!("failed checking memory workspace changes: {err}");
            job::failed(
                context.as_ref(),
                db.as_ref(),
                &claim,
                "failed_workspace_status",
            )
            .await;
            return;
        }
    };
    if !workspace_diff.has_changes() {
        tracing::error!("Phase 2 no changes");
        // We check only after sync of the file system.
        job::succeed(
            context.as_ref(),
            db.as_ref(),
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
        job::failed(
            context.as_ref(),
            db.as_ref(),
            &claim,
            "failed_workspace_diff_file",
        )
        .await;
        return;
    }

    // 8. Spawn the consolidation agent.
    let prompt = agent::get_prompt(&root);
    let agent = match context
        .spawn_consolidation_agent(agent_config, prompt)
        .await
    {
        Ok(agent) => agent,
        Err(err) => {
            tracing::error!("failed to spawn global memory consolidation agent: {err}");
            job::failed(context.as_ref(), db.as_ref(), &claim, "failed_spawn_agent").await;
            return;
        }
    };

    // 9. Hand off completion handling, heartbeats, and baseline reset.
    agent::handle(
        Arc::clone(&context),
        claim,
        new_watermark,
        raw_memories.clone(),
        root,
        agent,
        phase_two_e2e_timer,
    );

    // 10. Emit dispatch metrics.
    let counters = Counters {
        input: raw_memory_count as i64,
    };
    emit_metrics(context.as_ref(), counters);
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
        context: &MemoryStartupContext,
        db: &StateRuntime,
    ) -> Result<Claim, &'static str> {
        let claim = db
            .try_claim_global_phase2_job(context.thread_id(), crate::stage_two::JOB_LEASE_SECONDS)
            .await
            .map_err(|e| {
                tracing::error!("failed to claim job: {e}");
                "failed_claim"
            })?;
        let (token, watermark) = match claim {
            codex_state::Phase2JobClaimOutcome::Claimed {
                ownership_token,
                input_watermark,
            } => {
                context.counter(
                    MEMORY_PHASE_TWO_JOBS,
                    /*inc*/ 1,
                    &[("status", "claimed")],
                );
                (ownership_token, input_watermark)
            }
            codex_state::Phase2JobClaimOutcome::SkippedRetryUnavailable => {
                return Err("skipped_retry_unavailable");
            }
            codex_state::Phase2JobClaimOutcome::SkippedCooldown => {
                return Err("skipped_cooldown");
            }
            codex_state::Phase2JobClaimOutcome::SkippedRunning => return Err("skipped_running"),
        };

        Ok(Claim { token, watermark })
    }

    pub(super) async fn failed(
        context: &MemoryStartupContext,
        db: &StateRuntime,
        claim: &Claim,
        reason: &'static str,
    ) {
        context.counter(MEMORY_PHASE_TWO_JOBS, /*inc*/ 1, &[("status", reason)]);
        if matches!(
            db.mark_global_phase2_job_failed(
                &claim.token,
                reason,
                crate::stage_two::JOB_RETRY_DELAY_SECONDS,
            )
            .await,
            Ok(false)
        ) {
            let _ = db
                .mark_global_phase2_job_failed_if_unowned(
                    &claim.token,
                    reason,
                    crate::stage_two::JOB_RETRY_DELAY_SECONDS,
                )
                .await;
        }
    }

    pub(super) async fn succeed(
        context: &MemoryStartupContext,
        db: &StateRuntime,
        claim: &Claim,
        completion_watermark: i64,
        selected_outputs: &[codex_state::Stage1Output],
        reason: &'static str,
    ) -> bool {
        context.counter(MEMORY_PHASE_TWO_JOBS, /*inc*/ 1, &[("status", reason)]);
        db.mark_global_phase2_job_succeeded(&claim.token, completion_watermark, selected_outputs)
            .await
            .unwrap_or(false)
    }
}

mod agent {
    use super::*;
    use tracing::warn;

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
        // Consolidation runs as an internal worker and must not recursively delegate.
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
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };
        agent_config
            .permissions
            .set_legacy_sandbox_policy(consolidation_sandbox_policy, agent_config.cwd.as_path())
            .ok()?;

        agent_config.model = Some(
            config
                .memories
                .consolidation_model
                .clone()
                .unwrap_or(crate::stage_two::MODEL.to_string()),
        );
        agent_config.model_reasoning_effort = Some(crate::stage_two::REASONING_EFFORT);

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
        context: Arc<MemoryStartupContext>,
        claim: Claim,
        new_watermark: i64,
        selected_outputs: Vec<codex_state::Stage1Output>,
        memory_root: codex_utils_absolute_path::AbsolutePathBuf,
        agent: SpawnedConsolidationAgent,
        phase_two_e2e_timer: Option<codex_otel::Timer>,
    ) {
        let Some(db) = context.state_db() else {
            return;
        };

        tokio::spawn(async move {
            let _phase_two_e2e_timer = phase_two_e2e_timer;
            let SpawnedConsolidationAgent { thread_id, thread } = agent;

            // Loop the agent until we have the final status.
            let final_status =
                loop_agent(db.clone(), claim.token.clone(), thread_id, &thread).await;

            if matches!(final_status, AgentStatus::Completed(_)) {
                if let Some(token_usage) = thread
                    .token_usage_info()
                    .await
                    .map(|info| info.total_token_usage)
                {
                    emit_token_usage_metrics(context.as_ref(), &token_usage);
                }
                // Do not reset the workspace baseline if we lost the lock.
                let still_owns_lock = match db
                    .heartbeat_global_phase2_job(
                        &claim.token,
                        crate::stage_two::JOB_LEASE_SECONDS,
                    )
                    .await
                    .inspect_err(|err| {
                        tracing::error!(
                            "failed confirming global memory consolidation ownership before resetting workspace baseline: {err}"
                        );
                    }) {
                    Ok(true) => true,
                    Ok(false) => {
                        tracing::error!(
                            "lost global memory consolidation ownership before resetting workspace baseline"
                        );
                        false
                    }
                    Err(_) => {
                        job::failed(context.as_ref(), &db, &claim, "failed_confirm_ownership")
                            .await;
                        false
                    }
                };
                if still_owns_lock {
                    if let Err(err) = reset_memory_workspace_baseline(&memory_root).await {
                        tracing::error!("failed resetting memory workspace baseline: {err}");
                        job::failed(context.as_ref(), &db, &claim, "failed_workspace_commit").await;
                    } else if !job::succeed(
                        context.as_ref(),
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
                }
            } else {
                job::failed(context.as_ref(), &db, &claim, "failed_agent").await;
            }

            let cleanup_context = Arc::clone(&context);
            tokio::spawn(async move {
                if let Err(err) = cleanup_context
                    .shutdown_consolidation_agent(SpawnedConsolidationAgent { thread_id, thread })
                    .await
                {
                    warn!(
                        "failed to auto-close global memory consolidation agent {thread_id}: {err}"
                    );
                }
            });
        });
    }

    async fn loop_agent(
        db: Arc<StateRuntime>,
        token: String,
        thread_id: ThreadId,
        thread: &codex_core::CodexThread,
    ) -> AgentStatus {
        let mut heartbeat_interval =
            tokio::time::interval(Duration::from_secs(crate::stage_two::JOB_HEARTBEAT_SECONDS));
        heartbeat_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut status_poll_interval = tokio::time::interval(Duration::from_secs(1));
        status_poll_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let session_termination = thread.wait_until_terminated();
        tokio::pin!(session_termination);

        loop {
            let status = thread.agent_status().await;
            if is_final_agent_status(&status) {
                break status;
            }

            tokio::select! {
                _ = &mut session_termination => {
                    let status = thread.agent_status().await;
                    if is_final_agent_status(&status) {
                        break status;
                    }
                    tracing::warn!(
                        "memory consolidation agent {thread_id} exited before final status; last status was {status:?}"
                    );
                    break AgentStatus::Errored(format!(
                        "memory consolidation agent exited before final status: {status:?}"
                    ));
                }
                _ = status_poll_interval.tick() => {
                }
                _ = heartbeat_interval.tick() => {
                    match db
                        .heartbeat_global_phase2_job(
                            &token,
                            crate::stage_two::JOB_LEASE_SECONDS,
                        )
                        .await
                    {
                        Ok(true) => {}
                        Ok(false) => {
                            tracing::warn!(
                                "lost global phase-2 ownership during heartbeat for memory consolidation agent {thread_id}"
                            );
                            break AgentStatus::Errored(
                                "lost global phase-2 ownership during heartbeat".to_string(),
                            );
                        }
                        Err(err) => {
                            tracing::warn!(
                                "phase-2 heartbeat update failed for memory consolidation agent {thread_id}: {err}"
                            );
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

fn is_final_agent_status(status: &AgentStatus) -> bool {
    !matches!(
        status,
        AgentStatus::PendingInit | AgentStatus::Running | AgentStatus::Interrupted
    )
}

fn emit_metrics(context: &MemoryStartupContext, counters: Counters) {
    if counters.input > 0 {
        context.counter(MEMORY_PHASE_TWO_INPUT, counters.input, &[]);
    }

    context.counter(
        MEMORY_PHASE_TWO_JOBS,
        /*inc*/ 1,
        &[("status", "agent_spawned")],
    );
}

fn emit_token_usage_metrics(context: &MemoryStartupContext, token_usage: &TokenUsage) {
    context.histogram(
        MEMORY_PHASE_TWO_TOKEN_USAGE,
        token_usage.total_tokens.max(0),
        &[("token_type", "total")],
    );
    context.histogram(
        MEMORY_PHASE_TWO_TOKEN_USAGE,
        token_usage.input_tokens.max(0),
        &[("token_type", "input")],
    );
    context.histogram(
        MEMORY_PHASE_TWO_TOKEN_USAGE,
        token_usage.cached_input(),
        &[("token_type", "cached_input")],
    );
    context.histogram(
        MEMORY_PHASE_TWO_TOKEN_USAGE,
        token_usage.output_tokens.max(0),
        &[("token_type", "output")],
    );
    context.histogram(
        MEMORY_PHASE_TWO_TOKEN_USAGE,
        token_usage.reasoning_output_tokens.max(0),
        &[("token_type", "reasoning_output")],
    );
}
