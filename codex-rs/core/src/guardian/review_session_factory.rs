//! Prepares opaque session inputs for the extension-owned reviewer pool.
//! Context selection and assembly stay on the existing path pending its replacement.

use super::*;
use codex_guardian_reviewer::ReviewerPool;
use codex_guardian_reviewer::ReviewerRequest;
use codex_guardian_reviewer::ReviewerSessionFactory;
use codex_guardian_reviewer::SessionDisposition;

pub(super) struct PreparedSession {
    parent: Arc<Session>,
    context: GuardianReviewContext,
    config: Config,
    context_policy: ReviewContextPolicy,
    key: GuardianReviewSessionReuseKey,
    parent_compaction: Option<ResponseItem>,
    host: Arc<GuardianReviewSessionHost>,
}

impl PreparedSession {
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

impl ReviewerSessionFactory for PreparedSession {
    type Session = GuardianReviewSession;

    fn context(&self, previous: Option<&GuardianReviewSession>) -> GuardianReviewSessionReuseKey {
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

    async fn spawn(
        &self,
        context: GuardianReviewSessionReuseKey,
        kind: GuardianReviewSessionKind,
        snapshot: Option<GuardianReviewForkSnapshot>,
        cancellation: CancellationToken,
    ) -> anyhow::Result<GuardianReviewSession> {
        let mut config = self.config.clone();
        if matches!(kind, GuardianReviewSessionKind::EphemeralForked) {
            config.ephemeral = true;
        }
        let (
            initial_history,
            prior_review_count,
            initial_transcript_cursor,
            last_admitted_node_repl_response_sequence,
        ) = match snapshot {
            Some(snapshot) => (
                Some(snapshot.initial_history),
                snapshot.prior_review_count,
                snapshot.last_reviewed_transcript_cursor,
                snapshot.last_admitted_node_repl_response_sequence,
            ),
            None => (
                self.parent_compaction.clone().map(|item| {
                    InitialHistory::Forked(vec![RolloutItem::ResponseItem(item.into())])
                }),
                0,
                None,
                0,
            ),
        };
        let (session, io) = match &self.host.managed_threads {
            Some(threads) => {
                threads
                    .spawn(
                        &self.parent,
                        &self.context,
                        config,
                        cancellation.clone(),
                        initial_history,
                    )
                    .await?
            }
            None => {
                Box::pin(run_codex_thread_interactive(
                    config,
                    Arc::clone(&self.parent.services.auth_manager),
                    self.parent.services.models_manager.clone(),
                    Arc::clone(&self.parent),
                    Arc::clone(self.context.turn()),
                    self.context.environments().clone(),
                    cancellation.clone(),
                    SubAgentSource::Other(GUARDIAN_REVIEWER_NAME.to_owned()),
                    codex_extension_api::SessionIsolation::Isolated,
                    initial_history,
                    GitEnrichmentPolicy::Skip,
                    codex_sandboxing::WindowsSandboxProxySettingsMode::Preserve,
                ))
                .await?
            }
        };
        let inherited = session.inherited_instructions().await;
        let context = GuardianReviewSessionReuseKey {
            user_instructions: inherited.user,
            thread_instructions: inherited.thread,
            ..context
        };
        Ok(GuardianReviewSession {
            session,
            io,
            cancel_token: cancellation,
            reuse_key: context,
            state: Mutex::new(GuardianReviewState {
                prior_review_count,
                last_reviewed_transcript_cursor: initial_transcript_cursor,
                last_admitted_node_repl_response_sequence,
                pending_node_repl_evidence_admission: None,
                last_committed_fork_snapshot: None,
            }),
        })
    }
}

pub(super) struct PreparedReview {
    factory: PreparedSession,
    params: GuardianReviewSessionParams,
}

impl ReviewerRequest for PreparedReview {
    type Factory = PreparedSession;

    fn factory(&self) -> &PreparedSession {
        &self.factory
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
    let factory = PreparedSession::prepare(
        Arc::clone(&params.parent_session),
        params.parent_context.clone(),
        params.spawn_config.clone(),
        &params.parent_history,
        &params.node_repl_policy,
        params.compaction_model_hash.as_deref(),
    )
    .await?;
    Ok(PreparedReview { factory, params })
}

pub(crate) fn prewarm_guardian_review_session(
    parent: Arc<Session>,
    turn: Arc<TurnContext>,
) -> BoxFuture<'static, anyhow::Result<()>> {
    // Keep the Session -> Guardian -> Session startup future on the heap.
    Box::pin(async move {
        let context = GuardianReviewContext::from(turn);
        let config = guardian_review_session_config(&parent, &context).await?;
        let history = parent.clone_history().await;
        let factory = PreparedSession::prepare(
            Arc::clone(&parent),
            context,
            config.spawn_config,
            &history,
            &config.node_repl_policy,
            config.compaction_model_hash.as_deref(),
        )
        .await?;
        parent.guardian_review_session().prewarm(&factory).await
    })
}
