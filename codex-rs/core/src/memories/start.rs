use crate::codex::Session;
use crate::config::Config;
use crate::features::Feature;
use crate::memories::phase1;
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
) {
    if config.ephemeral
        || !config.features.enabled(Feature::MemoryTool)
        || matches!(source, SessionSource::SubAgent(_))
    {
        return;
    }

    if session.services.state_db.is_none() {
        warn!("state db unavailable for memories startup pipeline; skipping");
        return;
    }

    let weak_session = Arc::downgrade(session);
    tokio::spawn(async move {
        let Some(session) = weak_session.upgrade() else {
            return;
        };

        // Run phase 1.
        phase1::run(&session).await;
        // Run phase 2.
        crate::memories::dispatch::run_global_memory_consolidation(&session, config).await;
    });
}
