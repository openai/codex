use crate::codex::Session;
use crate::config::Config;
use crate::features::Feature;
use crate::memories::phase1;
use crate::memories::phase2;
use codex_protocol::protocol::BackgroundEventEvent;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SessionSource;
use std::sync::Arc;
use tracing::warn;

/// Starts the asynchronous startup memory pipeline for an eligible root session.
///
/// The pipeline is skipped for ephemeral sessions, disabled feature flags, and
/// subagent sessions.
pub(crate) fn start_memories_startup_task(
    session: &Arc<Session>,
    config: Arc<Config>,
    source: &SessionSource,
    progress_sub_id: Option<String>,
) {
    if config.ephemeral
        || !config.features.enabled(Feature::MemoryTool)
        || matches!(source, SessionSource::SubAgent(_))
    {
        return;
    }

    if session.services.state_db.is_none() {
        warn!("state db unavailable for memories startup pipeline; skipping");
        let weak_session = Arc::downgrade(session);
        tokio::spawn(async move {
            let Some(session) = weak_session.upgrade() else {
                return;
            };
            emit_memory_progress(
                session.as_ref(),
                &progress_sub_id,
                "phase 1 skipped (state db unavailable)",
            )
            .await;
        });
        return;
    }

    let weak_session = Arc::downgrade(session);
    tokio::spawn(async move {
        let Some(session) = weak_session.upgrade() else {
            return;
        };

        // Run phase 1.
        phase1::run(&session, &config, &progress_sub_id).await;
        // Run phase 2.
        phase2::run(&session, config, &progress_sub_id).await;
    });
}

pub(super) async fn emit_memory_progress(
    session: &Session,
    progress_sub_id: &Option<String>,
    message: impl Into<String>,
) {
    let Some(sub_id) = progress_sub_id.as_ref() else {
        return;
    };

    session
        .send_event_raw(Event {
            id: sub_id.clone(),
            msg: EventMsg::BackgroundEvent(BackgroundEventEvent {
                message: format!("memory startup: {}", message.into()),
            }),
        })
        .await;
}
