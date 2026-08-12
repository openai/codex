//! Resolves a thread ID to the rollout file the thread currently uses.
//!
//! Most threads have one rollout file. `thread/revert` keeps the thread ID stable while switching
//! the thread to a new rollout file, so callers cannot infer the selected path from the thread ID
//! alone. This module centralizes live-writer, SQLite, and filesystem fallback resolution.

use std::path::PathBuf;

use codex_protocol::ThreadId;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_rollout::find_archived_thread_path_by_id_str;
use codex_rollout::find_thread_path_by_id_str;

use super::LocalThreadStore;
use super::helpers::rollout_path_is_archived;
use super::live_writer;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

/// One thread resolved to the concrete rollout file it currently uses.
///
/// For ordinary threads, `thread_id` and `rollout_id` are the same. After `thread/revert`,
/// `thread_id` stays stable while `rollout_id` identifies the new immutable rollout file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ResolvedThreadRollout {
    pub(super) thread_id: ThreadId,
    pub(super) rollout_id: ThreadId,
    pub(super) path: PathBuf,
    pub(super) location: RolloutLocation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RolloutLocation {
    Unarchived,
    Archived,
}

pub(super) async fn resolve_current(
    store: &LocalThreadStore,
    thread_id: ThreadId,
) -> ThreadStoreResult<Option<ResolvedThreadRollout>> {
    resolve(store, thread_id, LookupScope::ExcludeArchived).await
}

pub(super) async fn resolve_current_including_archived(
    store: &LocalThreadStore,
    thread_id: ThreadId,
) -> ThreadStoreResult<Option<ResolvedThreadRollout>> {
    resolve(store, thread_id, LookupScope::IncludeArchived).await
}

#[derive(Clone, Copy)]
enum LookupScope {
    ExcludeArchived,
    IncludeArchived,
}

impl LookupScope {
    fn accepts(self, location: RolloutLocation) -> bool {
        match self {
            Self::ExcludeArchived => location == RolloutLocation::Unarchived,
            Self::IncludeArchived => true,
        }
    }
}

/// Resolves a thread's selected rollout in this order:
///
/// 1. The live writer, when one exists.
/// 2. SQLite's selected rollout path.
/// 3. Filesystem fallback for threads SQLite does not identify as paginated.
/// 4. Archived filesystem fallback, when requested.
async fn resolve(
    store: &LocalThreadStore,
    thread_id: ThreadId,
    scope: LookupScope,
) -> ThreadStoreResult<Option<ResolvedThreadRollout>> {
    if let Ok(path) = live_writer::rollout_path(store, thread_id).await
        && codex_rollout::existing_rollout_path(path.as_path())
            .await
            .is_some()
        && let Some(resolved) = resolve_path_in_scope(store, thread_id, path, scope).await?
    {
        return Ok(Some(resolved));
    }

    let state_db_ctx = store.state_db().await;
    if let Some(state_db_ctx) = state_db_ctx.as_deref() {
        match state_db_ctx.get_thread(thread_id).await {
            Ok(Some(metadata)) => {
                // Once SQLite identifies a thread as paginated, its rollout path is
                // authoritative: after `thread/revert`, a scan could find an older immutable
                // rollout for the same thread. Filesystem fallback remains available when SQLite
                // has no row or identifies the thread as legacy.
                if let Some(path) =
                    codex_rollout::existing_rollout_path(metadata.rollout_path.as_path()).await
                {
                    let belongs_to_thread =
                        match codex_rollout::read_session_meta_line(path.as_path()).await {
                            Ok(session_meta) => session_meta.meta.id == thread_id,
                            Err(_) => true,
                        };
                    if belongs_to_thread {
                        return resolve_path_in_scope(store, thread_id, path, scope).await;
                    }
                }
                if metadata.history_mode == ThreadHistoryMode::Paginated {
                    return Ok(None);
                }
            }
            Ok(None) => {}
            Err(err) => {
                return Err(ThreadStoreError::Internal {
                    message: format!("failed to read thread metadata for {thread_id}: {err}"),
                });
            }
        }
    }
    if let Some(path) = find_thread_path_by_id_str(
        store.config.codex_home.as_path(),
        &thread_id.to_string(),
        state_db_ctx.as_deref(),
    )
    .await
    .map_err(|err| ThreadStoreError::InvalidRequest {
        message: format!("failed to locate thread id {thread_id}: {err}"),
    })? && let Some(resolved) = resolve_path_in_scope(store, thread_id, path, scope).await?
    {
        return Ok(Some(resolved));
    }
    if !scope.accepts(RolloutLocation::Archived) {
        return Ok(None);
    }
    let path = find_archived_thread_path_by_id_str(
        store.config.codex_home.as_path(),
        &thread_id.to_string(),
        state_db_ctx.as_deref(),
    )
    .await
    .map_err(|err| ThreadStoreError::InvalidRequest {
        message: format!("failed to locate archived thread id {thread_id}: {err}"),
    })?;
    match path {
        Some(path) => resolve_path_in_scope(store, thread_id, path, scope).await,
        None => Ok(None),
    }
}

async fn resolve_path_in_scope(
    store: &LocalThreadStore,
    thread_id: ThreadId,
    path: PathBuf,
    scope: LookupScope,
) -> ThreadStoreResult<Option<ResolvedThreadRollout>> {
    let location = location_for_path(store, path.as_path());
    if !scope.accepts(location) {
        return Ok(None);
    }
    resolve_path(thread_id, path, location).await.map(Some)
}

fn location_for_path(store: &LocalThreadStore, path: &std::path::Path) -> RolloutLocation {
    if rollout_path_is_archived(store.config.codex_home.as_path(), path) {
        RolloutLocation::Archived
    } else {
        RolloutLocation::Unarchived
    }
}

async fn resolve_path(
    thread_id: ThreadId,
    path: PathBuf,
    location: RolloutLocation,
) -> ThreadStoreResult<ResolvedThreadRollout> {
    let rollout_id = match codex_rollout::rollout_id_from_path(path.as_path()) {
        Some(rollout_id) => rollout_id,
        None => {
            let history_mode = codex_rollout::read_session_meta_line(path.as_path())
                .await
                .map_err(|err| ThreadStoreError::Internal {
                    message: format!("failed to read session metadata {}: {err}", path.display()),
                })?
                .meta
                .history_mode;
            rollout_id_from_path_or_legacy_thread_id(path.as_path(), thread_id, history_mode)?
        }
    };
    Ok(ResolvedThreadRollout {
        thread_id,
        rollout_id,
        path,
        location,
    })
}

/// Returns the immutable rollout ID for a path while preserving legacy noncanonical filenames.
pub(super) fn rollout_id_from_path_or_legacy_thread_id(
    path: &std::path::Path,
    thread_id: ThreadId,
    history_mode: ThreadHistoryMode,
) -> ThreadStoreResult<ThreadId> {
    Ok(match codex_rollout::rollout_id_from_path(path) {
        Some(rollout_id) => rollout_id,
        None => {
            if history_mode == ThreadHistoryMode::Paginated {
                return Err(ThreadStoreError::InvalidRequest {
                    message: format!(
                        "paginated rollout path `{}` does not have a canonical rollout filename",
                        path.display()
                    ),
                });
            }
            thread_id
        }
    })
}
