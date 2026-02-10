use super::*;
use chrono::DateTime;
use chrono::Utc;
use sha2::Digest;
use sha2::Sha256;
use std::time::Duration;

const MEMORY_STARTUP_STAGE: &str = "run_memories_startup_pipeline";
const PHASE_ONE_THREAD_SCAN_LIMIT: usize = 5_000;
const PHASE_ONE_DB_LOCK_RETRY_LIMIT: usize = 3;
const PHASE_ONE_DB_LOCK_RETRY_BACKOFF_MS: u64 = 25;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct MemoryScopeTarget {
    scope_kind: &'static str,
    scope_key: String,
    memory_root: PathBuf,
}

#[derive(Clone, Debug)]
struct ClaimedPhaseOneCandidate {
    candidate: memories::RolloutCandidate,
    claimed_scopes: Vec<(MemoryScopeTarget, String)>,
}

#[derive(Clone)]
struct StageOneRequestContext {
    model_info: ModelInfo,
    otel_manager: OtelManager,
    reasoning_effort: Option<ReasoningEffortConfig>,
    reasoning_summary: ReasoningSummaryConfig,
    turn_metadata_header: Option<String>,
}

impl StageOneRequestContext {
    fn from_turn_context(turn_context: &TurnContext, turn_metadata_header: Option<String>) -> Self {
        Self {
            model_info: turn_context.model_info.clone(),
            otel_manager: turn_context.otel_manager.clone(),
            reasoning_effort: turn_context.reasoning_effort,
            reasoning_summary: turn_context.reasoning_summary,
            turn_metadata_header,
        }
    }
}

pub(super) fn start_memories_startup_task(
    session: &Arc<Session>,
    config: Arc<Config>,
    source: &SessionSource,
) {
    if config.ephemeral
        || !config.features.enabled(Feature::MemoryTool)
        || matches!(source, SessionSource::SubAgent(_))
    {
        return;
    }

    let weak_session = Arc::downgrade(session);
    tokio::spawn(async move {
        let Some(session) = weak_session.upgrade() else {
            return;
        };
        if let Err(err) = run_memories_startup_pipeline(&session, config).await {
            warn!("memories startup pipeline failed: {err}");
        }
    });
}

pub(super) async fn run_memories_startup_pipeline(
    session: &Arc<Session>,
    config: Arc<Config>,
) -> CodexResult<()> {
    let turn_context = session.new_default_turn().await;

    let Some(page) = state_db::list_threads_db(
        session.services.state_db.as_deref(),
        &config.codex_home,
        PHASE_ONE_THREAD_SCAN_LIMIT,
        None,
        ThreadSortKey::UpdatedAt,
        INTERACTIVE_SESSION_SOURCES,
        None,
        false,
    )
    .await
    else {
        warn!("state db unavailable for memories startup pipeline; skipping");
        return Ok(());
    };

    let selection_candidates = memories::select_rollout_candidates_from_db(
        &page.items,
        session.conversation_id,
        PHASE_ONE_THREAD_SCAN_LIMIT,
        memories::PHASE_ONE_MAX_ROLLOUT_AGE_DAYS,
    );
    let claimed_candidates = claim_phase_one_candidates(
        session,
        config.as_ref(),
        selection_candidates,
        memories::MAX_ROLLOUTS_PER_STARTUP,
    )
    .await;
    info!(
        "memory phase-1 candidate selection complete: {} claimed candidate(s) from {} indexed thread(s)",
        claimed_candidates.len(),
        page.items.len()
    );

    if claimed_candidates.is_empty() {
        return Ok(());
    }

    let stage_one_context = StageOneRequestContext::from_turn_context(
        turn_context.as_ref(),
        turn_context.resolve_turn_metadata_header().await,
    );

    let touched_scope_sets =
        futures::stream::iter(claimed_candidates.into_iter())
            .map(|claimed_candidate| {
                let session = Arc::clone(session);
                let stage_one_context = stage_one_context.clone();
                async move {
                    process_memory_candidate(session, claimed_candidate, stage_one_context).await
                }
            })
            .buffer_unordered(memories::PHASE_ONE_CONCURRENCY_LIMIT)
            .collect::<Vec<HashSet<MemoryScopeTarget>>>()
            .await;
    let touched_scopes = touched_scope_sets
        .into_iter()
        .flatten()
        .collect::<HashSet<MemoryScopeTarget>>();
    info!(
        "memory phase-1 extraction complete: {} scope(s) touched",
        touched_scopes.len()
    );

    if touched_scopes.is_empty() {
        return Ok(());
    }

    let consolidation_scope_count = touched_scopes.len();
    futures::stream::iter(touched_scopes.into_iter())
        .map(|scope| {
            let session = Arc::clone(session);
            let config = Arc::clone(&config);
            async move {
                run_memory_consolidation_for_scope(session, config, scope).await;
            }
        })
        .buffer_unordered(memories::PHASE_ONE_CONCURRENCY_LIMIT)
        .collect::<Vec<_>>()
        .await;
    info!(
        "memory phase-2 consolidation dispatch complete: {} scope(s) scheduled",
        consolidation_scope_count
    );

    Ok(())
}

async fn claim_phase_one_candidates(
    session: &Session,
    config: &Config,
    candidates: Vec<memories::RolloutCandidate>,
    max_claimed_candidates: usize,
) -> Vec<ClaimedPhaseOneCandidate> {
    if max_claimed_candidates == 0 {
        return Vec::new();
    }

    let Some(state_db) = session.services.state_db.as_deref() else {
        return Vec::new();
    };

    let mut claimed_candidates = Vec::new();
    for candidate in candidates {
        if claimed_candidates.len() >= max_claimed_candidates {
            break;
        }

        let source_updated_at = parse_source_updated_at_epoch(&candidate);
        let mut claimed_scopes = Vec::<(MemoryScopeTarget, String)>::new();
        for scope in memory_scope_targets_for_candidate(config, &candidate) {
            let Some(claim) = try_claim_phase1_job_with_retry(
                state_db,
                candidate.thread_id,
                scope.scope_kind,
                &scope.scope_key,
                session.conversation_id,
                source_updated_at,
            )
            .await
            else {
                continue;
            };

            if let codex_state::Phase1JobClaimOutcome::Claimed { ownership_token } = claim {
                claimed_scopes.push((scope, ownership_token));
            }
        }

        if !claimed_scopes.is_empty() {
            claimed_candidates.push(ClaimedPhaseOneCandidate {
                candidate,
                claimed_scopes,
            });
        }
    }

    claimed_candidates
}

async fn try_claim_phase1_job_with_retry(
    state_db: &codex_state::StateRuntime,
    thread_id: ThreadId,
    scope_kind: &str,
    scope_key: &str,
    owner_session_id: ThreadId,
    source_updated_at: i64,
) -> Option<codex_state::Phase1JobClaimOutcome> {
    for attempt in 0..=PHASE_ONE_DB_LOCK_RETRY_LIMIT {
        match state_db
            .try_claim_phase1_job(
                thread_id,
                scope_kind,
                scope_key,
                owner_session_id,
                source_updated_at,
                memories::PHASE_ONE_JOB_LEASE_SECONDS,
            )
            .await
        {
            Ok(claim) => return Some(claim),
            Err(err) => {
                let is_locked = err.to_string().contains("database is locked");
                if is_locked && attempt < PHASE_ONE_DB_LOCK_RETRY_LIMIT {
                    tokio::time::sleep(Duration::from_millis(
                        PHASE_ONE_DB_LOCK_RETRY_BACKOFF_MS * (attempt as u64 + 1),
                    ))
                    .await;
                    continue;
                }
                warn!("state db try_claim_phase1_job failed during {MEMORY_STARTUP_STAGE}: {err}");
                return None;
            }
        }
    }
    None
}

async fn process_memory_candidate(
    session: Arc<Session>,
    claimed_candidate: ClaimedPhaseOneCandidate,
    stage_one_context: StageOneRequestContext,
) -> HashSet<MemoryScopeTarget> {
    let candidate = claimed_candidate.candidate;
    let claimed_scopes = claimed_candidate.claimed_scopes;

    let mut ready_scopes = Vec::<(MemoryScopeTarget, String)>::new();
    for (scope, ownership_token) in claimed_scopes {
        if let Err(err) = memories::ensure_layout(&scope.memory_root).await {
            warn!(
                "failed to create memory layout for scope {}:{} root={}: {err}",
                scope.scope_kind,
                scope.scope_key,
                scope.memory_root.display()
            );
            mark_phase1_job_failed_best_effort(
                session.as_ref(),
                candidate.thread_id,
                scope.scope_kind,
                &scope.scope_key,
                &ownership_token,
                "failed to create memory layout",
            )
            .await;
            continue;
        }
        ready_scopes.push((scope, ownership_token));
    }
    if ready_scopes.is_empty() {
        return HashSet::new();
    }

    let (rollout_items, _thread_id, parse_errors) =
        match RolloutRecorder::load_rollout_items(&candidate.rollout_path).await {
            Ok(result) => result,
            Err(err) => {
                warn!(
                    "failed to load rollout {} for memories: {err}",
                    candidate.rollout_path.display()
                );
                fail_claimed_phase_one_jobs(
                    &session,
                    &candidate,
                    &ready_scopes,
                    "failed to load rollout",
                )
                .await;
                return HashSet::new();
            }
        };
    if parse_errors > 0 {
        warn!(
            "rollout {} had {parse_errors} parse errors while preparing stage-1 memory input",
            candidate.rollout_path.display()
        );
    }

    let rollout_contents = match memories::serialize_filtered_rollout_response_items(
        &rollout_items,
        memories::StageOneRolloutFilter::default(),
    ) {
        Ok(contents) => contents,
        Err(err) => {
            warn!(
                "failed to prepare filtered rollout payload {} for memories: {err}",
                candidate.rollout_path.display()
            );
            fail_claimed_phase_one_jobs(
                &session,
                &candidate,
                &ready_scopes,
                "failed to serialize filtered rollout",
            )
            .await;
            return HashSet::new();
        }
    };

    let prompt = Prompt {
        input: vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: memories::build_stage_one_input_message(
                    &candidate.rollout_path,
                    &rollout_contents,
                ),
            }],
            end_turn: None,
            phase: None,
        }],
        tools: Vec::new(),
        parallel_tool_calls: false,
        base_instructions: BaseInstructions {
            text: memories::RAW_MEMORY_PROMPT.to_string(),
        },
        personality: None,
        output_schema: Some(memories::stage_one_output_schema()),
    };

    let mut client_session = session.services.model_client.new_session();
    let mut stream = match client_session
        .stream(
            &prompt,
            &stage_one_context.model_info,
            &stage_one_context.otel_manager,
            stage_one_context.reasoning_effort,
            stage_one_context.reasoning_summary,
            stage_one_context.turn_metadata_header.as_deref(),
        )
        .await
    {
        Ok(stream) => stream,
        Err(err) => {
            warn!(
                "stage-1 memory request failed for rollout {}: {err}",
                candidate.rollout_path.display()
            );
            fail_claimed_phase_one_jobs(
                &session,
                &candidate,
                &ready_scopes,
                "stage-1 memory request failed",
            )
            .await;
            return HashSet::new();
        }
    };

    let output_text = match collect_response_text_until_completed(&mut stream).await {
        Ok(text) => text,
        Err(err) => {
            warn!(
                "failed while waiting for stage-1 memory response for rollout {}: {err}",
                candidate.rollout_path.display()
            );
            fail_claimed_phase_one_jobs(
                &session,
                &candidate,
                &ready_scopes,
                "stage-1 memory response stream failed",
            )
            .await;
            return HashSet::new();
        }
    };

    let stage_one_output = match memories::parse_stage_one_output(&output_text) {
        Ok(output) => output,
        Err(err) => {
            warn!(
                "invalid stage-1 memory payload for rollout {}: {err}",
                candidate.rollout_path.display()
            );
            fail_claimed_phase_one_jobs(
                &session,
                &candidate,
                &ready_scopes,
                "invalid stage-1 memory payload",
            )
            .await;
            return HashSet::new();
        }
    };

    let mut touched_scopes = HashSet::new();
    for (scope, ownership_token) in &ready_scopes {
        if persist_phase_one_memory_for_scope(
            &session,
            &candidate,
            scope,
            ownership_token,
            &stage_one_output.raw_memory,
            &stage_one_output.summary,
        )
        .await
        {
            touched_scopes.insert(scope.clone());
        }
    }

    touched_scopes
}

fn parse_source_updated_at_epoch(candidate: &memories::RolloutCandidate) -> i64 {
    candidate
        .updated_at
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc).timestamp())
        .unwrap_or_else(|| Utc::now().timestamp())
}

fn memory_scope_targets_for_candidate(
    config: &Config,
    candidate: &memories::RolloutCandidate,
) -> Vec<MemoryScopeTarget> {
    vec![
        MemoryScopeTarget {
            scope_kind: memories::MEMORY_SCOPE_KIND_CWD,
            scope_key: memories::memory_scope_key_for_cwd(&candidate.cwd),
            memory_root: memories::memory_root_for_cwd(&config.codex_home, &candidate.cwd),
        },
        MemoryScopeTarget {
            scope_kind: memories::MEMORY_SCOPE_KIND_USER,
            scope_key: memories::MEMORY_SCOPE_KEY_USER.to_string(),
            memory_root: memories::memory_root_for_user(&config.codex_home),
        },
    ]
}

async fn fail_claimed_phase_one_jobs(
    session: &Session,
    candidate: &memories::RolloutCandidate,
    claimed_scopes: &[(MemoryScopeTarget, String)],
    reason: &str,
) {
    for (scope, ownership_token) in claimed_scopes {
        mark_phase1_job_failed_best_effort(
            session,
            candidate.thread_id,
            scope.scope_kind,
            &scope.scope_key,
            ownership_token,
            reason,
        )
        .await;
    }
}

async fn persist_phase_one_memory_for_scope(
    session: &Session,
    candidate: &memories::RolloutCandidate,
    scope: &MemoryScopeTarget,
    ownership_token: &str,
    raw_memory: &str,
    summary: &str,
) -> bool {
    let Some(state_db) = session.services.state_db.as_deref() else {
        mark_phase1_job_failed_best_effort(
            session,
            candidate.thread_id,
            scope.scope_kind,
            &scope.scope_key,
            ownership_token,
            "state db unavailable for scoped thread memory upsert",
        )
        .await;
        return false;
    };

    let lease_renewed = match state_db
        .renew_phase1_job_lease(
            candidate.thread_id,
            scope.scope_kind,
            &scope.scope_key,
            ownership_token,
        )
        .await
    {
        Ok(renewed) => renewed,
        Err(err) => {
            warn!("state db renew_phase1_job_lease failed during {MEMORY_STARTUP_STAGE}: {err}");
            return false;
        }
    };
    if !lease_renewed {
        debug!(
            "memory phase-1 write skipped after ownership changed: rollout={} scope={} scope_key={}",
            candidate.rollout_path.display(),
            scope.scope_kind,
            scope.scope_key
        );
        return false;
    }

    let upserted = match state_db
        .upsert_thread_memory_for_scope_if_phase1_owner(
            candidate.thread_id,
            scope.scope_kind,
            &scope.scope_key,
            ownership_token,
            raw_memory,
            summary,
        )
        .await
    {
        Ok(upserted) => upserted,
        Err(err) => {
            warn!(
                "state db upsert_thread_memory_for_scope_if_phase1_owner failed during {MEMORY_STARTUP_STAGE}: {err}"
            );
            mark_phase1_job_failed_best_effort(
                session,
                candidate.thread_id,
                scope.scope_kind,
                &scope.scope_key,
                ownership_token,
                "failed to upsert scoped thread memory",
            )
            .await;
            return false;
        }
    };
    if upserted.is_none() {
        debug!(
            "memory phase-1 db upsert skipped after ownership changed: rollout={} scope={} scope_key={}",
            candidate.rollout_path.display(),
            scope.scope_kind,
            scope.scope_key
        );
        return false;
    }

    let latest_memories = match state_db
        .get_last_n_thread_memories_for_scope(
            scope.scope_kind,
            &scope.scope_key,
            memories::MAX_RAW_MEMORIES_PER_SCOPE,
        )
        .await
    {
        Ok(memories) => memories,
        Err(err) => {
            warn!(
                "state db get_last_n_thread_memories_for_scope failed during {MEMORY_STARTUP_STAGE}: {err}"
            );
            mark_phase1_job_failed_best_effort(
                session,
                candidate.thread_id,
                scope.scope_kind,
                &scope.scope_key,
                ownership_token,
                "failed to read scope memories after upsert",
            )
            .await;
            return false;
        }
    };

    if let Err(err) =
        memories::sync_raw_memories_from_memories(&scope.memory_root, &latest_memories).await
    {
        warn!(
            "failed syncing raw memories for scope {}:{} root={}: {err}",
            scope.scope_kind,
            scope.scope_key,
            scope.memory_root.display()
        );
        mark_phase1_job_failed_best_effort(
            session,
            candidate.thread_id,
            scope.scope_kind,
            &scope.scope_key,
            ownership_token,
            "failed to sync scope raw memories",
        )
        .await;
        return false;
    }

    if let Err(err) =
        memories::rebuild_memory_summary_from_memories(&scope.memory_root, &latest_memories).await
    {
        warn!(
            "failed rebuilding memory_summary for scope {}:{} root={}: {err}",
            scope.scope_kind,
            scope.scope_key,
            scope.memory_root.display()
        );
        mark_phase1_job_failed_best_effort(
            session,
            candidate.thread_id,
            scope.scope_kind,
            &scope.scope_key,
            ownership_token,
            "failed to rebuild scope memory summary",
        )
        .await;
        return false;
    }

    let mut hasher = Sha256::new();
    hasher.update(summary.as_bytes());
    let summary_hash = format!("{:x}", hasher.finalize());
    let raw_memory_path = scope
        .memory_root
        .join("raw_memories")
        .join(format!("{}.md", candidate.thread_id));
    let marked_succeeded = match state_db
        .mark_phase1_job_succeeded(
            candidate.thread_id,
            scope.scope_kind,
            &scope.scope_key,
            ownership_token,
            &raw_memory_path.display().to_string(),
            &summary_hash,
        )
        .await
    {
        Ok(marked) => marked,
        Err(err) => {
            warn!("state db mark_phase1_job_succeeded failed during {MEMORY_STARTUP_STAGE}: {err}");
            return false;
        }
    };
    if !marked_succeeded {
        return false;
    }

    if let Err(err) = state_db
        .mark_memory_scope_dirty(scope.scope_kind, &scope.scope_key, true)
        .await
    {
        warn!("state db mark_memory_scope_dirty failed during {MEMORY_STARTUP_STAGE}: {err}");
    }

    info!(
        "memory phase-1 raw memory persisted: rollout={} scope={} scope_key={} raw_memory_path={}",
        candidate.rollout_path.display(),
        scope.scope_kind,
        scope.scope_key,
        raw_memory_path.display()
    );
    true
}

async fn run_memory_consolidation_for_scope(
    session: Arc<Session>,
    config: Arc<Config>,
    scope: MemoryScopeTarget,
) {
    let lock_owner = session.conversation_id;
    let Some(lock_acquired) = state_db::try_acquire_memory_consolidation_lock(
        session.services.state_db.as_deref(),
        &scope.memory_root,
        lock_owner,
        memories::CONSOLIDATION_LOCK_LEASE_SECONDS,
        MEMORY_STARTUP_STAGE,
    )
    .await
    else {
        warn!(
            "failed to acquire memory consolidation lock for scope {}:{}; skipping consolidation",
            scope.scope_kind, scope.scope_key
        );
        return;
    };
    if !lock_acquired {
        debug!(
            "memory consolidation lock already held for scope {}:{}; skipping",
            scope.scope_kind, scope.scope_key
        );
        return;
    }

    let Some(state_db) = session.services.state_db.as_deref() else {
        warn!(
            "state db unavailable for scope {}:{}; skipping consolidation",
            scope.scope_kind, scope.scope_key
        );
        let _ = state_db::release_memory_consolidation_lock(
            session.services.state_db.as_deref(),
            &scope.memory_root,
            lock_owner,
            MEMORY_STARTUP_STAGE,
        )
        .await;
        return;
    };

    let latest_memories = match state_db
        .get_last_n_thread_memories_for_scope(
            scope.scope_kind,
            &scope.scope_key,
            memories::MAX_RAW_MEMORIES_PER_SCOPE,
        )
        .await
    {
        Ok(memories) => memories,
        Err(err) => {
            warn!(
                "state db get_last_n_thread_memories_for_scope failed during {MEMORY_STARTUP_STAGE}: {err}"
            );
            let _ = state_db::release_memory_consolidation_lock(
                session.services.state_db.as_deref(),
                &scope.memory_root,
                lock_owner,
                MEMORY_STARTUP_STAGE,
            )
            .await;
            return;
        }
    };

    let memory_root = scope.memory_root.clone();
    if let Err(err) =
        memories::prune_to_recent_memories_and_rebuild_summary(&memory_root, &latest_memories).await
    {
        warn!(
            "failed to refresh phase-1 memory outputs for scope {}:{}: {err}",
            scope.scope_kind, scope.scope_key
        );
        let _ = state_db::release_memory_consolidation_lock(
            session.services.state_db.as_deref(),
            &scope.memory_root,
            lock_owner,
            MEMORY_STARTUP_STAGE,
        )
        .await;
        return;
    }

    if let Err(err) = memories::wipe_consolidation_outputs(&memory_root).await {
        warn!(
            "failed to wipe previous consolidation outputs for scope {}:{}: {err}",
            scope.scope_kind, scope.scope_key
        );
        let _ = state_db::release_memory_consolidation_lock(
            session.services.state_db.as_deref(),
            &scope.memory_root,
            lock_owner,
            MEMORY_STARTUP_STAGE,
        )
        .await;
        return;
    }

    let prompt = memories::build_consolidation_prompt(&memory_root);
    let input = vec![UserInput::Text {
        text: prompt,
        text_elements: vec![],
    }];
    let mut consolidation_config = config.as_ref().clone();
    consolidation_config.cwd = memory_root.clone();
    let source = SessionSource::SubAgent(SubAgentSource::Other(
        memories::MEMORY_CONSOLIDATION_SUBAGENT_LABEL.to_string(),
    ));
    match session
        .services
        .agent_control
        .spawn_agent(consolidation_config, input, Some(source))
        .await
    {
        Ok(consolidation_agent_id) => {
            info!(
                "memory phase-2 consolidation agent started: scope={} scope_key={} agent_id={}",
                scope.scope_kind, scope.scope_key, consolidation_agent_id
            );
            spawn_memory_lock_release_task(
                session.as_ref(),
                scope.memory_root,
                lock_owner,
                consolidation_agent_id,
            );
        }
        Err(err) => {
            warn!(
                "failed to spawn memory consolidation agent for scope {}:{}: {err}",
                scope.scope_kind, scope.scope_key
            );
            let _ = state_db::release_memory_consolidation_lock(
                session.services.state_db.as_deref(),
                &scope.memory_root,
                lock_owner,
                MEMORY_STARTUP_STAGE,
            )
            .await;
        }
    }
}

fn spawn_memory_lock_release_task(
    session: &Session,
    cwd: PathBuf,
    lock_owner: ThreadId,
    consolidation_agent_id: ThreadId,
) {
    let state_db = session.services.state_db.clone();
    let agent_control = session.services.agent_control.clone();
    tokio::spawn(async move {
        let mut status_rx = match agent_control.subscribe_status(consolidation_agent_id).await {
            Ok(status_rx) => status_rx,
            Err(err) => {
                warn!(
                    "failed to subscribe to memory consolidation agent {} for cwd {}: {err}",
                    consolidation_agent_id,
                    cwd.display()
                );
                let _ = state_db::release_memory_consolidation_lock(
                    state_db.as_deref(),
                    &cwd,
                    lock_owner,
                    MEMORY_STARTUP_STAGE,
                )
                .await;
                return;
            }
        };

        let final_status = loop {
            let status = status_rx.borrow().clone();
            if is_final_agent_status(&status) {
                break Some(status);
            }
            if status_rx.changed().await.is_err() {
                warn!(
                    "lost status updates for memory consolidation agent {} in cwd {}; releasing lock",
                    consolidation_agent_id,
                    cwd.display()
                );
                break Some(status);
            }
        };

        let _ = state_db::release_memory_consolidation_lock(
            state_db.as_deref(),
            &cwd,
            lock_owner,
            MEMORY_STARTUP_STAGE,
        )
        .await;
        info!(
            "memory phase-2 consolidation agent finished: cwd={} agent_id={} final_status={:?}",
            cwd.display(),
            consolidation_agent_id,
            final_status
        );
    });
}

async fn mark_phase1_job_failed_best_effort(
    session: &Session,
    thread_id: ThreadId,
    scope_kind: &str,
    scope_key: &str,
    ownership_token: &str,
    failure_reason: &str,
) {
    let Some(state_db) = session.services.state_db.as_deref() else {
        return;
    };
    if let Err(err) = state_db
        .mark_phase1_job_failed(
            thread_id,
            scope_kind,
            scope_key,
            ownership_token,
            failure_reason,
        )
        .await
    {
        warn!("state db mark_phase1_job_failed failed during {MEMORY_STARTUP_STAGE}: {err}");
    }
}

async fn collect_response_text_until_completed(stream: &mut ResponseStream) -> CodexResult<String> {
    let mut output_text = String::new();

    loop {
        let Some(event) = stream.next().await else {
            return Err(CodexErr::Stream(
                "stream closed before response.completed".to_string(),
                None,
            ));
        };

        match event? {
            ResponseEvent::OutputTextDelta(delta) => output_text.push_str(&delta),
            ResponseEvent::OutputItemDone(item) => {
                if output_text.is_empty()
                    && let ResponseItem::Message { content, .. } = item
                    && let Some(text) = crate::compact::content_items_to_text(&content)
                {
                    output_text.push_str(&text);
                }
            }
            ResponseEvent::Completed { .. } => return Ok(output_text),
            _ => {}
        }
    }
}
