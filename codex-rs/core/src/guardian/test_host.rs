//! Explicit ThreadManager host for core's context-adapter unit tests.
//! Application-level tests install the real Guardian extension instead.

use std::sync::Arc;

use codex_extension_api::SessionIsolation;
use codex_home::CodexHomeUserInstructionsProvider;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::InternalSessionSource;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::ThreadSource;

use super::GuardianReviewSessionManager;
use crate::config::Config;
use crate::config::Constrained;
use crate::session::session::Session;

pub(crate) fn install(session: &Session, config: &Config) {
    let manager = Arc::new(crate::ThreadManager::new(
        config,
        Arc::clone(&session.services.auth_manager),
        Arc::clone(&session.services.models_manager),
        crate::CodexAppsToolsCache::default(),
        SessionSource::Exec,
        session.services.turn_environments.environment_manager(),
        codex_extension_api::empty_extension_registry(),
        Arc::new(CodexHomeUserInstructionsProvider::new(
            config.codex_home.clone(),
        )),
        /*analytics_events_client*/ None,
        crate::passthrough_image_store(),
        Arc::clone(&session.services.thread_store),
        /*agent_graph_store*/ None,
        session.installation_id.clone(),
        /*attestation_provider*/ None,
        /*external_time_provider*/ None,
    ));
    let runtime = Arc::new(codex_guardian_reviewer::ReviewerTasks::default());
    session
        .services
        .thread_extension_data
        .insert(GuardianReviewSessionManager::new(
            Arc::clone(&runtime),
            move |context, key, kind, snapshot, cancel| {
                let manager = Arc::clone(&manager);
                let runtime = Arc::clone(&runtime);
                Box::pin(async move {
                    let history_reset = context.history_reset.clone();
                    let (mut options, state) = context.thread_options(snapshot).await;
                    if matches!(
                        kind,
                        codex_analytics::GuardianReviewSessionKind::EphemeralForked
                    ) {
                        options.config.ephemeral = true;
                    }
                    options.config.permissions.approval_policy =
                        Constrained::allow_only(AskForApproval::Never);
                    options.session_source =
                        Some(SessionSource::Internal(InternalSessionSource::Guardian));
                    options.thread_source = Some(ThreadSource::GuardianReview);
                    options
                        .thread_extension_init
                        .insert(SessionIsolation::Isolated);
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
        ));
}
