//! Captures the context used by Guardian before and after it starts a reviewer agent.
//! Context selection and assembly stay on the existing path pending its replacement.

use super::*;
use codex_guardian_reviewer::ReviewerPool;
use codex_guardian_reviewer::ReviewerRequest;
use codex_guardian_reviewer::SessionDisposition;

pub struct PreparedGuardianContext {
    parent: Arc<Session>,
    context: GuardianReviewContext,
    config: Config,
    context_policy: ReviewContextPolicy,
    key: GuardianReviewSessionReuseKey,
    parent_compaction: Option<ResponseItem>,
    pub history_reset: CancellationToken,
}

impl PreparedGuardianContext {
    async fn prepare(
        parent: Arc<Session>,
        context: GuardianReviewContext,
        config: Config,
        history: &ContextManager,
        node_repl_policy: &GuardianNodeReplPolicy,
        compaction_model_hash: Option<&str>,
    ) -> anyhow::Result<Self> {
        let (reset_version, history_reset) = parent.history_reset().await;
        // Preparation may have raced with a history reset before capturing the reviewer context.
        if reset_version != history.reset_version {
            history_reset.cancel();
        }
        let context_policy =
            ReviewContextPolicy::for_context(parent.guardian_context_mode, &config.features);
        let root_authorization_version = context_policy.root_authorization_version(&parent).await;
        let parent_compaction = context_policy.parent_compaction(history, compaction_model_hash)?;
        let mut key = GuardianReviewSessionReuseKey::from_spawn_config(
            &config,
            parent.inherited_instructions().await,
            history.history_version(),
            parent.guardian_context_mode,
        )
        .with_environments(context.environments())
        .with_node_repl_policy_eligibility(context.model_info.computer_use_review_required())
        .with_node_repl_policy(node_repl_policy);
        key.root_authorization_version = root_authorization_version;
        key.parent_reset_version = history.reset_version;
        Ok(Self {
            parent,
            context,
            config,
            context_policy,
            key,
            parent_compaction,
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

    /// Moves fork history into startup options and retains only its context bookkeeping.
    /// Guardian selects the agent's identity and lifecycle.
    pub async fn thread_options(
        &self,
        snapshot: Option<GuardianReviewForkSnapshot>,
    ) -> (crate::StartThreadOptions, GuardianReviewState) {
        let (conversation, history) = snapshot.map(ConversationState::fork).unzip();
        let state = GuardianReviewState {
            conversation: conversation.unwrap_or_default(),
            last_admitted_node_repl_response_sequence: history.as_ref().map_or(0, |history| {
                history.last_admitted_node_repl_response_sequence
            }),
            pending_node_repl_evidence_admission: None,
        };
        let initial_history = history.map(|history| history.initial_history).or_else(|| {
            self.parent_compaction
                .clone()
                .map(|item| InitialHistory::Forked(vec![RolloutItem::ResponseItem(item.into())]))
        });
        let mut config = self.config.clone();
        config.model_provider.supports_websockets &= self
            .parent
            .services
            .model_client
            .responses_websocket_enabled();
        let options = crate::StartThreadOptions {
            internal_parent: Some(crate::thread_manager::InternalSessionParent {
                thread_id: self.parent.thread_id(),
                auth_manager: Arc::clone(&self.parent.services.auth_manager),
                agent_control: self.parent.services.agent_control.clone(),
                originator: self.context.turn().originator.clone(),
                inherited_instructions: Some(self.parent.inherited_instructions().await),
            }),
            initial_history: initial_history.unwrap_or(InitialHistory::New),
            environments: Some(self.context.environments().to_selections()),
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
        state: GuardianReviewState,
        cancellation: CancellationToken,
    ) -> GuardianReviewSession {
        let session = Arc::clone(&thread.session);
        let io = SessionIo {
            tx_sub: thread.io.tx_sub.clone(),
            rx_event: thread.io.rx_event.clone(),
            agent_status: thread.io.agent_status.clone(),
            session_loop_termination: thread.io.session_loop_termination.clone(),
        };
        let inherited = session.inherited_instructions().await;
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

pub(super) struct PreparedReview {
    context: Arc<PreparedGuardianContext>,
    params: GuardianReviewSessionParams,
}

impl ReviewerRequest for PreparedReview {
    type Session = GuardianReviewSession;

    fn setup(&self) -> Arc<PreparedGuardianContext> {
        Arc::clone(&self.context)
    }
    fn context(&self, previous: Option<&GuardianReviewSession>) -> GuardianReviewSessionReuseKey {
        self.context.reuse_key(previous)
    }
    fn deadline(&self) -> tokio::time::Instant {
        self.params.deadline
    }
    fn cancellation(&self) -> Option<&CancellationToken> {
        self.params.external_cancel.as_ref()
    }

    async fn run(
        &self,
        session: &GuardianReviewSession,
        kind: GuardianReviewSessionKind,
    ) -> (
        GuardianReviewSessionOutcome,
        SessionDisposition,
        GuardianReviewAnalyticsResult,
    ) {
        let (outcome, keep_session, analytics) = Box::pin(run_review_on_session(
            session,
            &self.params,
            kind,
            self.params.deadline,
        ))
        .await;
        record_failed_review(&session.session, &self.params, &outcome).await;
        let disposition = if keep_session {
            SessionDisposition::Reusable
        } else {
            SessionDisposition::Discard
        };
        (outcome, disposition, analytics)
    }
}

pub(crate) async fn run_guardian_review_session(
    pool: Arc<ReviewerPool<GuardianReviewSession>>,
    params: GuardianReviewSessionParams,
) -> (GuardianReviewSessionOutcome, GuardianReviewAnalyticsResult) {
    match prepare_review(params).await {
        Ok(prepared) => pool.review(prepared).await,
        Err(error) => (
            GuardianReviewSessionOutcome::PromptBuildFailed(error),
            GuardianReviewAnalyticsResult::without_session(),
        ),
    }
}

pub(super) async fn prepare_review(
    params: GuardianReviewSessionParams,
) -> anyhow::Result<PreparedReview> {
    let context = PreparedGuardianContext::prepare(
        Arc::clone(&params.parent_session),
        params.parent_context.clone(),
        params.spawn_config.clone(),
        &params.parent_history,
        &params.node_repl_policy,
        params.compaction_model_hash.as_deref(),
    )
    .await?;
    Ok(PreparedReview {
        context: Arc::new(context),
        params,
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
            config.compaction_model_hash.as_deref(),
        )
        .await
    })
}
