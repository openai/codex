//! Decides whether startup needs to invoke legacy -> paginated rollout migration.
//!
//! It keeps startup cheap by storing a creation-ordered cursor in SQLite and checking only newer
//! rollout files on later launches. When it finds legacy history or a pending recovery marker, it
//! invokes the existing full migration path.
//!
//! Empty or malformed rollouts are fingerprinted so they do not block the cursor forever. If one
//! changes later, startup retries it instead of trusting the old skip.

use std::collections::HashSet;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

use chrono::NaiveDateTime;
use codex_protocol::ThreadId;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_rollout::StateDbHandle;
use codex_state::RolloutMigrationCursor;
use codex_state::RolloutMigrationSkippedRollout;

use super::LocalThreadStore;
use super::RolloutMigrationMode;
use super::RolloutMigrationOptions;
use super::RolloutMigrationStatus;
use super::find_rollout_paths;
use super::migration_error;
use super::publish::pending_migration_thread_ids;
use super::telemetry::RolloutMigrationTrigger;
use crate::ThreadStoreResult;

const LEGACY_TO_PAGINATED_MIGRATION_ID: &str = "legacy_to_paginated_v1";
const EMPTY_SKIP_REASON: &str = "empty";
const MALFORMED_SESSION_META_SKIP_REASON: &str = "malformed_session_meta";
const CURSOR_LOOKBACK_SECONDS: i64 = 48 * 60 * 60;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RolloutFingerprint {
    size_bytes: i64,
    modified_at_ns: i64,
}

enum StartupInspection {
    Paginated,
    Legacy,
    Skipped,
    Unresolved,
}

pub(super) async fn migrate_rollouts_on_startup(store: &LocalThreadStore) -> ThreadStoreResult<()> {
    let Some(state_db) = store.state_db.as_ref() else {
        return Ok(());
    };
    let paths = find_all_rollout_paths(store).await?;
    let skipped_rollouts = state_db
        .list_rollout_migration_skipped_rollouts(LEGACY_TO_PAGINATED_MIGRATION_ID)
        .await
        .map_err(migration_error)?;
    if !pending_migration_thread_ids(&store.config.codex_home)
        .await?
        .is_empty()
    {
        return migrate_all_rollouts(store, paths, skipped_rollouts.as_slice()).await;
    }
    let (unchanged_skips, invalidated_skip) =
        revalidate_skipped_rollouts(store, skipped_rollouts.as_slice()).await?;
    let state = state_db
        .get_rollout_migration_state(LEGACY_TO_PAGINATED_MIGRATION_ID)
        .await
        .map_err(migration_error)?;

    if state.is_none() || invalidated_skip {
        return migrate_all_rollouts(store, paths, skipped_rollouts.as_slice()).await;
    }

    let last_checked_thread = state.and_then(|state| state.last_checked_thread);
    let lookback_created_at = last_checked_thread.as_ref().map(|cursor| {
        cursor
            .thread_created_at
            .saturating_sub(CURSOR_LOOKBACK_SECONDS)
    });
    let candidates = paths
        .iter()
        .filter(|path| {
            let relative_path = relative_rollout_path(store, path);
            !unchanged_skips.contains(relative_path.as_str())
                && thread_creation_cursor(path).is_none_or(|cursor| {
                    lookback_created_at.is_none_or(|lookback_created_at| {
                        cursor.thread_created_at >= lookback_created_at
                    })
                })
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Ok(());
    }

    let mut unresolved = false;
    for path in candidates {
        match inspect_rollout_path(store, path).await? {
            StartupInspection::Paginated | StartupInspection::Skipped => {}
            StartupInspection::Legacy => {
                return migrate_all_rollouts(store, paths, skipped_rollouts.as_slice()).await;
            }
            StartupInspection::Unresolved => unresolved = true,
        }
    }
    if unresolved {
        return Ok(());
    }

    advance_last_checked_thread(store, paths.as_slice()).await
}

async fn migrate_all_rollouts(
    store: &LocalThreadStore,
    paths_before_migration: Vec<PathBuf>,
    existing_skips: &[RolloutMigrationSkippedRollout],
) -> ThreadStoreResult<()> {
    let report = store
        .migrate_rollouts_with_progress_for_trigger(
            RolloutMigrationOptions {
                mode: RolloutMigrationMode::Apply,
                thread_ids: Vec::new(),
                max_mib_per_second: None,
            },
            |_| {},
            RolloutMigrationTrigger::Startup,
        )
        .await?;
    let existing_skip_paths = existing_skips
        .iter()
        .map(|skipped_rollout| skipped_rollout.rollout_path.as_str())
        .collect::<HashSet<_>>();
    let mut terminal = true;
    let mut reported_paths = HashSet::new();
    for outcome in &report.outcomes {
        let relative_path = relative_rollout_path(store, &outcome.rollout_path);
        reported_paths.insert(relative_path.clone());
        match outcome.status {
            RolloutMigrationStatus::Migrated | RolloutMigrationStatus::AlreadyPaginated => {
                if existing_skip_paths.contains(relative_path.as_str()) {
                    remove_skip(store, relative_path.as_str()).await?;
                }
            }
            RolloutMigrationStatus::SkippedEmpty | RolloutMigrationStatus::Failed => {
                if !matches!(
                    inspect_rollout_path(store, &outcome.rollout_path).await?,
                    StartupInspection::Skipped
                ) {
                    terminal = false;
                }
            }
            RolloutMigrationStatus::Eligible | RolloutMigrationStatus::SkippedBusy => {
                terminal = false
            }
        }
    }
    if !terminal {
        return Ok(());
    }
    for skipped_rollout in existing_skips {
        if !reported_paths.contains(skipped_rollout.rollout_path.as_str()) {
            remove_skip(store, skipped_rollout.rollout_path.as_str()).await?;
        }
    }
    // Only mark the pre-migration snapshot; newer rollouts wait for the next startup check.
    advance_last_checked_thread(store, paths_before_migration.as_slice()).await
}

async fn inspect_rollout_path(
    store: &LocalThreadStore,
    path: &Path,
) -> ThreadStoreResult<StartupInspection> {
    let before = rollout_fingerprint(path).await?;
    match codex_rollout::read_session_meta_line(path).await {
        Ok(metadata) if metadata.meta.history_mode == ThreadHistoryMode::Legacy => {
            Ok(StartupInspection::Legacy)
        }
        Ok(_) => Ok(StartupInspection::Paginated),
        Err(error) => {
            let after = rollout_fingerprint(path).await?;
            if before != after || !matches!(error.kind(), ErrorKind::Other | ErrorKind::InvalidData)
            {
                return Ok(StartupInspection::Unresolved);
            }
            record_skip(store, path, before).await?;
            Ok(StartupInspection::Skipped)
        }
    }
}

async fn record_skip(
    store: &LocalThreadStore,
    path: &Path,
    fingerprint: RolloutFingerprint,
) -> ThreadStoreResult<()> {
    let state_db = startup_state_db(store)?;
    let skipped_rollout = RolloutMigrationSkippedRollout {
        rollout_path: relative_rollout_path(store, path),
        rollout_size_bytes: fingerprint.size_bytes,
        rollout_modified_at_ns: fingerprint.modified_at_ns,
        skip_reason: if fingerprint.size_bytes == 0 {
            EMPTY_SKIP_REASON.to_string()
        } else {
            MALFORMED_SESSION_META_SKIP_REASON.to_string()
        },
    };
    state_db
        .record_rollout_migration_skip(LEGACY_TO_PAGINATED_MIGRATION_ID, &skipped_rollout)
        .await
        .map_err(migration_error)
}

async fn remove_skip(store: &LocalThreadStore, rollout_path: &str) -> ThreadStoreResult<()> {
    startup_state_db(store)?
        .remove_rollout_migration_skip(LEGACY_TO_PAGINATED_MIGRATION_ID, rollout_path)
        .await
        .map_err(migration_error)
}

async fn revalidate_skipped_rollouts(
    store: &LocalThreadStore,
    skipped_rollouts: &[RolloutMigrationSkippedRollout],
) -> ThreadStoreResult<(HashSet<String>, bool)> {
    let mut unchanged_skips = HashSet::new();
    let mut invalidated_skip = false;
    for skipped_rollout in skipped_rollouts {
        let path = store.config.codex_home.join(&skipped_rollout.rollout_path);
        let fingerprint = match rollout_fingerprint(&path).await {
            Ok(fingerprint) => fingerprint,
            Err(_) => {
                invalidated_skip = true;
                continue;
            }
        };
        if fingerprint.size_bytes == skipped_rollout.rollout_size_bytes
            && fingerprint.modified_at_ns == skipped_rollout.rollout_modified_at_ns
        {
            unchanged_skips.insert(skipped_rollout.rollout_path.clone());
        } else {
            invalidated_skip = true;
        }
    }
    Ok((unchanged_skips, invalidated_skip))
}

async fn advance_last_checked_thread(
    store: &LocalThreadStore,
    paths: &[PathBuf],
) -> ThreadStoreResult<()> {
    let last_checked_thread = paths
        .iter()
        .filter_map(|path| thread_creation_cursor(path))
        .max();
    startup_state_db(store)?
        .advance_rollout_migration_state(
            LEGACY_TO_PAGINATED_MIGRATION_ID,
            last_checked_thread.as_ref(),
        )
        .await
        .map_err(migration_error)
}

fn startup_state_db(store: &LocalThreadStore) -> ThreadStoreResult<&StateDbHandle> {
    store
        .state_db
        .as_ref()
        .ok_or_else(|| migration_error("startup migration requires state db"))
}

async fn find_all_rollout_paths(store: &LocalThreadStore) -> ThreadStoreResult<Vec<PathBuf>> {
    let mut paths =
        find_rollout_paths(&store.config.codex_home.join(codex_rollout::SESSIONS_SUBDIR)).await?;
    paths.extend(
        find_rollout_paths(
            &store
                .config
                .codex_home
                .join(codex_rollout::ARCHIVED_SESSIONS_SUBDIR),
        )
        .await?,
    );
    Ok(paths)
}

fn thread_creation_cursor(path: &Path) -> Option<RolloutMigrationCursor> {
    let name = path.file_name()?.to_str()?;
    let stem = name
        .strip_suffix(".jsonl.zst")
        .or_else(|| name.strip_suffix(".jsonl"))?
        .strip_prefix("rollout-")?;
    let separator = stem.len().checked_sub(37)?;
    let thread_id = stem.get(separator + 1..)?;
    ThreadId::from_string(thread_id).ok()?;
    let timestamp = NaiveDateTime::parse_from_str(stem.get(..separator)?, "%Y-%m-%dT%H-%M-%S")
        .ok()?
        .and_utc()
        .timestamp();
    Some(RolloutMigrationCursor {
        thread_created_at: timestamp,
        thread_id: thread_id.to_string(),
    })
}

fn relative_rollout_path(store: &LocalThreadStore, path: &Path) -> String {
    path.strip_prefix(&store.config.codex_home)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

async fn rollout_fingerprint(path: &Path) -> ThreadStoreResult<RolloutFingerprint> {
    let metadata = tokio::fs::metadata(path).await.map_err(migration_error)?;
    let size_bytes = i64::try_from(metadata.len()).map_err(migration_error)?;
    let modified_at_ns = metadata
        .modified()
        .map_err(migration_error)?
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(migration_error)?
        .as_nanos();
    let modified_at_ns = i64::try_from(modified_at_ns).map_err(migration_error)?;
    Ok(RolloutFingerprint {
        size_bytes,
        modified_at_ns,
    })
}

#[cfg(test)]
#[path = "startup_tests.rs"]
mod tests;
