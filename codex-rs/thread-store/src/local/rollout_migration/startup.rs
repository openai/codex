//! Decides whether startup needs to invoke legacy -> paginated rollout migration.
//!
//! It keeps startup cheap by storing a creation-ordered cursor in SQLite and checking only newer
//! rollout files on later launches. When it finds legacy history or a pending recovery marker, it
//! invokes the existing full migration path.
//!
//! Rollouts that background migration cannot finish are remembered so they do not hold the cursor
//! back forever. Ordinary failures stay skipped until a manual migration retries them; busy
//! rollouts are retried on later startups because the writer may have gone away.

use std::collections::HashSet;
use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
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
use super::RolloutMigrationReport;
use super::RolloutMigrationStatus;
use super::find_all_rollout_paths;
use super::migration_error;
use super::publish::migration_journal_path;
use super::publish::pending_migration_thread_ids;
use super::telemetry::RolloutMigrationTrigger;
use super::thread_id_from_rollout_filename;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

const LEGACY_TO_PAGINATED_MIGRATION_ID: &str = "legacy_to_paginated_v1";
const EMPTY_SKIP_REASON: &str = "empty";
const FAILED_SKIP_REASON: &str = "failed";
const MALFORMED_SESSION_META_SKIP_REASON: &str = "malformed_session_meta";
const BUSY_SKIP_REASON: &str = "busy";
const CURSOR_LOOKBACK_SECONDS: i64 = 48 * 60 * 60;
const MAINTENANCE_RETRY_DELAY: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct RolloutFingerprint {
    size_bytes: i64,
    modified_at_ns: i64,
}

enum StartupInspection {
    Paginated,
    Legacy,
    NeedsMigration,
    Skipped,
    Unresolved,
}

pub(super) async fn migrate_rollouts_on_startup(store: &LocalThreadStore) -> ThreadStoreResult<()> {
    let Some(state_db) = store.state_db.as_ref() else {
        return Ok(());
    };
    let paths = find_all_rollout_paths(&store.config.codex_home).await?;
    let mut skipped_rollouts = state_db
        .list_rollout_migration_skipped_rollouts(LEGACY_TO_PAGINATED_MIGRATION_ID)
        .await
        .map_err(migration_error)?;
    retry_busy_rollouts(store, skipped_rollouts.as_slice(), paths.as_slice()).await?;
    skipped_rollouts = state_db
        .list_rollout_migration_skipped_rollouts(LEGACY_TO_PAGINATED_MIGRATION_ID)
        .await
        .map_err(migration_error)?;
    if !pending_migration_thread_ids(&store.config.codex_home)
        .await?
        .is_empty()
    {
        return migrate_all_rollouts(store, paths, skipped_rollouts.as_slice()).await;
    }
    let skipped_file_names = skipped_rollout_file_names(store, skipped_rollouts.as_slice());
    let state = state_db
        .get_rollout_migration_state(LEGACY_TO_PAGINATED_MIGRATION_ID)
        .await
        .map_err(migration_error)?;

    if state.is_none() {
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
            !plain_rollout_file_name(path)
                .is_some_and(|file_name| skipped_file_names.contains(&file_name))
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
            StartupInspection::Legacy | StartupInspection::NeedsMigration => {
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
    let skipped_file_names = skipped_rollout_file_names(store, existing_skips);
    let pending_thread_ids = pending_migration_thread_ids(&store.config.codex_home).await?;
    let paths_to_migrate = paths_before_migration
        .iter()
        .filter(|path| {
            !plain_rollout_file_name(path)
                .is_some_and(|file_name| skipped_file_names.contains(&file_name))
                || thread_id_from_rollout_filename(path)
                    .is_some_and(|thread_id| pending_thread_ids.contains(&thread_id))
        })
        .cloned()
        .collect();
    let report = run_startup_migration(store, paths_to_migrate).await?;
    for outcome in &report.outcomes {
        update_skip_after_outcome(store, outcome).await?;
    }
    // Only mark the pre-migration snapshot; newer rollouts wait for the next startup check.
    advance_last_checked_thread(store, paths_before_migration.as_slice()).await
}

async fn retry_busy_rollouts(
    store: &LocalThreadStore,
    skipped_rollouts: &[RolloutMigrationSkippedRollout],
    discovered_paths: &[PathBuf],
) -> ThreadStoreResult<()> {
    let mut paths = Vec::new();
    let mut moved_skip_paths = Vec::new();
    for skipped_rollout in skipped_rollouts
        .iter()
        .filter(|skipped_rollout| skipped_rollout.skip_reason == BUSY_SKIP_REASON)
    {
        let stored_path = store.config.codex_home.join(&skipped_rollout.rollout_path);
        let path = if tokio::fs::try_exists(&stored_path)
            .await
            .map_err(migration_error)?
        {
            Some(stored_path.clone())
        } else {
            // Archive/unarchive moves one rollout between roots, while compression swaps between
            // its plain and compressed filenames. Match the plain basename across both.
            let file_name = plain_rollout_file_name(&stored_path);
            discovered_paths
                .iter()
                .find(|path| plain_rollout_file_name(path) == file_name)
                .cloned()
        };
        let Some(path) = path else {
            continue;
        };
        if path != stored_path {
            moved_skip_paths.push(skipped_rollout.rollout_path.as_str());
        }
        paths.push(path);
    }
    if paths.is_empty() {
        return Ok(());
    }
    let report = run_startup_migration(store, paths).await?;
    for moved_skip_path in moved_skip_paths {
        remove_skip(store, moved_skip_path).await?;
    }
    for outcome in &report.outcomes {
        update_skip_after_outcome(store, outcome).await?;
    }
    Ok(())
}

async fn run_startup_migration(
    store: &LocalThreadStore,
    paths: Vec<PathBuf>,
) -> ThreadStoreResult<RolloutMigrationReport> {
    loop {
        let Some(maintenance_guard) =
            codex_rollout::try_acquire_rollout_maintenance_lock(&store.config.codex_home)
                .map_err(migration_error)?
        else {
            tokio::time::sleep(MAINTENANCE_RETRY_DELAY).await;
            continue;
        };
        // Avoid counting expected compression contention as a failed migration run. The migration
        // path takes the real lock below, so retry if another maintainer wins this small gap.
        drop(maintenance_guard);
        match store
            .migrate_rollouts_with_progress_for_trigger(
                RolloutMigrationOptions {
                    mode: RolloutMigrationMode::Apply,
                    thread_ids: Vec::new(),
                    max_mib_per_second: None,
                },
                |_| {},
                RolloutMigrationTrigger::Startup,
                super::RolloutMigrationPaths::Known(paths.clone()),
            )
            .await
        {
            Err(ThreadStoreError::Conflict { .. }) => continue,
            result => return result,
        }
    }
}

async fn update_skip_after_outcome(
    store: &LocalThreadStore,
    outcome: &super::RolloutMigrationOutcome,
) -> ThreadStoreResult<()> {
    let relative_path = relative_rollout_path(store, &outcome.rollout_path);
    match outcome.status {
        RolloutMigrationStatus::Migrated | RolloutMigrationStatus::AlreadyPaginated => {
            remove_skip(store, relative_path.as_str()).await
        }
        RolloutMigrationStatus::SkippedEmpty => {
            record_current_skip(store, &outcome.rollout_path, EMPTY_SKIP_REASON).await
        }
        RolloutMigrationStatus::SkippedBusy => {
            record_current_skip(store, &outcome.rollout_path, BUSY_SKIP_REASON).await
        }
        RolloutMigrationStatus::Failed => {
            if outcome.thread_id.is_some_and(|thread_id| {
                migration_journal_path(&store.config.codex_home, thread_id).exists()
            }) {
                return Ok(());
            }
            record_current_skip(store, &outcome.rollout_path, FAILED_SKIP_REASON).await
        }
        RolloutMigrationStatus::Eligible => Ok(()),
    }
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
        Err(_) => {
            let after = rollout_fingerprint(path).await?;
            if before != after {
                return Ok(StartupInspection::Unresolved);
            }
            // The migration path re-reads empty files under the writer lock before deciding
            // whether they are terminally empty or just waiting for SessionMeta.
            if before.size_bytes == 0 {
                return Ok(StartupInspection::NeedsMigration);
            }
            record_skip(store, path, before, MALFORMED_SESSION_META_SKIP_REASON).await?;
            Ok(StartupInspection::Skipped)
        }
    }
}

async fn record_current_skip(
    store: &LocalThreadStore,
    path: &Path,
    skip_reason: &str,
) -> ThreadStoreResult<()> {
    // These fields remain in the generic schema, but background skips are permanent now. Keep
    // recording the best available fingerprint for humans inspecting SQLite.
    let fingerprint = rollout_fingerprint(path).await.unwrap_or_default();
    record_skip(store, path, fingerprint, skip_reason).await
}

async fn record_skip(
    store: &LocalThreadStore,
    path: &Path,
    fingerprint: RolloutFingerprint,
    skip_reason: &str,
) -> ThreadStoreResult<()> {
    let state_db = startup_state_db(store)?;
    let skipped_rollout = RolloutMigrationSkippedRollout {
        rollout_path: relative_rollout_path(store, path),
        rollout_size_bytes: fingerprint.size_bytes,
        rollout_modified_at_ns: fingerprint.modified_at_ns,
        skip_reason: skip_reason.to_string(),
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

fn skipped_rollout_file_names(
    store: &LocalThreadStore,
    skipped_rollouts: &[RolloutMigrationSkippedRollout],
) -> HashSet<OsString> {
    skipped_rollouts
        .iter()
        .filter_map(|skipped_rollout| {
            plain_rollout_file_name(&store.config.codex_home.join(&skipped_rollout.rollout_path))
        })
        .collect()
}

fn plain_rollout_file_name(path: &Path) -> Option<OsString> {
    codex_rollout::plain_rollout_path(path)
        .file_name()
        .map(std::ffi::OsStr::to_os_string)
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
