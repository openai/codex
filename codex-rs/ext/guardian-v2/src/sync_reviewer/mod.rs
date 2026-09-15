//! Runs Guardian agents through ThreadManager and owns their background tasks.
//! Context stays in the temporary core adapter. Parent stop joins all reviewer
//! work before the parent closes its history, including partial startup.

use std::sync::Arc;
use std::sync::Weak;

use codex_core::CodexResponsesHeaders;
use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_core::config::Constrained;
use codex_core::config::TokenBudgetConfig;
use codex_core::guardian_review::GuardianReviewSession;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::SessionIsolation;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadReadyInput;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::ThreadStopInput;
use codex_extension_api::TurnAbortInput;
use codex_extension_api::TurnLifecycleContributor;
use codex_extension_api::TurnStartInput;
use codex_extension_api::TurnStopInput;
use codex_guardian_reviewer::ReviewDenials;
use codex_guardian_reviewer::ReviewerPool;
use codex_guardian_reviewer::ReviewerTasks;
use codex_protocol::ThreadId;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::InternalSessionSource;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::ThreadSource;

mod reviewer_config;

/// Owns reviewer agents through the same thread manager as the parent conversation.
#[derive(Debug)]
struct GuardianExtension {
    thread_manager: Weak<ThreadManager>,
}

impl ThreadLifecycleContributor<Config> for GuardianExtension {
    fn on_thread_start<'a>(
        &'a self,
        input: ThreadStartInput<'a, Config>,
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            if input.session_source.is_internal() {
                return;
            }
            input
                .thread_store
                .insert(codex_guardian_reviewer::ReviewerConfig::<Config>(
                    reviewer_config::build_reviewer_config,
                ));
            let manager = self.thread_manager.clone();
            let runtime = input.thread_store.get_or_init(ReviewerTasks::default);
            input.thread_store.get_or_init(|| {
                ReviewerPool::<GuardianReviewSession>::new(
                    Arc::clone(&runtime),
                    move |context, key, kind, snapshot, cancel| {
                        let manager = manager.clone();
                        let runtime = Arc::clone(&runtime);
                        Box::pin(async move {
                            let history_reset = context.history_reset.clone();
                            // Register before checking cancellation so parent stop also joins
                            // a spawn racing with shutdown.
                            let _task = runtime.tasks.token();
                            anyhow::ensure!(
                                !runtime.cancellation.is_cancelled()
                                    && !cancel.is_cancelled()
                                    && !history_reset.is_cancelled(),
                                "Guardian is stopping"
                            );
                            let manager = manager.upgrade().ok_or_else(|| {
                                anyhow::anyhow!("thread manager is no longer available")
                            })?;
                            let (mut options, state) = context.thread_options(snapshot).await;
                            if matches!(
                                kind,
                                codex_analytics::GuardianReviewSessionKind::EphemeralForked
                            ) {
                                options.config.ephemeral = true;
                            }
                            options.session_source =
                                Some(SessionSource::Internal(InternalSessionSource::Guardian));
                            options.thread_source = Some(ThreadSource::GuardianReview);
                            // This is the backend reviewer model, independent of current login.
                            // Core checks the selected model and auth on each request attempt.
                            let provider = codex_model_provider::create_model_provider(
                                options.config.model_provider.clone(),
                                /*auth_manager*/ None,
                            );
                            options.thread_extension_init.insert(CodexResponsesHeaders {
                                model: provider.approval_review_preferred_model().to_owned(),
                                headers: http::HeaderMap::from_iter([(
                                    http::HeaderName::from_static("x-codex-guardian"),
                                    http::HeaderValue::from_static("reviewer"),
                                )]),
                            });
                            options
                                .thread_extension_init
                                .insert(SessionIsolation::Isolated);
                            options
                                .thread_extension_init
                                .insert(codex_guardian_reviewer::reviewer_allowed_tools());
                            let session_cancel = cancel.clone();
                            let until = async move {
                                let _cancel_on_exit = cancel.clone().drop_guard();
                                tokio::select! {
                                    _ = cancel.cancelled() => {}
                                    _ = history_reset.cancelled() => {}
                                }
                            };
                            let spawned = manager
                                .start_thread_until(options, until, &runtime.tasks)
                                .await?;
                            Ok(context
                                .bind_thread(&spawned.thread, key, state, session_cancel)
                                .await)
                        })
                    },
                )
            });
        })
    }

    fn on_thread_ready<'a>(
        &'a self,
        input: ThreadReadyInput<'a, Config>,
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            if input.session_source.is_internal()
                || !matches!(
                    input.config.permissions.approval_policy.value(),
                    AskForApproval::OnRequest | AskForApproval::Granular(_)
                )
                || input.config.approvals_reviewer != ApprovalsReviewer::AutoReview
            {
                return;
            }
            let Some(runtime) = input.thread_store.get::<ReviewerTasks>() else {
                return;
            };
            let task = runtime.tasks.token();
            if runtime.cancellation.is_cancelled() {
                return;
            }
            let Some(manager) = self.thread_manager.upgrade() else {
                return;
            };
            let Ok(thread_id) = ThreadId::from_string(input.thread_store.level_id()) else {
                return;
            };
            let Ok(parent) = manager.get_thread(thread_id).await else {
                return;
            };
            let Some(pool) = input
                .thread_store
                .get::<ReviewerPool<GuardianReviewSession>>()
            else {
                return;
            };
            let cancel = runtime.cancellation.clone();
            tokio::spawn(async move {
                let _task = task;
                let prepare = async {
                    let context =
                        codex_core::guardian_review::prepare_review_prewarm(&parent).await?;
                    let key = context.reuse_key(/*previous*/ None);
                    pool.prewarm(Arc::new(context), key).await
                };
                tokio::select! {
                    _ = cancel.cancelled() => {}
                    result = prepare => {
                        if let Err(error) = result {
                            tracing::warn!("failed to prewarm Guardian reviewer: {error:#}");
                        }
                    }
                }
            });
        })
    }

    fn on_thread_stop<'a>(&'a self, input: ThreadStopInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            if let Some(pool) = input
                .thread_store
                .get::<ReviewerPool<GuardianReviewSession>>()
            {
                pool.shutdown().await;
            }
        })
    }
}

impl TurnLifecycleContributor for GuardianExtension {
    fn on_turn_start<'a>(&'a self, input: TurnStartInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(ReviewDenials::clear_turn(input.thread_store, input.turn_id))
    }
    fn on_turn_stop<'a>(&'a self, input: TurnStopInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(ReviewDenials::clear_turn(
            input.thread_store,
            input.turn_store.level_id(),
        ))
    }
    fn on_turn_abort<'a>(&'a self, input: TurnAbortInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(ReviewDenials::clear_turn(
            input.thread_store,
            input.turn_store.level_id(),
        ))
    }
}

/// Registers the synchronous reviewer and its thread and turn cleanup.
pub fn install(
    registry: &mut ExtensionRegistryBuilder<Config>,
    thread_manager: Weak<ThreadManager>,
) {
    let extension = Arc::new(GuardianExtension { thread_manager });
    registry.thread_lifecycle_contributor(extension.clone());
    registry.turn_lifecycle_contributor(extension);
}

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod tests;
