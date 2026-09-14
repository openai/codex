//! Prepares opaque session inputs for the extension-owned reviewer pool.
//! Context selection and assembly stay on the existing path pending its replacement.

use super::*;
use codex_guardian_reviewer::ReviewerPool;
use codex_guardian_reviewer::ReviewerRequest;
use codex_guardian_reviewer::SessionDisposition;

pub(crate) struct PreparedGuardianContext {
    parent: Arc<Session>,
    context: GuardianReviewContext,
    config: Config,
    context_policy: ReviewContextPolicy,
    key: GuardianReviewSessionReuseKey,
    parent_compaction: Option<ResponseItem>,
    host: Arc<GuardianReviewSessionHost>,
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
        let host = parent
            .services
            .thread_extension_data
            .get_or_init(GuardianReviewSessionHost::default);
        Ok(Self {
            parent,
            context,
            config,
            context_policy,
            key,
            parent_compaction,
            host,
        })
    }
}

impl PreparedGuardianContext {
    fn reuse_key(&self, previous: Option<&GuardianReviewSession>) -> GuardianReviewSessionReuseKey {
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

    /// Returns captured parent inputs; Guardian selects the agent's identity and lifecycle.
    async fn thread_options(
        &self,
        initial_history: Option<InitialHistory>,
    ) -> crate::StartThreadOptions {
        let initial_history = initial_history.or_else(|| {
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
        crate::StartThreadOptions {
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
        }
    }

    /// Binds context bookkeeping to an agent that Guardian has already started.
    async fn bind_session(
        &self,
        session: Arc<Session>,
        io: SessionIo,
        context: GuardianReviewSessionReuseKey,
        state: GuardianReviewState,
        cancellation: CancellationToken,
    ) -> GuardianReviewSession {
        let inherited = session.inherited_instructions().await;
        let context = GuardianReviewSessionReuseKey {
            user_instructions: inherited.user,
            thread_instructions: inherited.thread,
            ..context
        };
        GuardianReviewSession {
            session,
            io,
            cancel_token: cancellation,
            reuse_key: context,
            state: Mutex::new(state),
        }
    }
}

impl PreparedGuardianContext {
    pub(crate) async fn spawn(
        &self,
        context: GuardianReviewSessionReuseKey,
        kind: GuardianReviewSessionKind,
        snapshot: Option<GuardianReviewForkSnapshot>,
        cancellation: CancellationToken,
    ) -> anyhow::Result<GuardianReviewSession> {
        let (conversation, history) = snapshot.map(ConversationState::fork).unzip();
        let state = GuardianReviewState {
            conversation: conversation.unwrap_or_default(),
            last_admitted_node_repl_response_sequence: history.as_ref().map_or(0, |history| {
                history.last_admitted_node_repl_response_sequence
            }),
            pending_node_repl_evidence_admission: None,
        };
        let mut options = self
            .thread_options(history.map(|history| history.initial_history))
            .await;
        if matches!(kind, GuardianReviewSessionKind::EphemeralForked) {
            options.config.ephemeral = true;
        }
        let threads = self.host.managed_threads.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Guardian extension is not installed for this thread")
        })?;
        let (session, io) = threads
            .spawn(&self.parent, options, cancellation.clone())
            .await?;
        Ok(self
            .bind_session(session, io, context, state, cancellation)
            .await)
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

pub(crate) fn prewarm_guardian_review_session(
    parent: Arc<Session>,
    turn: Arc<TurnContext>,
) -> BoxFuture<'static, anyhow::Result<()>> {
    Box::pin(async move {
        let context = prepare_prewarm(Arc::clone(&parent), turn).await?;
        let key = context.reuse_key(/*previous*/ None);
        parent
            .guardian_review_session()
            .prewarm(Arc::new(context), key)
            .await
    })
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
