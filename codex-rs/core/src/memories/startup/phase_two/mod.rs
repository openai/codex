mod dispatch;
mod watch;

use super::super::MAX_ROLLOUTS_PER_STARTUP;
use super::super::PHASE_TWO_CONCURRENCY_LIMIT;
use super::MemoryScopeTarget;
use super::memory_scope_target_for_pending_scope;
use crate::codex::Session;
use crate::config::Config;
use futures::StreamExt;
use std::sync::Arc;

/// Runs startup phase 2:
///
/// 1. Load scopes pending consolidation from the DB.
/// 2. Claim scope jobs.
/// 3. Spawn consolidation agents for owned scopes.
pub(super) async fn run_phase_two(session: &Arc<Session>, config: Arc<Config>) -> usize {
    let scopes =
        list_phase2_scopes(session.as_ref(), config.as_ref(), MAX_ROLLOUTS_PER_STARTUP).await;
    let consolidation_scope_count = scopes.len();

    futures::stream::iter(scopes.into_iter())
        .map(|scope| {
            let session = Arc::clone(session);
            let config = Arc::clone(&config);
            async move {
                dispatch::run_memory_consolidation_for_scope(session, config, scope).await;
            }
        })
        .buffer_unordered(PHASE_TWO_CONCURRENCY_LIMIT)
        .collect::<Vec<_>>()
        .await;

    consolidation_scope_count
}

async fn list_phase2_scopes(
    session: &Session,
    config: &Config,
    limit: usize,
) -> Vec<MemoryScopeTarget> {
    if limit == 0 {
        return Vec::new();
    }

    let Some(state_db) = session.services.state_db.as_deref() else {
        return Vec::new();
    };

    let pending_scopes = match state_db.list_pending_scope_consolidations(limit).await {
        Ok(scopes) => scopes,
        Err(_) => return Vec::new(),
    };

    pending_scopes
        .into_iter()
        .filter_map(|scope| memory_scope_target_for_pending_scope(config, scope))
        .collect()
}
