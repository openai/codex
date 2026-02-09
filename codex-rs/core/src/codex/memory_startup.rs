use super::*;

const MEMORY_STARTUP_STAGE: &str = "run_memories_startup_pipeline";

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
        200,
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

    let mut existing_memories = Vec::new();
    for item in &page.items {
        if let Some(memory) = state_db::get_thread_memory(
            session.services.state_db.as_deref(),
            item.id,
            MEMORY_STARTUP_STAGE,
        )
        .await
        {
            existing_memories.push(memory);
        }
    }

    let candidates = memories::select_rollout_candidates_from_db(
        &page.items,
        session.conversation_id,
        &existing_memories,
        memories::MAX_ROLLOUTS_PER_STARTUP,
    );
    info!(
        "memory phase-1 candidate selection complete: {} candidate(s) from {} indexed thread(s)",
        candidates.len(),
        page.items.len()
    );

    if candidates.is_empty() {
        return Ok(());
    }

    let stage_one_context = StageOneRequestContext::from_turn_context(
        turn_context.as_ref(),
        turn_context.resolve_turn_metadata_header().await,
    );

    let touched_cwds =
        futures::stream::iter(candidates.into_iter())
            .map(|candidate| {
                let session = Arc::clone(session);
                let config = Arc::clone(&config);
                let stage_one_context = stage_one_context.clone();
                async move {
                    process_memory_candidate(session, config, candidate, stage_one_context).await
                }
            })
            .buffer_unordered(memories::PHASE_ONE_CONCURRENCY_LIMIT)
            .filter_map(futures::future::ready)
            .collect::<HashSet<PathBuf>>()
            .await;
    info!(
        "memory phase-1 extraction complete: {} cwd(s) touched",
        touched_cwds.len()
    );

    if touched_cwds.is_empty() {
        return Ok(());
    }

    let consolidation_cwd_count = touched_cwds.len();
    futures::stream::iter(touched_cwds.into_iter())
        .map(|cwd| {
            let session = Arc::clone(session);
            let config = Arc::clone(&config);
            async move {
                run_memory_consolidation_for_cwd(session, config, cwd).await;
            }
        })
        .buffer_unordered(memories::PHASE_ONE_CONCURRENCY_LIMIT)
        .collect::<Vec<_>>()
        .await;
    info!(
        "memory phase-2 consolidation dispatch complete: {} cwd(s) scheduled",
        consolidation_cwd_count
    );

    Ok(())
}

async fn process_memory_candidate(
    session: Arc<Session>,
    config: Arc<Config>,
    candidate: memories::RolloutCandidate,
    stage_one_context: StageOneRequestContext,
) -> Option<PathBuf> {
    let memory_root = memories::memory_root_for_cwd(&config.codex_home, &candidate.cwd);
    if let Err(err) = memories::ensure_layout(&memory_root).await {
        warn!(
            "failed to create memory layout for cwd {}: {err}",
            candidate.cwd.display()
        );
        return None;
    }

    let (rollout_items, _thread_id, parse_errors) =
        match RolloutRecorder::load_rollout_items(&candidate.rollout_path).await {
            Ok(result) => result,
            Err(err) => {
                warn!(
                    "failed to load rollout {} for memories: {err}",
                    candidate.rollout_path.display()
                );
                return None;
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
            return None;
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
            return None;
        }
    };

    let output_text = match collect_response_text_until_completed(&mut stream).await {
        Ok(text) => text,
        Err(err) => {
            warn!(
                "failed while waiting for stage-1 memory response for rollout {}: {err}",
                candidate.rollout_path.display()
            );
            return None;
        }
    };

    let stage_one_output = match memories::parse_stage_one_output(&output_text) {
        Ok(output) => output,
        Err(err) => {
            warn!(
                "invalid stage-1 memory payload for rollout {}: {err}",
                candidate.rollout_path.display()
            );
            return None;
        }
    };

    let raw_memory_path =
        match memories::write_raw_memory(&memory_root, &candidate, &stage_one_output.raw_memory)
            .await
        {
            Ok(path) => path,
            Err(err) => {
                warn!(
                    "failed to write raw memory for rollout {}: {err}",
                    candidate.rollout_path.display()
                );
                return None;
            }
        };

    if state_db::upsert_thread_memory(
        session.services.state_db.as_deref(),
        candidate.thread_id,
        &stage_one_output.raw_memory,
        &stage_one_output.summary,
        MEMORY_STARTUP_STAGE,
    )
    .await
    .is_none()
    {
        warn!(
            "failed to upsert thread memory for rollout {}; removing {}",
            candidate.rollout_path.display(),
            raw_memory_path.display()
        );
        if let Err(err) = tokio::fs::remove_file(&raw_memory_path).await
            && err.kind() != std::io::ErrorKind::NotFound
        {
            warn!(
                "failed to remove orphaned raw memory {}: {err}",
                raw_memory_path.display()
            );
        }
        return None;
    }
    info!(
        "memory phase-1 raw memory persisted: rollout={} cwd={} raw_memory_path={}",
        candidate.rollout_path.display(),
        candidate.cwd.display(),
        raw_memory_path.display()
    );

    Some(candidate.cwd)
}

async fn run_memory_consolidation_for_cwd(
    session: Arc<Session>,
    config: Arc<Config>,
    cwd: PathBuf,
) {
    let lock_owner = session.conversation_id;
    let Some(lock_acquired) = state_db::try_acquire_memory_consolidation_lock(
        session.services.state_db.as_deref(),
        &cwd,
        lock_owner,
        memories::CONSOLIDATION_LOCK_LEASE_SECONDS,
        MEMORY_STARTUP_STAGE,
    )
    .await
    else {
        warn!(
            "failed to acquire memory consolidation lock for cwd {}; skipping consolidation",
            cwd.display()
        );
        return;
    };
    if !lock_acquired {
        debug!(
            "memory consolidation lock already held for cwd {}; skipping",
            cwd.display()
        );
        return;
    }

    let Some(latest_memories) = state_db::get_last_n_thread_memories_for_cwd(
        session.services.state_db.as_deref(),
        &cwd,
        memories::MAX_RAW_MEMORIES_PER_CWD,
        MEMORY_STARTUP_STAGE,
    )
    .await
    else {
        warn!(
            "failed to read recent thread memories for cwd {}; skipping consolidation",
            cwd.display()
        );
        let _ = state_db::release_memory_consolidation_lock(
            session.services.state_db.as_deref(),
            &cwd,
            lock_owner,
            MEMORY_STARTUP_STAGE,
        )
        .await;
        return;
    };

    let memory_root = memories::memory_root_for_cwd(&config.codex_home, &cwd);
    if let Err(err) =
        memories::prune_to_recent_memories_and_rebuild_summary(&memory_root, &latest_memories).await
    {
        warn!(
            "failed to refresh phase-1 memory outputs for cwd {}: {err}",
            cwd.display()
        );
        let _ = state_db::release_memory_consolidation_lock(
            session.services.state_db.as_deref(),
            &cwd,
            lock_owner,
            MEMORY_STARTUP_STAGE,
        )
        .await;
        return;
    }

    if let Err(err) = memories::wipe_consolidation_outputs(&memory_root).await {
        warn!(
            "failed to wipe previous consolidation outputs for cwd {}: {err}",
            cwd.display()
        );
        let _ = state_db::release_memory_consolidation_lock(
            session.services.state_db.as_deref(),
            &cwd,
            lock_owner,
            MEMORY_STARTUP_STAGE,
        )
        .await;
        return;
    }

    let prompt = memories::build_consolidation_prompt(&memory_root);
    let mut consolidation_config = config.as_ref().clone();
    consolidation_config.cwd = memory_root.clone();
    let source = SessionSource::SubAgent(SubAgentSource::Other(
        memories::MEMORY_CONSOLIDATION_SUBAGENT_LABEL.to_string(),
    ));
    match session
        .services
        .agent_control
        .spawn_agent(consolidation_config, prompt, Some(source))
        .await
    {
        Ok(consolidation_agent_id) => {
            info!(
                "memory phase-2 consolidation agent started: cwd={} agent_id={}",
                cwd.display(),
                consolidation_agent_id
            );
            spawn_memory_lock_release_task(
                session.as_ref(),
                cwd,
                lock_owner,
                consolidation_agent_id,
            );
        }
        Err(err) => {
            warn!(
                "failed to spawn memory consolidation agent for cwd {}: {err}",
                cwd.display()
            );
            let _ = state_db::release_memory_consolidation_lock(
                session.services.state_db.as_deref(),
                &cwd,
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
