use super::super::prompt::BUNDLED_GUARDIAN_POLICY_TEMPLATE;
use super::super::prompt::guardian_policy_prompt_with_config_and_template;
use super::*;
use crate::context_manager::ContextManager;
use codex_history::CodexHarnessMetadata;
use codex_history::ResponseItemEnvelope;
use codex_protocol::openai_models::AutoReviewMessages;
use codex_protocol::openai_models::ModelMessages;
use codex_protocol::protocol::AgentStatus;
use codex_protocol::protocol::ErrorEvent;
use codex_protocol::protocol::Submission;
use codex_protocol::protocol::TurnAbortReason;
use codex_protocol::protocol::TurnAbortedEvent;
use codex_protocol::protocol::TurnCompleteEvent;

#[tokio::test]
async fn run_review_preserves_evidence_during_parent_compaction() {
    const EVIDENCE: &str = "The inspected repository is public.";
    let (parent, turn, _events) =
        crate::session::tests::make_session_and_context_with_auth_and_config_and_rx(
            codex_login::CodexAuth::from_api_key("Test API Key"),
            Vec::new(),
            |config| {
                config
                    .features
                    .enable(Feature::GuardianThreadContext)
                    .unwrap();
                config.features.disable(Feature::TokenBudget).unwrap();
            },
        )
        .await;
    let mut params = test_review_params().await;
    params.spawn_config = build_guardian_review_session_config(
        turn.config.as_ref(),
        /*live_network_config*/ None,
        &params.model,
        params.reasoning_effort.clone(),
        /*model_messages*/ None,
    )
    .unwrap();
    params.parent_session = Arc::clone(&parent);
    params.parent_context = GuardianReviewContext::from(Arc::clone(&turn));
    params.compaction_model_hash = Some("matching".to_owned());
    let evidence: ResponseItem = serde_json::from_value(serde_json::json!({
        "type": "function_call_output", "call_id": "prior-inspection", "output": EVIDENCE
    }))
    .unwrap();
    parent
        .record_conversation_items(&turn, turn.model_info(), &[evidence])
        .await;
    params.parent_history = parent.clone_history().await;

    // An idle, prewarmed reviewer is a normal manager state.
    let (mut reviewer, tx_event, rx_sub) = test_review_session().await;
    reviewer.reuse_key = GuardianReviewSessionReuseKey::from_spawn_config(
        &params.spawn_config,
        parent.user_instructions().await,
        params.parent_history.history_version(),
        parent.guardian_context_mode,
    )
    .with_environments(params.parent_context.environments())
    .with_node_repl_policy_eligibility(
        params
            .parent_context
            .turn()
            .model_info()
            .node_repl_auto_review_required,
    )
    .with_node_repl_policy(&params.node_repl_policy);
    let manager = GuardianReviewSessionManager::default();
    prewarm_test_session(&manager, reviewer).await;
    // Capture the review context, then compact the parent before the reviewer builds its prompt.
    let prepared = factory::prepare_review(params).await.unwrap();
    let checkpoint: ResponseItem = serde_json::from_value(serde_json::json!({
        "type": "compaction", "id": "cmp_new", "encrypted_content": "new-checkpoint"
    }))
    .unwrap();
    let (window_number, window_ids) = parent.advance_auto_compact_window().await;
    parent
        .replace_compacted_history(
            vec![checkpoint.into()],
            /*reference_context_item*/ None,
            /*world_state_baseline*/ None,
            crate::compact::CompactedHistoryMetadata {
                message: String::new(),
                window_number,
                window_ids,
                compaction_response_id: None,
                compaction_model_hash: Some("matching".to_owned()),
            },
        )
        .await;
    let ((outcome, _), submitted_text) = tokio::join!(manager.review(prepared), async {
        let submission = rx_sub.recv().await.unwrap();
        let id = submission.id;
        let Op::TurnInput { request, reply, .. } = submission.op else {
            panic!("expected reviewer prompt");
        };
        let codex_protocol::turn_input::TurnInput::UserInput { content, .. } = request.input else {
            panic!("expected user input");
        };
        let text = serde_json::to_string(&content).unwrap();
        reply
            .send(Ok(TurnInputSubmission::Started {
                turn_id: id.clone(),
            }))
            .unwrap();
        tx_event
            .send(turn_complete_event(
                &id,
                Some("review finished"),
                /*time_to_first_token_ms*/ None,
            ))
            .await
            .unwrap();
        text
    });
    assert!(matches!(
        outcome,
        GuardianReviewSessionOutcome::Completed(Ok(_))
    ));
    assert!(
        submitted_text.contains(EVIDENCE),
        "review must retain evidence from before the concurrent compaction: {submitted_text}"
    );
}

async fn test_review_session() -> (
    GuardianReviewSession,
    async_channel::Sender<Event>,
    async_channel::Receiver<Submission>,
) {
    let (session, _turn, _rx) = crate::session::tests::make_session_and_context_with_rx().await;
    let (tx_sub, rx_sub) = async_channel::bounded(4);
    let (tx_event, rx_event) = async_channel::unbounded();
    let (_agent_status_tx, agent_status) = tokio::sync::watch::channel(AgentStatus::PendingInit);
    let reuse_key = GuardianReviewSessionReuseKey::from_spawn_config(
        session.get_config().await.as_ref(),
        session.user_instructions().await,
        session.clone_history().await.history_version(),
        GuardianContextMode::Legacy,
    );

    (
        GuardianReviewSession {
            session,
            io: SessionIo {
                tx_sub,
                rx_event,
                agent_status,
                session_loop_termination: crate::session::completed_session_loop_termination(),
            },
            cancel_token: CancellationToken::new(),
            reuse_key,
            state: Mutex::new(GuardianReviewState {
                prior_review_count: 0,
                last_reviewed_transcript_cursor: None,
                last_admitted_node_repl_response_sequence: 0,
                pending_node_repl_evidence_admission: None,
                last_committed_fork_snapshot: None,
            }),
        },
        tx_event,
        rx_sub,
    )
}

fn turn_complete_event(
    turn_id: &str,
    last_agent_message: Option<&str>,
    time_to_first_token_ms: Option<i64>,
) -> Event {
    Event {
        id: turn_id.to_string(),
        msg: EventMsg::TurnComplete(TurnCompleteEvent {
            turn_id: turn_id.to_string(),
            started_at: None,
            last_agent_message: last_agent_message.map(str::to_string),
            error: None,
            completed_at: None,
            duration_ms: None,
            time_to_first_token_ms,
        }),
    }
}

fn turn_aborted_event(turn_id: &str) -> Event {
    Event {
        id: turn_id.to_string(),
        msg: EventMsg::TurnAborted(TurnAbortedEvent {
            turn_id: Some(turn_id.to_string()),
            started_at: None,
            reason: TurnAbortReason::Interrupted,
            completed_at: None,
            duration_ms: None,
        }),
    }
}

async fn test_review_params() -> GuardianReviewSessionParams {
    let (session, turn) = crate::session::tests::make_session_and_context().await;
    let model = turn.model_info().slug.clone();
    let reasoning_effort = turn.reasoning_effort().cloned();
    let reasoning_summary = turn.reasoning_summary();
    let personality = turn.personality();
    #[allow(deprecated)]
    let cwd = turn.cwd.clone();
    let spawn_config = build_guardian_review_session_config(
        turn.config.as_ref(),
        /*live_network_config*/ None,
        model.as_str(),
        reasoning_effort.clone(),
        /*model_messages*/ None,
    )
    .expect("guardian config");

    GuardianReviewSessionParams {
        parent_history: session.clone_history().await,
        parent_session: Arc::new(session),
        parent_context: GuardianReviewContext::from(Arc::new(turn)),
        spawn_config,
        node_repl_policy: GuardianNodeReplPolicy::from_model_messages(/*messages*/ None),
        request: GuardianApprovalRequest::ExecCommand {
            id: "shell-1".to_string(),
            environment_id: codex_exec_server::LOCAL_ENVIRONMENT_ID.to_string(),
            command: vec!["git".to_string(), "status".to_string()],
            cwd: cwd.clone().into(),
            guardian_cwd: codex_utils_path_uri::LegacyAppPathString::from_abs_path(&cwd),
            sandbox_permissions: crate::sandboxing::SandboxPermissions::UseDefault,
            additional_permissions: None,
            justification: Some("Inspect repo state.".to_string()),
            tty: false,
        },
        reasons: ApprovalRequestReasons::default(),
        schema: super::super::guardian_output_schema(),
        model,
        compaction_model_hash: None,
        reasoning_effort,
        guardian_default_review_model_id: "codex-auto-review".to_string(),
        guardian_catalog_contains_auto_review: true,
        guardian_review_model_overridden: false,
        guardian_review_model_override: None,
        reasoning_summary,
        personality,
        external_cancel: None,
        deadline: tokio::time::Instant::now() + Duration::from_secs(30),
    }
}

#[tokio::test]
async fn spawned_guardian_session_preserves_windows_sandbox_proxy_settings() {
    let params = test_review_params().await;
    let manager = params.parent_session.guardian_review_session();
    prewarm_guardian_review_session(
        params.parent_session,
        Arc::clone(params.parent_context.turn()),
    )
    .await
    .expect("initialize Guardian session");
    let mode = manager
        .trunk()
        .await
        .expect("Guardian session")
        .session
        .windows_sandbox_proxy_settings_mode;

    assert_eq!(
        mode,
        codex_sandboxing::WindowsSandboxProxySettingsMode::Preserve
    );
    manager.shutdown().await;
}

#[tokio::test]
async fn guardian_review_session_config_change_invalidates_cached_session() {
    let parent_config = crate::config::test_config().await;
    let cached_spawn_config = build_guardian_review_session_config(
        &parent_config,
        /*live_network_config*/ None,
        "active-model",
        /*reasoning_effort*/ None,
        /*model_messages*/ None,
    )
    .expect("cached guardian config");
    let cached_reuse_key = GuardianReviewSessionReuseKey::from_spawn_config(
        &cached_spawn_config,
        /*user_instructions*/ None,
        /*parent_history_version*/ 0,
        GuardianContextMode::Legacy,
    );

    let mut changed_parent_config = parent_config;
    changed_parent_config.model_provider.base_url =
        Some("https://guardian.example.invalid/v1".to_string());
    let next_spawn_config = build_guardian_review_session_config(
        &changed_parent_config,
        /*live_network_config*/ None,
        "active-model",
        /*reasoning_effort*/ None,
        /*model_messages*/ None,
    )
    .expect("next guardian config");
    let next_reuse_key = GuardianReviewSessionReuseKey::from_spawn_config(
        &next_spawn_config,
        /*user_instructions*/ None,
        /*parent_history_version*/ 0,
        GuardianContextMode::Legacy,
    );

    assert_eq!(
        cached_reuse_key.cwd,
        PathUri::from_abs_path(&cached_spawn_config.cwd)
    );
    assert_ne!(cached_reuse_key, next_reuse_key);
    assert_eq!(
        cached_reuse_key,
        GuardianReviewSessionReuseKey::from_spawn_config(
            &cached_spawn_config,
            /*user_instructions*/ None,
            /*parent_history_version*/ 0,
            GuardianContextMode::Legacy,
        )
    );

    assert_eq!(
        cached_reuse_key,
        GuardianReviewSessionReuseKey::from_spawn_config(
            &cached_spawn_config,
            /*user_instructions*/ None,
            /*parent_history_version*/ 1,
            GuardianContextMode::Legacy,
        )
    );
    assert_ne!(
        cached_reuse_key
            .clone()
            .with_node_repl_policy_eligibility(/*required*/ false),
        cached_reuse_key
            .clone()
            .with_node_repl_policy_eligibility(/*required*/ true),
        "switching parent-model Node REPL eligibility must invalidate reviewer history"
    );
    let messages = serde_json::from_value(serde_json::json!({
        "auto_review": { "node_repl_policy": "Catalog REPL policy." }
    }))
    .expect("catalog model messages");
    assert_ne!(
        cached_reuse_key.clone().with_node_repl_policy(
            &GuardianNodeReplPolicy::from_model_messages(/*messages*/ None),
        ),
        cached_reuse_key.with_node_repl_policy(&GuardianNodeReplPolicy::from_model_messages(Some(
            &messages
        )),),
        "changing the effective Node REPL policy must invalidate reviewer history"
    );

    let mut compaction_enabled_config = cached_spawn_config;
    compaction_enabled_config
        .features
        .enable(Feature::GuardianReuseParentCompaction)
        .expect("Guardian parent-compaction reuse should be configurable");
    assert_ne!(
        GuardianReviewSessionReuseKey::from_spawn_config(
            &compaction_enabled_config,
            /*user_instructions*/ None,
            /*parent_history_version*/ 0,
            GuardianContextMode::Legacy,
        ),
        GuardianReviewSessionReuseKey::from_spawn_config(
            &compaction_enabled_config,
            /*user_instructions*/ None,
            /*parent_history_version*/ 1,
            GuardianContextMode::Legacy,
        )
    );
}

#[test_case::test_case(true; "thread owned")]
#[test_case::test_case(false; "legacy")]
#[tokio::test]
async fn encrypted_parent_compaction_requires_original_item_id(thread_context_enabled: bool) {
    let (session, _) = crate::session::tests::make_session_and_context().await;
    let mut features = session.get_config().await.features.clone();
    features
        .enable(Feature::GuardianReuseParentCompaction)
        .expect("legacy reuse");
    features
        .set_enabled(Feature::GuardianThreadContext, thread_context_enabled)
        .expect("context mode");
    let policy =
        ReviewContextPolicy::for_context(GuardianContextMode::from_features(&features), &features);
    let item = ResponseItem::Compaction {
        id: Some(codex_protocol::ResponseItemId::from_server(
            "cmp_guardian_parent_summary".to_string(),
        )),
        encrypted_content: "encrypted guardian parent summary".to_string(),
        internal_chat_message_metadata_passthrough: None,
    };

    let mut history = ContextManager::new();
    history.replace_annotated(vec![ResponseItemEnvelope {
        item: item.clone(),
        metadata: Some(CodexHarnessMetadata {
            compaction_model_hash: Some("compatible".to_owned()),
            ..Default::default()
        }),
    }]);
    assert_eq!(
        policy
            .parent_compaction(&history, Some("compatible"))
            .expect("valid checkpoint"),
        Some(item)
    );
    // The latest unusable checkpoint must not fall back to the older valid one.
    let mut items = history.annotated_items().to_vec();
    items.push(
        ResponseItem::Compaction {
            id: None,
            encrypted_content: "encrypted guardian parent summary".to_string(),
            internal_chat_message_metadata_passthrough: None,
        }
        .into(),
    );
    history.replace_annotated(items);
    let result = policy.parent_compaction(&history, Some("compatible"));
    if thread_context_enabled {
        assert!(result.is_err());
    } else {
        assert_eq!(result.expect("legacy omission"), None);
    }
}

#[tokio::test]
async fn guardian_prompt_cache_key_is_scoped_to_parent_thread() {
    let session_source =
        SessionSource::SubAgent(SubAgentSource::Other(GUARDIAN_REVIEWER_NAME.to_string()));
    let parent_thread_id = ThreadId::new();
    let key = prompt_cache_key_override_for_review_session(&session_source, Some(parent_thread_id))
        .expect("guardian prompt cache key");

    assert_eq!(key, format!("guardian:{parent_thread_id}"));
    assert!(
        key.len() <= 64,
        "guardian prompt cache key should fit the Responses API limit"
    );
    assert_eq!(
        key,
        prompt_cache_key_override_for_review_session(&session_source, Some(parent_thread_id))
            .expect("same guardian prompt cache key")
    );
    assert_ne!(
        key,
        prompt_cache_key_override_for_review_session(&session_source, Some(ThreadId::new()))
            .expect("different parent guardian prompt cache key")
    );
    assert_eq!(
        None,
        prompt_cache_key_override_for_review_session(&SessionSource::Cli, Some(parent_thread_id))
    );
    assert_eq!(
        None,
        prompt_cache_key_override_for_review_session(
            &session_source,
            /*parent_thread_id*/ None
        )
    );
}

#[tokio::test]
async fn guardian_review_session_compact_scope_change_invalidates_cached_session() {
    let parent_config = crate::config::test_config().await;
    let cached_spawn_config = build_guardian_review_session_config(
        &parent_config,
        /*live_network_config*/ None,
        "active-model",
        /*reasoning_effort*/ None,
        /*model_messages*/ None,
    )
    .expect("cached guardian config");
    let cached_reuse_key = GuardianReviewSessionReuseKey::from_spawn_config(
        &cached_spawn_config,
        /*user_instructions*/ None,
        /*parent_history_version*/ 0,
        GuardianContextMode::Legacy,
    );

    let mut changed_parent_config = parent_config;
    changed_parent_config.model_auto_compact_token_limit_scope =
        AutoCompactTokenLimitScope::BodyAfterPrefix;
    let next_spawn_config = build_guardian_review_session_config(
        &changed_parent_config,
        /*live_network_config*/ None,
        "active-model",
        /*reasoning_effort*/ None,
        /*model_messages*/ None,
    )
    .expect("next guardian config");
    let next_reuse_key = GuardianReviewSessionReuseKey::from_spawn_config(
        &next_spawn_config,
        /*user_instructions*/ None,
        /*parent_history_version*/ 0,
        GuardianContextMode::Legacy,
    );

    assert_ne!(cached_reuse_key, next_reuse_key);
}

#[tokio::test]
async fn guardian_review_session_config_disables_hooks() {
    let mut parent_config = crate::config::test_config().await;
    parent_config
        .features
        .enable(Feature::CodexHooks)
        .expect("enable hooks on parent config");

    let guardian_config = build_guardian_review_session_config(
        &parent_config,
        /*live_network_config*/ None,
        "active-model",
        /*reasoning_effort*/ None,
        /*model_messages*/ None,
    )
    .expect("guardian config");

    assert!(!guardian_config.features.enabled(Feature::CodexHooks));
}

#[tokio::test]
async fn guardian_review_session_config_disables_skill_instructions() {
    let mut parent_config = crate::config::test_config().await;
    parent_config.include_skill_instructions = true;

    let guardian_config = build_guardian_review_session_config(
        &parent_config,
        /*live_network_config*/ None,
        "active-model",
        /*reasoning_effort*/ None,
        /*model_messages*/ None,
    )
    .expect("guardian config");

    assert!(!guardian_config.include_skill_instructions);
}

#[tokio::test]
async fn guardian_review_session_config_prefers_managed_policy_and_uses_catalog_template() {
    let mut parent_config = crate::config::test_config().await;
    let managed_policy = "Use the managed Guardian policy.";
    let catalog_template = "Catalog Guardian template:\n{{ tenant_policy_config }}";
    parent_config.guardian_policy_config = Some(managed_policy.to_string());
    let model_messages = ModelMessages {
        persistent_instructions: None,
        tools: None,
        instructions_template: None,
        instructions_variables: None,
        approvals: None,
        collaboration_modes: None,
        auto_review: Some(AutoReviewMessages {
            policy: Some("Use the catalog Guardian policy.".to_string()),
            policy_template: Some(catalog_template.to_string()),
            node_repl_policy: None,
            rejection_instructions: None,
            timeout_instructions: None,
        }),
        permissions: None,
        multi_agent: None,
        token_budget: None,
        confirmation_policies: None,
        guardian_v2: None,
    };

    let guardian_config = build_guardian_review_session_config(
        &parent_config,
        /*live_network_config*/ None,
        "active-model",
        /*reasoning_effort*/ None,
        Some(&model_messages),
    )
    .expect("guardian config");

    assert_eq!(
        guardian_config.base_instructions,
        Some(guardian_policy_prompt_with_config_and_template(
            managed_policy,
            catalog_template,
        ))
    );
}

#[tokio::test]
async fn guardian_review_session_config_preserves_explicit_empty_catalog_policy() {
    let parent_config = crate::config::test_config().await;
    let model_messages = ModelMessages {
        persistent_instructions: None,
        tools: None,
        instructions_template: None,
        instructions_variables: None,
        approvals: None,
        collaboration_modes: None,
        auto_review: Some(AutoReviewMessages {
            policy: Some(String::new()),
            policy_template: None,
            node_repl_policy: None,
            rejection_instructions: None,
            timeout_instructions: None,
        }),
        permissions: None,
        multi_agent: None,
        token_budget: None,
        confirmation_policies: None,
        guardian_v2: None,
    };

    let guardian_config = build_guardian_review_session_config(
        &parent_config,
        /*live_network_config*/ None,
        "active-model",
        /*reasoning_effort*/ None,
        Some(&model_messages),
    )
    .expect("guardian config");

    assert_eq!(
        guardian_config.base_instructions,
        Some(guardian_policy_prompt_with_config_and_template(
            "",
            BUNDLED_GUARDIAN_POLICY_TEMPLATE,
        ))
    );
    assert_ne!(
        guardian_config.base_instructions,
        Some(guardian_policy_prompt_with_config_and_template(
            BUNDLED_GUARDIAN_POLICY,
            BUNDLED_GUARDIAN_POLICY_TEMPLATE,
        ))
    );
}

#[tokio::test]
async fn guardian_review_session_config_preserves_explicit_empty_catalog_template() {
    let parent_config = crate::config::test_config().await;
    let catalog_policy = "Use the catalog Guardian policy.";
    let model_messages = ModelMessages {
        persistent_instructions: None,
        tools: None,
        instructions_template: None,
        instructions_variables: None,
        approvals: None,
        collaboration_modes: None,
        auto_review: Some(AutoReviewMessages {
            policy: Some(catalog_policy.to_string()),
            policy_template: Some(String::new()),
            node_repl_policy: None,
            rejection_instructions: None,
            timeout_instructions: None,
        }),
        permissions: None,
        multi_agent: None,
        token_budget: None,
        confirmation_policies: None,
        guardian_v2: None,
    };

    let guardian_config = build_guardian_review_session_config(
        &parent_config,
        /*live_network_config*/ None,
        "active-model",
        /*reasoning_effort*/ None,
        Some(&model_messages),
    )
    .expect("guardian config");

    assert_eq!(
        guardian_config.base_instructions,
        Some(guardian_policy_prompt_with_config_and_template(
            catalog_policy,
            "",
        ))
    );
    assert_ne!(
        guardian_config.base_instructions,
        Some(guardian_policy_prompt_with_config_and_template(
            catalog_policy,
            BUNDLED_GUARDIAN_POLICY_TEMPLATE,
        ))
    );
}

#[tokio::test(flavor = "current_thread")]
async fn run_before_review_deadline_times_out_before_future_completes() {
    let outcome = run_before_review_deadline(
        tokio::time::Instant::now() + Duration::from_millis(10),
        /*external_cancel*/ None,
        async {
            tokio::time::sleep(Duration::from_millis(50)).await;
        },
    )
    .await;

    assert!(matches!(
        outcome,
        Err(GuardianReviewSessionOutcome::TimedOut)
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn run_before_review_deadline_aborts_when_cancelled() {
    let cancel_token = CancellationToken::new();
    let canceller = cancel_token.clone();
    drop(tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(10)).await;
        canceller.cancel();
    }));

    let outcome = run_before_review_deadline(
        tokio::time::Instant::now() + Duration::from_secs(1),
        Some(&cancel_token),
        std::future::pending::<()>(),
    )
    .await;

    assert!(matches!(
        outcome,
        Err(GuardianReviewSessionOutcome::Aborted)
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn run_before_review_deadline_with_cancel_cancels_token_on_timeout() {
    let cancel_token = CancellationToken::new();

    let outcome = run_before_review_deadline_with_cancel(
        tokio::time::Instant::now() + Duration::from_millis(10),
        /*external_cancel*/ None,
        &cancel_token,
        async {
            tokio::time::sleep(Duration::from_millis(50)).await;
        },
    )
    .await;

    assert!(matches!(
        outcome,
        Err(GuardianReviewSessionOutcome::TimedOut)
    ));
    assert!(cancel_token.is_cancelled());
}

#[tokio::test(flavor = "current_thread")]
async fn run_before_review_deadline_with_cancel_cancels_token_on_abort() {
    let external_cancel = CancellationToken::new();
    let external_canceller = external_cancel.clone();
    let cancel_token = CancellationToken::new();
    drop(tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(10)).await;
        external_canceller.cancel();
    }));

    let outcome = run_before_review_deadline_with_cancel(
        tokio::time::Instant::now() + Duration::from_secs(1),
        Some(&external_cancel),
        &cancel_token,
        std::future::pending::<()>(),
    )
    .await;

    assert!(matches!(
        outcome,
        Err(GuardianReviewSessionOutcome::Aborted)
    ));
    assert!(cancel_token.is_cancelled());
}

#[tokio::test(flavor = "current_thread")]
async fn run_before_review_deadline_with_cancel_preserves_token_on_success() {
    let cancel_token = CancellationToken::new();

    let outcome = run_before_review_deadline_with_cancel(
        tokio::time::Instant::now() + Duration::from_secs(1),
        /*external_cancel*/ None,
        &cancel_token,
        async { 42usize },
    )
    .await;

    assert_eq!(outcome.unwrap(), 42);
    assert!(!cancel_token.is_cancelled());
}

#[test]
fn had_prior_review_context_tracks_prompt_mode() {
    assert!(!had_prior_review_context(&GuardianPromptMode::Full));
    assert!(had_prior_review_context(&GuardianPromptMode::Delta {
        cursor: GuardianTranscriptCursor {
            parent_history_version: 7,
            transcript_entry_count: 42,
        }
    }));
}

#[test]
fn token_usage_delta_never_reports_negative_usage() {
    let start = TokenUsage {
        input_tokens: 10,
        cached_input_tokens: 8,
        cache_write_input_tokens: 8,
        output_tokens: 6,
        reasoning_output_tokens: 4,
        total_tokens: 28,
        codex_rollout_budget_units: None,
    };
    let end = TokenUsage {
        input_tokens: 15,
        cached_input_tokens: 7,
        cache_write_input_tokens: 7,
        output_tokens: 10,
        reasoning_output_tokens: 2,
        total_tokens: 34,
        codex_rollout_budget_units: None,
    };

    assert_eq!(
        token_usage_delta(&start, &end),
        TokenUsage {
            input_tokens: 5,
            cached_input_tokens: 0,
            cache_write_input_tokens: 0,
            output_tokens: 4,
            reasoning_output_tokens: 0,
            total_tokens: 6,
            codex_rollout_budget_units: None,
        }
    );
}

#[tokio::test]
async fn run_review_on_reused_session_waits_for_submitted_turn() {
    let (review_session, tx_event, rx_sub) = test_review_session().await;
    {
        let mut state = review_session.state.lock().await;
        state.prior_review_count = 1;
        state.last_reviewed_transcript_cursor = Some(GuardianTranscriptCursor {
            parent_history_version: 0,
            transcript_entry_count: 0,
        });
    }
    let params = test_review_params().await;

    let review = tokio::spawn(async move {
        run_review_on_session(
            &review_session,
            &params,
            GuardianReviewSessionKind::TrunkReused,
            tokio::time::Instant::now() + Duration::from_secs(1),
        )
        .await
    });
    let submission = rx_sub.recv().await.expect("guardian submission");
    let id = submission.id;
    let Op::TurnInput { reply, .. } = submission.op else {
        panic!("expected turn-input submission");
    };
    reply
        .send(Ok(TurnInputSubmission::Started {
            turn_id: id.clone(),
        }))
        .expect("reply to guardian submission");
    tx_event
        .send(turn_complete_event("prior-turn", Some("stale"), Some(9)))
        .await
        .expect("queue prior turn completion");
    tx_event
        .send(turn_complete_event(id.as_str(), Some("fresh"), Some(42)))
        .await
        .expect("queue submitted turn completion");

    let (outcome, keep_review_session, analytics_result) =
        review.await.expect("review task should complete");
    let GuardianReviewSessionOutcome::Completed(Ok(last_agent_message)) = outcome else {
        panic!("expected submitted turn completion");
    };
    assert_eq!(last_agent_message.as_deref(), Some("fresh"));
    assert_eq!(analytics_result.time_to_first_token_ms, Some(42));
    assert!(keep_review_session);
}

#[tokio::test]
async fn run_review_removes_trunk_when_event_stream_is_broken() {
    let (mut review_session, tx_event, rx_sub) = test_review_session().await;
    let params = test_review_params().await;
    review_session.reuse_key = GuardianReviewSessionReuseKey::from_spawn_config(
        &params.spawn_config,
        params.parent_session.user_instructions().await,
        params
            .parent_session
            .clone_history()
            .await
            .history_version(),
        GuardianContextMode::Legacy,
    )
    .with_environments(params.parent_context.environments())
    .with_node_repl_policy(&params.node_repl_policy);
    let manager = Arc::new(GuardianReviewSessionManager::default());
    prewarm_test_session(&manager, review_session).await;
    let manager_for_review = Arc::clone(&manager);
    let review =
        tokio::spawn(async move { run_guardian_review_session(manager_for_review, params).await });
    let submission = rx_sub.recv().await.expect("guardian submission");
    let id = submission.id;
    let Op::TurnInput { reply, .. } = submission.op else {
        panic!("expected turn-input submission");
    };
    reply
        .send(Ok(TurnInputSubmission::Started { turn_id: id }))
        .expect("reply to guardian submission");
    drop(tx_event);

    let (outcome, _) = review.await.expect("review task should complete");

    assert!(matches!(
        outcome,
        GuardianReviewSessionOutcome::Completed(Err(_))
    ));
    assert!(manager.trunk().await.is_none());
}

#[tokio::test]
async fn wait_for_guardian_review_ignores_prior_turn_completion() {
    let (review_session, tx_event, _rx_sub) = test_review_session().await;
    tx_event
        .send(turn_complete_event("prior-turn", Some("stale"), Some(9)))
        .await
        .expect("queue prior turn completion");
    tx_event
        .send(turn_complete_event("current-turn", Some("fresh"), Some(42)))
        .await
        .expect("queue current turn completion");

    let mut analytics_result = GuardianReviewAnalyticsResult::without_session();
    let (outcome, keep_review_session, capture_token_usage) = wait_for_guardian_review(
        &review_session,
        "current-turn",
        tokio::time::Instant::now() + Duration::from_secs(1),
        /*external_cancel*/ None,
        &mut analytics_result,
    )
    .await;

    let GuardianReviewSessionOutcome::Completed(Ok(last_agent_message)) = outcome else {
        panic!("expected current turn completion");
    };
    assert_eq!(last_agent_message.as_deref(), Some("fresh"));
    assert_eq!(analytics_result.time_to_first_token_ms, Some(42));
    assert!(keep_review_session);
    assert!(capture_token_usage);
}

#[tokio::test]
async fn wait_for_guardian_review_ignores_prior_turn_errors() {
    let (review_session, tx_event, _rx_sub) = test_review_session().await;
    tx_event
        .send(Event {
            id: "prior-turn".to_string(),
            msg: EventMsg::Error(ErrorEvent {
                misalignment: None,
                message: "stale guardian error".to_string(),
                codex_error_info: None,
            }),
        })
        .await
        .expect("queue prior turn error");
    tx_event
        .send(turn_complete_event(
            "current-turn",
            /*last_agent_message*/ None,
            Some(42),
        ))
        .await
        .expect("queue current turn completion");

    let mut analytics_result = GuardianReviewAnalyticsResult::without_session();
    let (outcome, keep_review_session, capture_token_usage) = wait_for_guardian_review(
        &review_session,
        "current-turn",
        tokio::time::Instant::now() + Duration::from_secs(1),
        /*external_cancel*/ None,
        &mut analytics_result,
    )
    .await;

    let GuardianReviewSessionOutcome::Completed(Ok(last_agent_message)) = outcome else {
        panic!("expected current turn completion");
    };
    assert_eq!(last_agent_message, None);
    assert_eq!(analytics_result.time_to_first_token_ms, Some(42));
    assert!(keep_review_session);
    assert!(capture_token_usage);
}

#[tokio::test]
async fn wait_for_guardian_review_preserves_structured_session_error() {
    let (review_session, tx_event, _rx_sub) = test_review_session().await;
    tx_event
        .send(Event {
            id: "current-turn".to_string(),
            msg: EventMsg::Error(ErrorEvent {
                misalignment: None,
                message: "temporary failure".to_string(),
                codex_error_info: Some(CodexErrorInfo::ServerOverloaded),
            }),
        })
        .await
        .expect("queue guardian error");
    tx_event
        .send(turn_complete_event(
            "current-turn",
            /*last_agent_message*/ None,
            Some(42),
        ))
        .await
        .expect("queue current turn completion");

    let mut analytics_result = GuardianReviewAnalyticsResult::without_session();
    let (outcome, keep_review_session, capture_token_usage) = wait_for_guardian_review(
        &review_session,
        "current-turn",
        tokio::time::Instant::now() + Duration::from_secs(1),
        /*external_cancel*/ None,
        &mut analytics_result,
    )
    .await;

    let GuardianReviewSessionOutcome::SessionFailed {
        error, error_info, ..
    } = outcome
    else {
        panic!("expected structured session failure");
    };
    assert_eq!(error.to_string(), "temporary failure");
    assert_eq!(error_info, Some(CodexErrorInfo::ServerOverloaded));
    assert!(keep_review_session);
    assert!(capture_token_usage);
}

#[tokio::test]
async fn wait_for_guardian_review_ignores_prior_turn_aborts() {
    let (review_session, tx_event, _rx_sub) = test_review_session().await;
    tx_event
        .send(turn_aborted_event("prior-turn"))
        .await
        .expect("queue prior turn abort");
    tx_event
        .send(turn_complete_event("current-turn", Some("fresh"), Some(42)))
        .await
        .expect("queue current turn completion");

    let mut analytics_result = GuardianReviewAnalyticsResult::without_session();
    let (outcome, keep_review_session, capture_token_usage) = wait_for_guardian_review(
        &review_session,
        "current-turn",
        tokio::time::Instant::now() + Duration::from_secs(1),
        /*external_cancel*/ None,
        &mut analytics_result,
    )
    .await;

    let GuardianReviewSessionOutcome::Completed(Ok(last_agent_message)) = outcome else {
        panic!("expected current turn completion");
    };
    assert_eq!(last_agent_message.as_deref(), Some("fresh"));
    assert_eq!(analytics_result.time_to_first_token_ms, Some(42));
    assert!(keep_review_session);
    assert!(capture_token_usage);
}

#[tokio::test]
async fn wait_for_guardian_review_timeout_drains_expected_turn_after_stale_terminal_event() {
    let (review_session, tx_event, rx_sub) = test_review_session().await;
    tx_event
        .send(turn_complete_event("prior-turn", Some("stale"), Some(9)))
        .await
        .expect("queue prior turn completion");
    let tx_interrupt_event = tx_event.clone();
    let interrupt_response = tokio::spawn(async move {
        let submission = rx_sub.recv().await.expect("interrupt submission");
        assert!(matches!(submission.op, Op::Interrupt));
        tx_interrupt_event
            .send(turn_aborted_event("current-turn"))
            .await
            .expect("queue current turn abort");
    });

    let mut analytics_result = GuardianReviewAnalyticsResult::without_session();
    let (outcome, keep_review_session, capture_token_usage) = wait_for_guardian_review(
        &review_session,
        "current-turn",
        tokio::time::Instant::now() + Duration::from_millis(10),
        /*external_cancel*/ None,
        &mut analytics_result,
    )
    .await;

    interrupt_response
        .await
        .expect("interrupt response task should complete");
    assert!(matches!(outcome, GuardianReviewSessionOutcome::TimedOut));
    assert!(keep_review_session);
    assert!(!capture_token_usage);
}

#[tokio::test]
async fn wait_for_guardian_review_cancel_drains_expected_turn_after_stale_terminal_event() {
    let (review_session, tx_event, rx_sub) = test_review_session().await;
    tx_event
        .send(turn_complete_event("prior-turn", Some("stale"), Some(9)))
        .await
        .expect("queue prior turn completion");
    let tx_interrupt_event = tx_event.clone();
    let interrupt_response = tokio::spawn(async move {
        let submission = rx_sub.recv().await.expect("interrupt submission");
        assert!(matches!(submission.op, Op::Interrupt));
        tx_interrupt_event
            .send(turn_aborted_event("current-turn"))
            .await
            .expect("queue current turn abort");
    });
    let external_cancel = CancellationToken::new();
    external_cancel.cancel();

    let mut analytics_result = GuardianReviewAnalyticsResult::without_session();
    let (outcome, keep_review_session, capture_token_usage) = wait_for_guardian_review(
        &review_session,
        "current-turn",
        tokio::time::Instant::now() + Duration::from_secs(1),
        Some(&external_cancel),
        &mut analytics_result,
    )
    .await;

    interrupt_response
        .await
        .expect("interrupt response task should complete");
    assert!(matches!(outcome, GuardianReviewSessionOutcome::Aborted));
    assert!(keep_review_session);
    assert!(!capture_token_usage);
}

#[tokio::test]
async fn interrupt_and_drain_turn_ignores_prior_turn_completion() {
    let (review_session, tx_event, _rx_sub) = test_review_session().await;
    tx_event
        .send(turn_complete_event("prior-turn", Some("stale"), Some(9)))
        .await
        .expect("queue prior turn completion");
    tx_event
        .send(turn_aborted_event("current-turn"))
        .await
        .expect("queue current turn abort");

    interrupt_and_drain_turn(&review_session, "current-turn")
        .await
        .expect("drain current turn");

    assert!(review_session.io.rx_event.try_recv().is_err());
}

// Reuse the existing in-memory reviewer fixture through the production prewarm path.
async fn prewarm_test_session(pool: &GuardianReviewSessionManager, session: GuardianReviewSession) {
    struct ReadySession {
        context: GuardianReviewSessionReuseKey,
        session: Mutex<Option<GuardianReviewSession>>,
    }
    impl codex_guardian_reviewer::ReviewerSessionFactory for ReadySession {
        type Session = GuardianReviewSession;
        fn context(
            &self,
            _previous: Option<&GuardianReviewSession>,
        ) -> GuardianReviewSessionReuseKey {
            self.context.clone()
        }
        async fn spawn(
            &self,
            _context: GuardianReviewSessionReuseKey,
            _kind: GuardianReviewSessionKind,
            _snapshot: Option<GuardianReviewForkSnapshot>,
            _cancellation: CancellationToken,
        ) -> anyhow::Result<GuardianReviewSession> {
            Ok(self.session.lock().await.take().expect("one fixture spawn"))
        }
    }
    let factory = ReadySession {
        context: session.reuse_key.clone(),
        session: Mutex::new(Some(session)),
    };
    pool.prewarm(&factory).await.unwrap();
}
