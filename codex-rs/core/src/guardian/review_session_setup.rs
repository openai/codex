//! Captures action-time parent context and starts or recovers its Guardian reviewer.
//! Checkpoint recovery allows one fresh attempt under the original review deadline.
//! Both attempts share captured context and use separate recovery flags.
//! Checkpoints are captured before final authorization validation and published without yielding.
//! Recorded reviewer items and interruptions are forwarded to the parent's raw-event subscription.

use codex_guardian_reviewer::ReviewDeadline;
use std::sync::atomic::AtomicBool;

use super::*;
use crate::guardian::input_budget::CheckpointRecovery;
use codex_features::Feature;
use codex_guardian_reviewer::ReviewerPool;
use codex_guardian_reviewer::ReviewerRequest;
use codex_guardian_reviewer::ReviewerSelection;
use codex_protocol::protocol::TurnEnvironmentSelection;

#[derive(Clone)]
pub struct PreparedGuardianContext {
    parent: Arc<Session>,
    context: GuardianReviewContext,
    config: Config,
    context_policy: ReviewContextPolicy,
    key: GuardianReviewSessionReuseKey,
    parent_compaction: Option<ResponseItem>,
    reviewer_selection: ReviewerSelection,
    pub history_reset: CancellationToken,
}

impl PreparedGuardianContext {
    async fn prepare(
        parent: Arc<Session>,
        mut context: GuardianReviewContext,
        config: Config,
        history: &ContextManager,
        node_repl_policy: &GuardianNodeReplPolicy,
    ) -> anyhow::Result<Self> {
        let (reset_version, history_reset) = parent.history_reset().await;
        // Preparation may have raced with a history reset before capturing the reviewer context.
        if reset_version != history.reset_version {
            history_reset.cancel();
        }
        let context_mode =
            GuardianContextMode::from_history(history.conversation_history_snapshot().as_ref());
        let context_policy = ReviewContextPolicy::for_context(context_mode, &config.features);
        let root_review_version =
            context_policy::root_review_version(context_policy, &parent).await;
        let parent_compaction = context_policy.parent_compaction(history.annotated_items())?;
        let mut instructions = parent.inherited_instructions().await;
        context.capture_project_instructions(&parent).await;
        instructions.project_snapshot = context.project_instructions.clone();
        let mut key = GuardianReviewSessionReuseKey::from_spawn_config(
            &config,
            instructions,
            history.history_version(),
            context_mode,
        )
        .with_environments(context.environments())
        .with_node_repl_policy_eligibility(context.model_info.computer_use_review_required())
        .with_node_repl_policy(node_repl_policy);
        key.root_review_version = root_review_version;
        key.parent_reset_version = history.reset_version;
        Ok(Self {
            parent,
            context,
            config,
            context_policy,
            key,
            parent_compaction,
            reviewer_selection: ReviewerSelection::ReuseIfAvailable,
            history_reset,
        })
    }
}

impl PreparedGuardianContext {
    pub fn reuse_key(
        &self,
        previous: Option<&GuardianReviewSession>,
    ) -> GuardianReviewSessionReuseKey {
        let mut key = self.key.clone();
        if self.context_policy != ReviewContextPolicy::ThreadOwned
            && self.parent_compaction.is_none()
            && let Some(previous) = previous
        {
            // Without a decryptable summary, the existing reviewer may hold the
            // only remaining authorization or restriction from parent history.
            key.parent_history_version = previous.reuse_key.parent_history_version;
        }
        key
    }

    /// Moves fork history into startup options; only reusable reviewers retain a rollback checkpoint.
    /// Guardian selects the agent's identity and lifecycle.
    pub async fn thread_options(
        &self,
        kind: GuardianReviewSessionKind,
        snapshot: Option<GuardianReviewForkSnapshot>,
    ) -> (crate::StartThreadOptions, GuardianReviewState) {
        let (state, initial_history) = GuardianReviewState::for_session(
            kind,
            snapshot,
            self.reviewer_selection,
            self.parent_compaction.as_ref(),
        );
        let mut config = self.config.clone();
        config.model_provider.supports_websockets &= self
            .parent
            .services
            .model_client
            .responses_websocket_enabled();
        let options = crate::StartThreadOptions {
            prompt_cache_key: self.parent.services.model_client.prompt_cache_key_policy(),
            history_mode: Some(codex_protocol::protocol::ThreadHistoryMode::Paginated),
            internal_parent: Some(crate::thread_manager::InternalSessionParent {
                thread_id: self.parent.thread_id(),
                auth_manager: Arc::clone(&self.parent.services.auth_manager),
                agent_control: crate::agent::control::AgentControlInit::Provided {
                    control: Arc::clone(&self.parent.services.agent_control),
                    runtime: self.parent.services.local_agent_runtime.clone(),
                },
                originator: self.context.turn().originator.clone(),
                // Review the same applied instructions captured by the reuse key.
                // A live provider could advance independently while reviewing this action.
                inherited_instructions: Some(SessionInstructions {
                    user: self.key.user_instructions.clone(),
                    thread: self.key.thread_instructions.clone(),
                    project_snapshot: self.key.project_instructions.clone(),
                    ..Default::default()
                }),
            }),
            initial_history: initial_history.unwrap_or(InitialHistory::New),
            environments: Some(
                self.context
                    .environments()
                    .to_selections()
                    .into_iter()
                    .map(TurnEnvironmentSelection::into_request)
                    .collect(),
            ),
            inherited_environments: Some(self.context.environments().clone()),
            client_mcp_extensions: self.parent.services.client_mcp_extensions.clone(),
            ..crate::StartThreadOptions::new(config)
        };
        (options, state)
    }

    /// Binds context bookkeeping to an agent that Guardian has already started.
    pub async fn bind_thread(
        &self,
        thread: &crate::CodexThread,
        context: GuardianReviewSessionReuseKey,
        mut state: GuardianReviewState,
        cancellation: CancellationToken,
    ) -> GuardianReviewSession {
        let session = Arc::clone(&thread.session);
        if self.parent.enabled(Feature::GuardianRecordedItems) {
            session
                .parent_observer_event_tx
                .get_or_init(|| self.parent.get_tx_event());
        }
        let io = SessionIo {
            tx_sub: thread.io.tx_sub.clone(),
            rx_event: thread.io.rx_event.clone(),
            agent_status: thread.io.agent_status.clone(),
            session_loop_termination: thread.io.session_loop_termination.clone(),
        };
        let inherited = session.inherited_instructions().await;
        {
            let history = session.clone_history().await;
            state.transcript_history_version = history.history_version();
            state.fresh_parent_checkpoint &= self
                .parent_compaction
                .as_ref()
                .zip(codex_history::CompactionCheckpoint::latest(
                    history.annotated_items(),
                ))
                .is_some_and(|(parent, loaded)| parent == loaded.item);
        }
        let context = GuardianReviewSessionReuseKey {
            user_instructions: inherited.user,
            thread_instructions: inherited.thread,
            ..context
        };
        crate::session::emit_subagent_session_started(
            &self.parent.services.analytics_events_client,
            self.parent.app_server_client_metadata().await,
            session.session_id(),
            session.thread_id(),
            Some(self.parent.thread_id()),
            session.thread_config_snapshot().await,
            SubAgentSource::Other(GUARDIAN_REVIEWER_NAME.to_owned()),
            /*resumed_created_at*/ None,
        );
        GuardianReviewSession {
            session,
            io,
            cancel_token: cancellation,
            reuse_key: context,
            state: Mutex::new(state),
        }
    }
}

#[derive(Clone)]
pub(in crate::guardian) struct PreparedReview {
    context: Arc<PreparedGuardianContext>,
    params: Arc<GuardianReviewSessionParams>,
    recovery_requested: Arc<AtomicBool>,
}

impl ReviewerRequest for PreparedReview {
    type Session = GuardianReviewSession;

    fn setup(&self) -> Arc<PreparedGuardianContext> {
        Arc::clone(&self.context)
    }
    fn context(&self, previous: Option<&GuardianReviewSession>) -> GuardianReviewSessionReuseKey {
        self.context.reuse_key(previous)
    }
    fn requires_fresh_session(&self) -> bool {
        self.context.reviewer_selection == ReviewerSelection::FreshParentCheckpoint
    }
    fn deadline(&self) -> ReviewDeadline {
        self.params.deadline
    }
    fn cancellation(&self) -> Option<&CancellationToken> {
        self.params.external_cancel.as_ref()
    }

    async fn run(
        &self,
        session: &GuardianReviewSession,
        kind: GuardianReviewSessionKind,
    ) -> ReviewSessionResult {
        // A prewarmed session can already be fresh on the initial attempt. Consume
        // this before running: even a failed review can leave new input in history.
        let fresh_parent_checkpoint =
            std::mem::take(&mut session.state.lock().await.fresh_parent_checkpoint);
        if self.context.parent_compaction.is_some() {
            session
                .session
                .services
                .thread_extension_data
                .insert(CheckpointRecovery {
                    requested: Arc::clone(&self.recovery_requested),
                    history_version: session.session.clone_history().await.history_version(),
                    fresh_parent_checkpoint,
                });
        } else {
            session
                .session
                .services
                .thread_extension_data
                .remove::<CheckpointRecovery>();
        }
        let mut result = Box::pin(run_review_on_session(
            session,
            &self.params,
            kind,
            self.params.deadline,
        ))
        .await;
        let recovery_requested = self.recovery_requested.load(Ordering::Acquire);
        if recovery_requested {
            result.disposition = SessionDisposition::Discard;
        }
        if self.context.reviewer_selection == ReviewerSelection::FreshParentCheckpoint
            || !recovery_requested
        {
            record_failed_review(&session.session, &self.params, &result.outcome).await;
        }
        let checkpoint = if result.disposition == SessionDisposition::Reusable
            && matches!(result.outcome, GuardianReviewSessionOutcome::Completed(_))
            && !matches!(kind, GuardianReviewSessionKind::EphemeralForked)
        {
            let items = session.session.guardian_fork_history().await;
            let history_version = session.session.clone_history().await.history_version();
            Some((items, history_version, session.state.lock().await))
        } else {
            None
        };
        // All checkpoint capture and lock acquisition must precede validation. No await
        // may follow a valid allow before publication; new guidance during capture must
        // invalidate the attempt without overwriting the last committed review.
        if let GuardianReviewSessionOutcome::Completed(Ok(Some(message))) = &result.outcome
            && codex_guardian_reviewer::parse_guardian_assessment(message).is_ok_and(|assessment| {
                assessment.outcome == super::super::GuardianAssessmentOutcome::Allow
            })
            && let Some(authorization) = &self.params.authorization
            && let Some(invalidation) = authorization
                .invalidation(
                    &self.params.parent_session,
                    self.params.external_cancel.as_ref(),
                )
                .await
        {
            let outcome = invalidation.outcome;
            result.analytics.context_changed |= invalidation.context_changed;
            result.disposition =
                if matches!(outcome, GuardianReviewSessionOutcome::StaleAuthorization)
                    && result.disposition == SessionDisposition::Reusable
                {
                    SessionDisposition::Rollback
                } else {
                    SessionDisposition::Discard
                };
            result.outcome = outcome;
            return result;
        }
        if let Some((items, history_version, mut state)) = checkpoint {
            state.commit_snapshot(
                items,
                history_version,
                session
                    .session
                    .services
                    .thread_extension_data
                    .get::<CheckpointRecovery>()
                    .as_deref(),
            );
        }
        result
    }
}

pub(crate) async fn run_guardian_review_session(
    pool: Arc<ReviewerPool<GuardianReviewSession>>,
    params: GuardianReviewSessionParams,
) -> (GuardianReviewSessionOutcome, GuardianReviewAnalyticsResult) {
    let context_mode = GuardianContextMode::from_history(
        params
            .parent_history
            .conversation_history_snapshot()
            .as_ref(),
    );
    let (outcome, mut analytics) = match prepare_review(params).await {
        Ok(mut prepared) => {
            let result = pool.review(prepared.clone()).await;
            if prepared.recovery_requested.load(Ordering::Acquire) {
                // One restart only, under the original deadline. An oversized parent
                // checkpoint must fail rather than repeatedly compact and recreate.
                Arc::make_mut(&mut prepared.context).reviewer_selection =
                    ReviewerSelection::FreshParentCheckpoint;
                prepared.recovery_requested = Arc::default();
                let (outcome, mut analytics) = pool.review(prepared).await;
                analytics.context_changed |= result.1.context_changed;
                (outcome, analytics)
            } else {
                result
            }
        }
        Err(error) => (
            GuardianReviewSessionOutcome::PromptBuildFailed(error),
            GuardianReviewAnalyticsResult::without_session(),
        ),
    };
    // Keep the captured mode even when preparation or reviewer startup fails.
    analytics.guardian_context_mode = Some(context_mode.as_str());
    (outcome, analytics)
}

pub(in crate::guardian) async fn prepare_review(
    params: GuardianReviewSessionParams,
) -> anyhow::Result<PreparedReview> {
    let context = PreparedGuardianContext::prepare(
        Arc::clone(&params.parent_session),
        params.parent_context.clone(),
        params.spawn_config.clone(),
        &params.parent_history,
        &params.node_repl_policy,
    )
    .await?;
    Ok(PreparedReview {
        context: Arc::new(context),
        params: Arc::new(params),
        recovery_requested: Arc::default(),
    })
}

/// Captures the same startup context used by the existing prompt builder.
/// The caller owns scheduling, cancellation, and the reviewer pool.
pub async fn prepare_review_prewarm(
    parent: &crate::CodexThread,
) -> anyhow::Result<PreparedGuardianContext> {
    let turn = parent
        .session
        .new_startup_prewarm_turn_with_sub_id(crate::session::INITIAL_SUBMIT_ID.to_owned())
        .await;
    prepare_prewarm(Arc::clone(&parent.session), turn).await
}

pub(super) fn prepare_prewarm(
    parent: Arc<Session>,
    turn: Arc<TurnContext>,
) -> BoxFuture<'static, anyhow::Result<PreparedGuardianContext>> {
    Box::pin(async move {
        let context = GuardianReviewContext::from(turn);
        let config = guardian_review_session_config(&parent, &context).await?;
        let history = parent.clone_history().await;
        PreparedGuardianContext::prepare(
            Arc::clone(&parent),
            context,
            config.spawn_config,
            &history,
            &config.node_repl_policy,
        )
        .await
    })
}
