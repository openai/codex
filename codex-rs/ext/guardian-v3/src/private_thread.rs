//! Model runtime owned by a parent task tracker. Dropping its handle requests shutdown.
use std::sync::Arc;

use codex_analytics::GuardianReviewSessionKind;
use codex_core::CodexThread;
use codex_core::ThreadManager;
use codex_core::guardian_review::GuardianReviewSession;
use codex_core::guardian_review::prepare_review_prewarm;
use codex_extension_api::SessionIsolation;
use codex_extension_api::ToolPolicy;
use codex_guardian_reviewer::ReviewerTasks;
use codex_protocol::protocol::InternalSessionSource;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::TruncationPolicy;
use tokio_util::sync::DropGuard;

pub(super) struct PrivateThread {
    pub runtime: GuardianReviewSession,
    pub policy: TruncationPolicy,
    _stop: DropGuard,
}

impl PrivateThread {
    pub async fn start(
        manager: &Arc<ThreadManager>,
        parent: &CodexThread,
        owner: &ReviewerTasks,
        instructions: &str,
    ) -> anyhow::Result<Self> {
        let prepared = prepare_review_prewarm(parent)
            .await?
            .without_parent_instructions();
        let key = prepared.reuse_key(/*previous*/ None);
        let (mut options, state) = prepared
            .thread_options(GuardianReviewSessionKind::TrunkNew, /*snapshot*/ None)
            .await;
        options.initial_history = codex_history::InitialHistory::New;
        options.config.ephemeral = true;
        options.config.base_instructions = Some(instructions.into());
        options.session_source = Some(SessionSource::Internal(InternalSessionSource::Guardian));
        options
            .thread_extension_init
            .insert(SessionIsolation::Isolated);
        options.thread_extension_init.insert(ToolPolicy {
            allowed_tools: Some(Vec::new()),
            ..Default::default()
        });
        let stop = owner.cancellation.child_token();
        let stop_guard = stop.clone().drop_guard();
        let reset = prepared.history_reset.clone();
        let until = stop.clone();
        let spawned = manager
            .start_thread_until(
                options,
                async move {
                    tokio::select! { _ = until.cancelled() => (), _ = reset.cancelled() => () }
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
            .bind_thread(&spawned.thread, key, state, stop.clone())
            .await;
        Ok(Self {
            runtime,
            policy,
            _stop: stop_guard,
        })
    }
}
