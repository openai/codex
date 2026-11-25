//! A retained verdict conversation binds one actor’s live board tools to an isolated reviewer thread.
use std::sync::Arc;

use crate::LiveRetrievalBinding;
use crate::inbox::ActorInbox;
use codex_analytics::GuardianReviewSessionKind;
use codex_core::CodexThread;
use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_core::guardian_review::GuardianReviewSession;
use codex_core::guardian_review::PreparedGuardianContext;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::IsolatedSessionExtensions;
use codex_extension_api::SessionIsolation;
use codex_extension_api::ToolName;
use codex_extension_api::ToolPolicy;
use codex_guardian_reviewer::ReviewerTasks;
use codex_message_board::AgentMessageBoard;
use codex_protocol::ThreadId;
use codex_protocol::protocol::InternalSessionSource;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::TruncationPolicy;
use tokio_util::sync::CancellationToken;
use tokio_util::sync::DropGuard;

pub(super) struct Conversation {
    // A prewarmed thread is still new until its first reusable review.
    pub(super) kind: GuardianReviewSessionKind,
    pub(super) thread: Arc<CodexThread>,
    _stop: DropGuard,
    pub(super) inbox: Arc<ActorInbox>,
    delivered: usize,
    policy: TruncationPolicy,
    pub(super) runtime: GuardianReviewSession,
}

impl Conversation {
    /// Record only the prefix observed when this drain began. Later arrivals can wait for the next drain.
    pub(super) async fn deliver(
        &mut self,
        actor: ThreadId,
        board: &dyn AgentMessageBoard,
    ) -> anyhow::Result<()> {
        let through = self.inbox.len();
        if self.delivered >= through {
            return Ok(());
        }
        let thread = &self.thread;
        self.inbox
            .deliver(
                actor,
                board,
                &mut self.delivered,
                through,
                self.policy,
                |items| async move { Ok(thread.inject_response_items(items).await?) },
            )
            .await
    }

    pub(super) async fn start(
        manager: &Arc<ThreadManager>,
        prepared: &PreparedGuardianContext,
        owner: &ReviewerTasks,
        binding: LiveRetrievalBinding,
        stop: CancellationToken,
        shadow: bool,
    ) -> anyhow::Result<Self> {
        let key = prepared.reuse_key(/*previous*/ None);
        let (mut options, state) = prepared
            .thread_options(GuardianReviewSessionKind::TrunkNew, /*snapshot*/ None)
            .await;
        if shadow {
            options.config.ephemeral = true;
            options.config.feedback_enabled = false;
            options
                .thread_extension_init
                .insert(codex_core::guardian_review::ShadowReviewSession);
        }
        options.initial_history = codex_history::InitialHistory::New;
        let mut registry = ExtensionRegistryBuilder::<Config>::default();
        crate::install_retrieval_tools(&mut registry);
        options
            .thread_extension_init
            .insert(IsolatedSessionExtensions(Arc::new(registry.build())));
        // Code Mode dispatches nested calls through this same board-only registry.
        let allowed = LiveRetrievalBinding::tool_names()
            .chain(["exec", "wait"].map(ToolName::plain))
            .collect();
        options.thread_source = Some(codex_protocol::protocol::ThreadSource::GuardianReview);
        options.client_mcp_extensions = Default::default();
        options.session_source = Some(SessionSource::Internal(InternalSessionSource::Guardian));
        options
            .thread_extension_init
            .insert(SessionIsolation::Isolated);
        let inbox = Arc::clone(&binding.inbox);
        options.thread_extension_init.insert(binding);
        options.thread_extension_init.insert(ToolPolicy {
            allowed_tools: Some(allowed),
            ..Default::default()
        });
        options
            .thread_extension_init
            .insert(codex_core::guardian_review::reviewer_headers(
                &options.config,
            ));
        let stop_guard = stop.clone().drop_guard();
        let until = stop.clone();
        let reset = prepared.history_reset.clone();
        let spawned = manager
            .start_thread_until(
                options,
                async move {
                    tokio::select! {
                        _ = until.cancelled() => (),
                        _ = reset.cancelled() => until.cancel(),
                    }
                },
                &owner.tasks,
            )
            .await?;
        let policy = spawned
            .thread
            .selected_model_info()
            .await
            .truncation_policy
            .into();
        let runtime = prepared
            .bind_thread(&spawned.thread, key, state, stop)
            .await;
        Ok(Self {
            kind: GuardianReviewSessionKind::TrunkNew,
            thread: spawned.thread,
            _stop: stop_guard,
            delivered: 0,
            inbox,
            policy,
            runtime,
        })
    }
}
