mod phase_one;
mod phase_two;

use crate::codex::Session;
use crate::config::Config;
use crate::error::Result as CodexResult;
use crate::features::Feature;
use crate::memories::layout::memory_root_for_cwd;
use crate::memories::layout::memory_root_for_user;
use crate::memories::scope::MEMORY_SCOPE_KEY_USER;
use crate::memories::scope::MEMORY_SCOPE_KIND_CWD;
use crate::memories::scope::MEMORY_SCOPE_KIND_USER;
use crate::rollout::INTERACTIVE_SESSION_SOURCES;
use crate::rollout::list::ThreadSortKey;
use crate::state_db;
use codex_protocol::protocol::SessionSource;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;
use tracing::warn;

pub(super) const PHASE_ONE_THREAD_SCAN_LIMIT: usize = 5_000;

/// Canonical memory scope metadata used by both startup phases.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct MemoryScopeTarget {
    /// Scope family used for DB ownership and dirty-state tracking.
    pub(super) scope_kind: &'static str,
    /// Scope identifier used for DB keys.
    pub(super) scope_key: String,
    /// On-disk root where phase-1 artifacts and phase-2 outputs live.
    pub(super) memory_root: PathBuf,
}

/// Converts a pending scope consolidation row into a concrete filesystem target for phase 2.
///
/// Unsupported scope kinds or malformed user-scope keys are ignored.
pub(super) fn memory_scope_target_for_pending_scope(
    config: &Config,
    pending_scope: codex_state::PendingScopeConsolidation,
) -> Option<MemoryScopeTarget> {
    let scope_kind = pending_scope.scope_kind;
    let scope_key = pending_scope.scope_key;

    match scope_kind.as_str() {
        MEMORY_SCOPE_KIND_CWD => {
            let cwd = PathBuf::from(&scope_key);
            Some(MemoryScopeTarget {
                scope_kind: MEMORY_SCOPE_KIND_CWD,
                scope_key,
                memory_root: memory_root_for_cwd(&config.codex_home, &cwd),
            })
        }
        MEMORY_SCOPE_KIND_USER => {
            if scope_key != MEMORY_SCOPE_KEY_USER {
                warn!(
                    "skipping unsupported user memory scope key for phase-2: {}:{}",
                    scope_kind, scope_key
                );
                return None;
            }
            Some(MemoryScopeTarget {
                scope_kind: MEMORY_SCOPE_KIND_USER,
                scope_key,
                memory_root: memory_root_for_user(&config.codex_home),
            })
        }
        _ => {
            warn!(
                "skipping unsupported memory scope for phase-2 consolidation: {}:{}",
                scope_kind, scope_key
            );
            None
        }
    }
}

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

    let weak_session = Arc::downgrade(session);
    tokio::spawn(async move {
        let Some(session) = weak_session.upgrade() else {
            return;
        };
        if let Err(err) = run_memories_startup_pipeline(&session, config).await {
            warn!("memories startup pipeline failed: {err}");
        }
    });
}

/// Runs the startup memory pipeline.
///
/// Phase 1 selects rollout candidates, performs stage-1 extraction requests in
/// parallel, persists stage-1 outputs, and enqueues consolidation work.
///
/// Phase 2 claims pending scopes and spawns consolidation agents.
pub(super) async fn run_memories_startup_pipeline(
    session: &Arc<Session>,
    config: Arc<Config>,
) -> CodexResult<()> {
    let Some(page) = state_db::list_threads_db(
        session.services.state_db.as_deref(),
        &config.codex_home,
        PHASE_ONE_THREAD_SCAN_LIMIT,
        None,
        ThreadSortKey::UpdatedAt,
        INTERACTIVE_SESSION_SOURCES,
        None,
        false,
    )
    .await
    else {
        warn!("state db unavailable for memories startup pipeline; skipping");
        return Ok(());
    };

    let phase_one = phase_one::run_phase_one(session, &page.items).await;
    info!(
        "memory phase-1 candidate selection complete: {} claimed candidate(s) from {} indexed thread(s)",
        phase_one.claimed_candidate_count,
        page.items.len()
    );
    info!(
        "memory phase-1 extraction complete: {} scope(s) touched",
        phase_one.touched_scope_count
    );

    let consolidation_scope_count = phase_two::run_phase_two(session, config).await;
    info!(
        "memory phase-2 consolidation dispatch complete: {} scope(s) scheduled",
        consolidation_scope_count
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::test_config;
    use std::path::PathBuf;

    /// Verifies that phase-2 pending scope rows are translated only for supported scopes.
    #[test]
    fn pending_scope_mapping_accepts_supported_scopes_only() {
        let mut config = test_config();
        config.codex_home = PathBuf::from("/tmp/memory-startup-test-home");

        let cwd_target = memory_scope_target_for_pending_scope(
            &config,
            codex_state::PendingScopeConsolidation {
                scope_kind: MEMORY_SCOPE_KIND_CWD.to_string(),
                scope_key: "/tmp/project-a".to_string(),
            },
        )
        .expect("cwd scope should map");
        assert_eq!(cwd_target.scope_kind, MEMORY_SCOPE_KIND_CWD);

        let user_target = memory_scope_target_for_pending_scope(
            &config,
            codex_state::PendingScopeConsolidation {
                scope_kind: MEMORY_SCOPE_KIND_USER.to_string(),
                scope_key: MEMORY_SCOPE_KEY_USER.to_string(),
            },
        )
        .expect("valid user scope should map");
        assert_eq!(user_target.scope_kind, MEMORY_SCOPE_KIND_USER);

        assert!(
            memory_scope_target_for_pending_scope(
                &config,
                codex_state::PendingScopeConsolidation {
                    scope_kind: MEMORY_SCOPE_KIND_USER.to_string(),
                    scope_key: "unexpected-user-key".to_string(),
                },
            )
            .is_none()
        );

        assert!(
            memory_scope_target_for_pending_scope(
                &config,
                codex_state::PendingScopeConsolidation {
                    scope_kind: "unknown".to_string(),
                    scope_key: "scope".to_string(),
                },
            )
            .is_none()
        );
    }
}
