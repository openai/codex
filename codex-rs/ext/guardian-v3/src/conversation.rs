//! A retained verdict conversation binds one actor’s live board tools to an isolated reviewer thread.
use std::sync::Arc;

use crate::LiveRetrievalBinding;
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
use codex_protocol::protocol::InternalSessionSource;
use codex_protocol::protocol::SessionSource;
use tokio_util::sync::DropGuard;

pub(super) struct Conversation {
    pub(super) thread: Arc<CodexThread>,
    _stop: DropGuard,
    pub(super) runtime: GuardianReviewSession,
}

impl Conversation {
    pub(super) async fn start(
        manager: &Arc<ThreadManager>,
        prepared: &PreparedGuardianContext,
        owner: &ReviewerTasks,
        binding: LiveRetrievalBinding,
    ) -> anyhow::Result<Self> {
        let key = prepared.reuse_key(/*previous*/ None);
        let (mut options, state) = prepared
            .thread_options(GuardianReviewSessionKind::TrunkNew, /*snapshot*/ None)
            .await;
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
        let stop = owner.cancellation.child_token();
        let stop_guard = stop.clone().drop_guard();
        let until = stop.clone();
        let reset = prepared.history_reset.clone();
        let spawned = manager
            .start_thread_until(
                options,
                async move {
                    tokio::select! { _ = until.cancelled() => (), _ = reset.cancelled() => () }
                },
                &owner.tasks,
            )
            .await?;
        let runtime = prepared
            .bind_thread(&spawned.thread, key, state, stop)
            .await;
        Ok(Self {
            thread: spawned.thread,
            _stop: stop_guard,
            runtime,
        })
    }
}
