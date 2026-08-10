//! Orchestrates legacy rollout migration into paginated history.
//!
//! This is the high-level migration state machine: find rollout files, decide whether each one is
//! eligible, take the maintenance and writer locks, canonicalize into a staged JSONL file,
//! project that staged file into SQLite, verify the projection, then atomically publish it.
//!
//! The important invariant is that we always leave behind either the original legacy rollout or a
//! recoverable paginated rollout. Once the rollout path is replaced, the durable `.pending`
//! journal must be enough for a later migration run to finish SQLite recovery safely.

use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use chrono::DateTime;
use codex_app_server_protocol::project_rollout_line;
use codex_protocol::ThreadId;
use codex_protocol::protocol::InternalSessionSource;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::protocol::ThreadSource;
use codex_rollout::RolloutItem;
use codex_rollout::RolloutLine;
use serde::Serialize;
use tokio::fs::File;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::io::BufWriter;
use tokio::time::Instant;

use super::LocalThreadStore;
use super::thread_history;
use super::thread_history::ProjectedRolloutLine;
use super::thread_history::RolloutProjectionStep;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

mod canonicalizer;
mod legacy_event;
mod line_parser;
mod publish;
mod rollback;
mod rollback_plan;
mod rollback_replay;
mod startup;
mod subagent;
mod telemetry;

use canonicalizer::LegacyRolloutCanonicalizer;
use publish::compress_rollout_to_path;
use publish::compressed_staged_rollout_path;
use publish::decompress_rollout_to_path;
use publish::decompressed_staged_rollout_path;
use publish::migration_journal_path;
use publish::pending_migration_thread_ids;
use publish::remove_file_if_present;
use publish::rewrite_subagent_history_boundary;
use publish::rewritten_staged_rollout_path;
use publish::staged_rollout_path;
use publish::sync_parent_directory;
use publish::write_migration_journal;
use rollback_plan::RollbackPlan;
use rollback_plan::RollbackPlanner;
use telemetry::RolloutMigrationTelemetry;
use telemetry::RolloutMigrationTrigger;

const PROJECTION_BATCH_BYTES: u64 = 256 * 1024;
const MAX_ROLLOUT_LINE_BYTES: usize = 16 * 1024 * 1024;

enum CanonicalizationAttempt {
    Complete {
        expected_length: u64,
        expected_ordinal: u64,
    },
    NeedsRollbackPlan,
}

struct CanonicalizationSource<'a> {
    thread_id: ThreadId,
    source_path: &'a Path,
    staged_path: &'a Path,
    source_permissions: &'a std::fs::Permissions,
    canonical_session_meta: &'a RolloutLine,
}

/// Controls whether eligible rollouts are reported or migrated.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RolloutMigrationMode {
    /// Report eligible files without modifying local storage.
    #[default]
    DryRun,
    /// Publish paginated rollouts and materialize their SQLite history.
    Apply,
}

/// Selection and throughput limits for a local rollout migration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RolloutMigrationOptions {
    pub mode: RolloutMigrationMode,
    pub thread_ids: Vec<ThreadId>,
    /// Optional aggregate rollout I/O limit. Without one, migration runs as fast as local I/O
    /// allows while still yielding between projection-sized batches.
    pub max_mib_per_second: Option<u64>,
}

impl Default for RolloutMigrationOptions {
    fn default() -> Self {
        Self {
            mode: RolloutMigrationMode::DryRun,
            thread_ids: Vec::new(),
            max_mib_per_second: None,
        }
    }
}

/// The observable result of inspecting one local rollout.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RolloutMigrationStatus {
    Eligible,
    Migrated,
    AlreadyPaginated,
    SkippedEmpty,
    SkippedBusy,
    Failed,
}

/// The per-thread result of a rollout migration run.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RolloutMigrationOutcome {
    pub thread_id: Option<ThreadId>,
    pub rollout_path: PathBuf,
    pub status: RolloutMigrationStatus,
    pub bytes_processed: u64,
    pub message: Option<String>,
}

/// The complete result of scanning active rollout files.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct RolloutMigrationReport {
    pub outcomes: Vec<RolloutMigrationOutcome>,
}

/// Incremental progress emitted while rollout migration scans discovered paths.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RolloutMigrationProgress {
    pub processed_paths: usize,
    pub total_paths: usize,
    /// None means the scanned path was excluded by the requested thread filter.
    pub outcome_status: Option<RolloutMigrationStatus>,
}

struct RolloutMigrationRateLimiter {
    started_at: Instant,
    bytes_processed: u64,
    bytes_per_second: Option<u64>,
    bytes_since_yield: u64,
}

struct RolloutRecord {
    line: Option<RolloutLine>,
    byte_count: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RolloutMigrationKind {
    Ordinary,
    Subagent,
}

impl RolloutMigrationRateLimiter {
    fn new(max_mib_per_second: Option<u64>) -> ThreadStoreResult<Self> {
        let bytes_per_second = max_mib_per_second
            .map(|rate| {
                rate.checked_mul(1024 * 1024)
                    .filter(|rate| *rate > 0)
                    .ok_or_else(|| ThreadStoreError::InvalidRequest {
                        message: "--max-mib-per-second must be a positive supported integer"
                            .to_string(),
                    })
            })
            .transpose()?;
        Ok(Self {
            started_at: Instant::now(),
            bytes_processed: 0,
            bytes_per_second,
            bytes_since_yield: 0,
        })
    }

    async fn account(&mut self, bytes: u64) {
        self.bytes_processed = self.bytes_processed.saturating_add(bytes);
        self.bytes_since_yield = self.bytes_since_yield.saturating_add(bytes);
        if self.bytes_since_yield < PROJECTION_BATCH_BYTES {
            return;
        }
        self.bytes_since_yield = 0;
        let Some(bytes_per_second) = self.bytes_per_second else {
            tokio::task::yield_now().await;
            return;
        };
        let expected =
            Duration::from_secs_f64(self.bytes_processed as f64 / bytes_per_second as f64);
        if let Some(remaining) = expected.checked_sub(self.started_at.elapsed()) {
            tokio::time::sleep(remaining).await;
        } else {
            tokio::task::yield_now().await;
        }
    }
}

impl LocalThreadStore {
    /// Check whether startup needs to migrate legacy rollouts, then migrate in the background
    /// when it does.
    pub async fn migrate_rollouts_on_startup(&self) -> ThreadStoreResult<()> {
        startup::migrate_rollouts_on_startup(self).await
    }

    /// Inspect or migrate eligible legacy rollout files beneath active and archived sessions.
    pub async fn migrate_rollouts(
        &self,
        options: RolloutMigrationOptions,
    ) -> ThreadStoreResult<RolloutMigrationReport> {
        self.migrate_rollouts_with_progress_for_trigger(
            options,
            |_| {},
            RolloutMigrationTrigger::Manual,
        )
        .await
    }

    /// Inspect or migrate rollouts while reporting each discovered path after it is processed.
    pub async fn migrate_rollouts_with_progress(
        &self,
        options: RolloutMigrationOptions,
        on_progress: impl FnMut(RolloutMigrationProgress),
    ) -> ThreadStoreResult<RolloutMigrationReport> {
        self.migrate_rollouts_with_progress_for_trigger(
            options,
            on_progress,
            RolloutMigrationTrigger::Manual,
        )
        .await
    }

    async fn migrate_rollouts_with_progress_for_trigger(
        &self,
        options: RolloutMigrationOptions,
        mut on_progress: impl FnMut(RolloutMigrationProgress),
        trigger: RolloutMigrationTrigger,
    ) -> ThreadStoreResult<RolloutMigrationReport> {
        let telemetry = RolloutMigrationTelemetry::new(trigger, &options);
        let result = self
            .migrate_rollouts_with_progress_inner(options, &mut on_progress)
            .await;
        telemetry.finish(&result);
        result
    }

    async fn migrate_rollouts_with_progress_inner(
        &self,
        options: RolloutMigrationOptions,
        on_progress: &mut impl FnMut(RolloutMigrationProgress),
    ) -> ThreadStoreResult<RolloutMigrationReport> {
        let mut limiter = RolloutMigrationRateLimiter::new(options.max_mib_per_second)?;
        let _maintenance_guard = match options.mode {
            RolloutMigrationMode::DryRun => None,
            RolloutMigrationMode::Apply => Some(
                codex_rollout::try_acquire_rollout_maintenance_lock(&self.config.codex_home)
                    .map_err(migration_error)?
                    .ok_or_else(|| ThreadStoreError::Conflict {
                        message: "rollout compression or another migration is already running"
                            .to_string(),
                    })?,
            ),
        };
        let mut paths =
            find_rollout_paths(&self.config.codex_home.join(codex_rollout::SESSIONS_SUBDIR))
                .await?;
        paths.extend(
            find_rollout_paths(
                &self
                    .config
                    .codex_home
                    .join(codex_rollout::ARCHIVED_SESSIONS_SUBDIR),
            )
            .await?,
        );
        if options.mode == RolloutMigrationMode::Apply {
            let pending_thread_ids = pending_migration_thread_ids(&self.config.codex_home).await?;
            paths.sort_by_key(|path| {
                !thread_id_from_rollout_filename(path)
                    .is_some_and(|thread_id| pending_thread_ids.contains(&thread_id))
            });
        }
        let total_paths = paths.len();
        let mut report = RolloutMigrationReport::default();

        for (index, path) in paths.into_iter().enumerate() {
            let outcome = self
                .migrate_rollout_path(path, &options, &mut limiter)
                .await?;
            let outcome_status = outcome.as_ref().map(|outcome| outcome.status);
            if let Some(outcome) = outcome {
                report.outcomes.push(outcome);
            }
            on_progress(RolloutMigrationProgress {
                processed_paths: index + 1,
                total_paths,
                outcome_status,
            });
        }

        Ok(report)
    }

    async fn migrate_rollout_path(
        &self,
        mut path: PathBuf,
        options: &RolloutMigrationOptions,
        limiter: &mut RolloutMigrationRateLimiter,
    ) -> ThreadStoreResult<Option<RolloutMigrationOutcome>> {
        let metadata = match codex_rollout::read_session_meta_line(&path).await {
            Ok(metadata) => metadata,
            Err(error) => {
                let thread_id = thread_id_from_rollout_filename(&path);
                if !matches_selection(&options.thread_ids, thread_id) {
                    return Ok(None);
                }
                let empty = tokio::fs::metadata(&path)
                    .await
                    .is_ok_and(|metadata| metadata.len() == 0);
                return Ok(Some(RolloutMigrationOutcome {
                    thread_id,
                    rollout_path: path,
                    status: if empty {
                        RolloutMigrationStatus::SkippedEmpty
                    } else {
                        RolloutMigrationStatus::Failed
                    },
                    bytes_processed: 0,
                    message: (!empty).then(|| error.to_string()),
                }));
            }
        };
        let thread_id = metadata.meta.id;
        if !matches_selection(&options.thread_ids, Some(thread_id)) {
            return Ok(None);
        }
        let is_memory_consolidation = matches!(
            metadata.meta.source,
            SessionSource::Internal(InternalSessionSource::MemoryConsolidation)
                | SessionSource::SubAgent(SubAgentSource::MemoryConsolidation)
        ) || matches!(
            metadata.meta.thread_source,
            Some(ThreadSource::MemoryConsolidation)
        );
        let kind = if !is_memory_consolidation
            && (matches!(metadata.meta.source, SessionSource::SubAgent(_))
                || matches!(metadata.meta.thread_source, Some(ThreadSource::Subagent)))
        {
            RolloutMigrationKind::Subagent
        } else {
            RolloutMigrationKind::Ordinary
        };

        let journal_path = migration_journal_path(&self.config.codex_home, thread_id);
        let pending_published_migration = metadata.meta.history_mode
            == ThreadHistoryMode::Paginated
            && options.mode == RolloutMigrationMode::Apply
            && tokio::fs::try_exists(&journal_path)
                .await
                .map_err(migration_error)?;
        if metadata.meta.history_mode == ThreadHistoryMode::Paginated {
            let bytes_before = limiter.bytes_processed;
            let result = if pending_published_migration {
                let _live_writer_guard = self.live_writer_locks.lock(thread_id).await;
                match self
                    .recover_published_migration(thread_id, &path, &journal_path, limiter)
                    .await
                {
                    Ok(recovered_path) => {
                        path = recovered_path;
                        Ok(RolloutMigrationStatus::Migrated)
                    }
                    Err(ThreadStoreError::Conflict { message }) => {
                        let bytes_processed = limiter.bytes_processed.saturating_sub(bytes_before);
                        return Ok(Some(skipped_busy_outcome(
                            thread_id,
                            path,
                            message,
                            bytes_processed,
                        )));
                    }
                    Err(error) => Err(error),
                }
            } else {
                Ok(RolloutMigrationStatus::AlreadyPaginated)
            };
            let bytes_processed = limiter.bytes_processed.saturating_sub(bytes_before);
            return Ok(Some(migration_outcome(
                thread_id,
                path,
                result,
                bytes_processed,
            )));
        }

        if options.mode == RolloutMigrationMode::DryRun {
            return Ok(Some(migration_outcome(
                thread_id,
                path,
                Ok(RolloutMigrationStatus::Eligible),
                /*bytes_processed*/ 0,
            )));
        }

        let _live_writer_guard = self.live_writer_locks.lock(thread_id).await;
        let _writer_guard = match self.writer_lock_coordinator.acquire(thread_id) {
            Ok(guard) => guard,
            Err(ThreadStoreError::Conflict { message }) => {
                return Ok(Some(skipped_busy_outcome(
                    thread_id, path, message, /*bytes_processed*/ 0,
                )));
            }
            Err(error) => {
                return Ok(Some(migration_outcome(
                    thread_id,
                    path,
                    Err(error),
                    /*bytes_processed*/ 0,
                )));
            }
        };
        let bytes_before = limiter.bytes_processed;
        let result = match self
            .migrate_one_rollout(thread_id, &path, &journal_path, kind, limiter)
            .await
        {
            Ok(()) => Ok(RolloutMigrationStatus::Migrated),
            Err(error) => {
                if let Err(cleanup_error) = self
                    .cleanup_failed_unpublished_migration(thread_id, &path, &journal_path)
                    .await
                {
                    Err(migration_error(format!(
                        "{error}; failed to clean up unpublished migration: {cleanup_error}"
                    )))
                } else {
                    Err(error)
                }
            }
        };
        let bytes_processed = limiter.bytes_processed.saturating_sub(bytes_before);
        Ok(Some(match result {
            Err(ThreadStoreError::Conflict { message }) => {
                skipped_busy_outcome(thread_id, path, message, bytes_processed)
            }
            result => migration_outcome(thread_id, path, result, bytes_processed),
        }))
    }

    async fn migrate_one_rollout(
        &self,
        thread_id: ThreadId,
        rollout_path: &Path,
        journal_path: &Path,
        kind: RolloutMigrationKind,
        limiter: &mut RolloutMigrationRateLimiter,
    ) -> ThreadStoreResult<()> {
        if let Some(state_db) = &self.state_db
            && state_db
                .get_thread(thread_id)
                .await
                .map_err(migration_error)?
                .is_none()
        {
            return Err(migration_error(format!(
                "thread {thread_id} is missing its SQLite metadata"
            )));
        }

        let compressed = rollout_path_is_compressed(rollout_path);
        let staged_path = staged_rollout_path(rollout_path)?;
        let decompressed_path = compressed
            .then(|| decompressed_staged_rollout_path(rollout_path))
            .transpose()?;
        thread_history::delete_thread(self, thread_id).await?;
        write_migration_journal(journal_path).await?;

        let source_metadata = tokio::fs::metadata(rollout_path)
            .await
            .map_err(migration_error)?;
        let source_modified = source_metadata.modified().ok();
        let source_permissions = source_metadata.permissions();
        let source_path = if let Some(decompressed_path) = decompressed_path.as_ref() {
            decompress_rollout_to_path(rollout_path, decompressed_path).await?;
            let decompressed_bytes = tokio::fs::metadata(decompressed_path)
                .await
                .map_err(migration_error)?
                .len();
            limiter
                .account(source_metadata.len().saturating_add(decompressed_bytes))
                .await;
            decompressed_path.as_path()
        } else {
            rollout_path
        };
        let source_file = File::open(source_path).await.map_err(migration_error)?;
        let mut source = BufReader::with_capacity(PROJECTION_BATCH_BYTES as usize, source_file);
        let mut bytes = Vec::new();

        // Paginated rollouts always keep their canonical SessionMeta at ordinal zero. Legacy
        // readers tolerate a pre-header prefix, so find that metadata before replaying the source
        // instead of buffering the prefix in memory.
        let canonical_session_meta = loop {
            let record = read_rollout_record(&mut source, &mut bytes)
                .await?
                .ok_or_else(|| migration_error("rollout contains no session metadata"))?;
            limiter.account(record.byte_count).await;
            let Some(line) = record.line else {
                continue;
            };
            if matches!(&line.item, RolloutItem::SessionMeta(_)) {
                break line;
            }
        };
        drop(source);

        let canonicalization_source = CanonicalizationSource {
            thread_id,
            source_path,
            staged_path: &staged_path,
            source_permissions: &source_permissions,
            canonical_session_meta: &canonical_session_meta,
        };
        let bounded_subagent_context = if kind == RolloutMigrationKind::Subagent {
            let RolloutItem::SessionMeta(session_meta) = &canonical_session_meta.item else {
                return Err(migration_error("canonical session metadata is missing"));
            };
            let context =
                subagent::select_bounded_context(source_path.to_path_buf(), session_meta.clone())
                    .await?;
            limiter.account(source_metadata.len()).await;
            context
        } else {
            None
        };
        let (_, expected_ordinal) = if let Some(items) = bounded_subagent_context {
            Self::write_bounded_subagent_rollout(&canonicalization_source, items, limiter).await?
        } else {
            Self::write_rollout_with_rollback_plan(&canonicalization_source, limiter).await?
        };

        if kind == RolloutMigrationKind::Subagent {
            rewrite_subagent_history_boundary(&staged_path, expected_ordinal).await?;
        }
        let expected_length = tokio::fs::metadata(&staged_path)
            .await
            .map_err(migration_error)?
            .len();

        // SQLite projection only starts after every staged-file mutation is durable.
        let modified_at = source_modified;
        let path = staged_path.clone();
        tokio::task::spawn_blocking(move || {
            let file = std::fs::OpenOptions::new().write(true).open(path)?;
            if let Some(modified_at) = modified_at {
                file.set_times(std::fs::FileTimes::new().set_modified(modified_at))?;
            }
            file.sync_all()
        })
        .await
        .map_err(migration_error)?
        .map_err(migration_error)?;

        self.project_rollout_in_batches(thread_id, &staged_path, limiter)
            .await?;
        let projection = thread_history::projection_state(self, thread_id)
            .await?
            .ok_or_else(|| migration_error("completed rollout has no SQLite projection"))?;
        if projection.next_byte_offset != expected_length
            || projection.next_ordinal != expected_ordinal
        {
            return Err(migration_error(
                "SQLite projection does not cover the complete staged rollout",
            ));
        }

        let compressed_staged_path = if compressed {
            let path = compressed_staged_rollout_path(rollout_path)?;
            compress_rollout_to_path(&staged_path, &path, source_permissions, source_modified)
                .await?;
            Some(path)
        } else {
            None
        };

        // Older writers do not know about migration locks. Keep the legacy path visible if its
        // append-only source changed while the replacement rollout was being staged.
        let current_source_metadata = tokio::fs::metadata(rollout_path)
            .await
            .map_err(migration_error)?;
        if current_source_metadata.len() != source_metadata.len()
            || current_source_metadata.modified().ok() != source_modified
        {
            return Err(ThreadStoreError::Conflict {
                message: "rollout changed while migration was staging it; close older Codex processes and retry".to_string(),
            });
        }

        if let Some(compressed_staged_path) = compressed_staged_path {
            tokio::fs::rename(compressed_staged_path, rollout_path)
                .await
                .map_err(migration_error)?;
            remove_file_if_present(&staged_path).await?;
            if let Some(decompressed_path) = decompressed_path.as_ref() {
                remove_file_if_present(decompressed_path).await?;
            }
        } else {
            tokio::fs::rename(&staged_path, rollout_path)
                .await
                .map_err(migration_error)?;
        }
        sync_parent_directory(rollout_path).await?;
        self.finish_published_migration(thread_id, journal_path)
            .await
    }

    async fn build_rollback_plan(
        source_path: &Path,
        limiter: &mut RolloutMigrationRateLimiter,
    ) -> ThreadStoreResult<RollbackPlan> {
        let source_file = File::open(source_path).await.map_err(migration_error)?;
        let mut source = BufReader::with_capacity(PROJECTION_BATCH_BYTES as usize, source_file);
        let mut bytes = Vec::new();
        let mut planner = RollbackPlanner::new();
        while let Some(record) = read_rollout_record(&mut source, &mut bytes).await? {
            limiter.account(record.byte_count).await;
            if let Some(line) = record.line {
                planner.observe(&line)?;
            }
        }
        Ok(planner.finish())
    }

    async fn write_rollout_with_rollback_plan(
        input: &CanonicalizationSource<'_>,
        limiter: &mut RolloutMigrationRateLimiter,
    ) -> ThreadStoreResult<(u64, u64)> {
        let attempt = Self::write_canonical_rollout(input, /*plan*/ None, limiter).await?;
        match attempt {
            CanonicalizationAttempt::Complete {
                expected_length,
                expected_ordinal,
            } => Ok((expected_length, expected_ordinal)),
            CanonicalizationAttempt::NeedsRollbackPlan => {
                remove_file_if_present(input.staged_path).await?;
                let plan = Self::build_rollback_plan(input.source_path, limiter).await?;
                let CanonicalizationAttempt::Complete {
                    expected_length,
                    expected_ordinal,
                } = Self::write_canonical_rollout(input, Some(&plan), limiter).await?
                else {
                    return Err(migration_error(
                        "planned rollout still contains a rollback marker",
                    ));
                };
                Ok((expected_length, expected_ordinal))
            }
        }
    }

    async fn write_bounded_subagent_rollout(
        input: &CanonicalizationSource<'_>,
        items: Vec<RolloutItem>,
        limiter: &mut RolloutMigrationRateLimiter,
    ) -> ThreadStoreResult<(u64, u64)> {
        let staged_file = tokio::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(input.staged_path)
            .await
            .map_err(migration_error)?;
        staged_file
            .set_permissions(input.source_permissions.clone())
            .await
            .map_err(migration_error)?;
        let mut staged = BufWriter::with_capacity(PROJECTION_BATCH_BYTES as usize, staged_file);
        let mut canonicalizer = LegacyRolloutCanonicalizer::new(input.thread_id);
        let written = canonicalizer
            .write_head_session_meta(input.canonical_session_meta.clone(), &mut staged)
            .await?;
        limiter.account(written).await;
        for item in items {
            let line = RolloutLine {
                timestamp: input.canonical_session_meta.timestamp.clone(),
                ordinal: None,
                item,
            };
            let written = canonicalizer.process_line(line, &mut staged).await?;
            limiter.account(written).await;
        }
        let written = canonicalizer
            .finish(&mut staged, &input.canonical_session_meta.timestamp)
            .await?;
        limiter.account(written).await;
        staged.flush().await.map_err(migration_error)?;
        Ok((
            canonicalizer.output_byte_offset(),
            canonicalizer.next_ordinal(),
        ))
    }

    async fn write_canonical_rollout(
        input: &CanonicalizationSource<'_>,
        plan: Option<&RollbackPlan>,
        limiter: &mut RolloutMigrationRateLimiter,
    ) -> ThreadStoreResult<CanonicalizationAttempt> {
        let source_file = File::open(input.source_path)
            .await
            .map_err(migration_error)?;
        let staged_file = tokio::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(input.staged_path)
            .await
            .map_err(migration_error)?;
        staged_file
            .set_permissions(input.source_permissions.clone())
            .await
            .map_err(migration_error)?;
        let mut source = BufReader::with_capacity(PROJECTION_BATCH_BYTES as usize, source_file);
        let mut staged = BufWriter::with_capacity(PROJECTION_BATCH_BYTES as usize, staged_file);
        let mut bytes = Vec::new();
        let mut parsed_record_index = 0_usize;
        let mut canonicalizer = LegacyRolloutCanonicalizer::new(input.thread_id);
        let written = canonicalizer
            .write_head_session_meta(input.canonical_session_meta.clone(), &mut staged)
            .await?;
        limiter.account(written).await;
        let mut last_timestamp = input.canonical_session_meta.timestamp.clone();

        while let Some(record) = read_rollout_record(&mut source, &mut bytes).await? {
            limiter.account(record.byte_count).await;
            let Some(line) = record.line else {
                continue;
            };
            if plan.is_none()
                && matches!(
                    &line.item,
                    RolloutItem::EventMsg(codex_protocol::protocol::EventMsg::ThreadRolledBack(_))
                )
            {
                return Ok(CanonicalizationAttempt::NeedsRollbackPlan);
            }
            let line = if let Some(plan) = plan {
                let planned = plan.apply(parsed_record_index, line)?;
                parsed_record_index = parsed_record_index
                    .checked_add(1)
                    .ok_or_else(|| migration_error("legacy rollout record index overflow"))?;
                let Some(line) = planned else {
                    continue;
                };
                line
            } else {
                line
            };
            last_timestamp = line.timestamp.clone();
            let written = canonicalizer.process_line(line, &mut staged).await?;
            limiter.account(written).await;
        }
        if let Some(plan) = plan
            && parsed_record_index != plan.record_count()
        {
            return Err(migration_error(
                "rollback plan source length changed during replay",
            ));
        }

        let written = canonicalizer.finish(&mut staged, &last_timestamp).await?;
        limiter.account(written).await;
        staged.flush().await.map_err(migration_error)?;
        Ok(CanonicalizationAttempt::Complete {
            expected_length: canonicalizer.output_byte_offset(),
            expected_ordinal: canonicalizer.next_ordinal(),
        })
    }

    async fn recover_published_migration(
        &self,
        thread_id: ThreadId,
        rollout_path: &Path,
        journal_path: &Path,
        limiter: &mut RolloutMigrationRateLimiter,
    ) -> ThreadStoreResult<PathBuf> {
        let _writer_guard = self.writer_lock_coordinator.acquire(thread_id)?;
        let decompressed_path = rollout_path_is_compressed(rollout_path)
            .then(|| decompressed_staged_rollout_path(rollout_path))
            .transpose()?;
        let projection_path = if let Some(decompressed_path) = decompressed_path.as_ref() {
            decompress_rollout_to_path(rollout_path, decompressed_path).await?;
            let decompressed_bytes = tokio::fs::metadata(decompressed_path)
                .await
                .map_err(migration_error)?
                .len();
            limiter
                .account(
                    tokio::fs::metadata(rollout_path)
                        .await
                        .map_err(migration_error)?
                        .len()
                        .saturating_add(decompressed_bytes),
                )
                .await;
            decompressed_path.as_path()
        } else {
            rollout_path
        };
        let expected_length = tokio::fs::metadata(projection_path)
            .await
            .map_err(migration_error)?
            .len();
        let projection = thread_history::projection_state(self, thread_id).await?;
        if projection.is_none_or(|state| state.next_byte_offset != expected_length) {
            thread_history::delete_thread(self, thread_id).await?;
            self.project_rollout_in_batches(thread_id, projection_path, limiter)
                .await?;
        }
        remove_file_if_present(&staged_rollout_path(rollout_path)?).await?;
        remove_file_if_present(&compressed_staged_rollout_path(rollout_path)?).await?;
        if let Some(decompressed_path) = decompressed_path.as_ref() {
            remove_file_if_present(decompressed_path).await?;
        }
        self.finish_published_migration(thread_id, journal_path)
            .await?;
        Ok(rollout_path.to_path_buf())
    }

    async fn cleanup_failed_unpublished_migration(
        &self,
        thread_id: ThreadId,
        rollout_path: &Path,
        journal_path: &Path,
    ) -> ThreadStoreResult<()> {
        let Ok(metadata) = codex_rollout::read_session_meta_line(rollout_path).await else {
            return Ok(());
        };
        if metadata.meta.history_mode == ThreadHistoryMode::Paginated {
            return Ok(());
        }

        thread_history::delete_thread(self, thread_id).await?;
        let staged_path = staged_rollout_path(rollout_path)?;
        remove_file_if_present(&staged_path).await?;
        remove_file_if_present(&rewritten_staged_rollout_path(&staged_path)?).await?;
        remove_file_if_present(&compressed_staged_rollout_path(rollout_path)?).await?;
        remove_file_if_present(&decompressed_staged_rollout_path(rollout_path)?).await?;
        remove_file_if_present(journal_path).await?;
        sync_parent_directory(journal_path).await
    }

    async fn finish_published_migration(
        &self,
        thread_id: ThreadId,
        journal_path: &Path,
    ) -> ThreadStoreResult<()> {
        if let Some(state_db) = &self.state_db
            && !state_db
                .mark_thread_paginated(thread_id)
                .await
                .map_err(migration_error)?
        {
            return Err(migration_error(format!(
                "thread {thread_id} is missing its SQLite metadata"
            )));
        }
        tokio::fs::remove_file(journal_path)
            .await
            .map_err(migration_error)?;
        sync_parent_directory(journal_path).await
    }

    async fn project_rollout_in_batches(
        &self,
        thread_id: ThreadId,
        rollout_path: &Path,
        limiter: &mut RolloutMigrationRateLimiter,
    ) -> ThreadStoreResult<()> {
        let subagent_history_start_ordinal = codex_rollout::read_session_meta_line(rollout_path)
            .await
            .map_err(migration_error)?
            .meta
            .subagent_history_start_ordinal;
        let file = File::open(rollout_path).await.map_err(migration_error)?;
        let mut reader = BufReader::with_capacity(PROJECTION_BATCH_BYTES as usize, file);
        let mut line_bytes = Vec::new();
        let mut batch = Vec::new();
        let mut batch_start = 0_u64;
        let mut offset = 0_u64;

        while let Some(record) = read_rollout_record(&mut reader, &mut line_bytes).await? {
            let next_offset = offset
                .checked_add(record.byte_count)
                .ok_or_else(|| migration_error("staged rollout byte offset overflow"))?;
            limiter.account(record.byte_count).await;
            let Some(line) = record.line else {
                offset = next_offset;
                continue;
            };
            let ordinal = line
                .ordinal
                .ok_or_else(|| migration_error("staged rollout line is missing its ordinal"))?;
            let fallback_created_at_ms = DateTime::parse_from_rfc3339(&line.timestamp)
                .map_err(migration_error)?
                .timestamp_millis();
            batch.push(RolloutProjectionStep::Line(ProjectedRolloutLine {
                ordinal,
                start_byte_offset: offset,
                end_byte_offset: next_offset,
                fallback_created_at_ms: Some(fallback_created_at_ms),
                changes: if subagent_history_start_ordinal.is_some_and(|start| ordinal < start) {
                    Default::default()
                } else {
                    project_rollout_line(&line)
                },
            }));
            offset = next_offset;

            if offset.saturating_sub(batch_start) >= PROJECTION_BATCH_BYTES {
                thread_history::apply_projection(
                    self,
                    thread_id,
                    batch_start,
                    offset,
                    /*initial_ordinal*/ 0,
                    std::mem::take(&mut batch),
                )
                .await?;
                batch_start = offset;
            }
        }

        if batch_start != offset {
            thread_history::apply_projection(
                self,
                thread_id,
                batch_start,
                offset,
                /*initial_ordinal*/ 0,
                batch,
            )
            .await?;
        }
        Ok(())
    }
}

async fn read_rollout_record(
    reader: &mut BufReader<File>,
    bytes: &mut Vec<u8>,
) -> ThreadStoreResult<Option<RolloutRecord>> {
    bytes.clear();
    let mut byte_count = reader
        .take((MAX_ROLLOUT_LINE_BYTES + 1) as u64)
        .read_until(b'\n', bytes)
        .await
        .map_err(migration_error)?;
    if byte_count == 0 {
        return Ok(None);
    }
    if byte_count > MAX_ROLLOUT_LINE_BYTES {
        // Some historical tool outputs are too large to migrate safely in memory. Discard the
        // whole record, including any unread suffix.
        while bytes.last() != Some(&b'\n') {
            bytes.clear();
            let chunk_bytes = reader
                .take((MAX_ROLLOUT_LINE_BYTES + 1) as u64)
                .read_until(b'\n', bytes)
                .await
                .map_err(migration_error)?;
            if chunk_bytes == 0 {
                break;
            }
            byte_count = byte_count
                .checked_add(chunk_bytes)
                .ok_or_else(|| migration_error("rollout record byte count overflow"))?;
        }
        return Ok(Some(RolloutRecord {
            line: None,
            byte_count: byte_count as u64,
        }));
    }
    // Legacy records do not have ordinals, so malformed complete records cannot be repaired.
    // Skip them and let the next newline-delimited record resynchronize the stream.
    let line = line_parser::parse_legacy_rollout_line(bytes).unwrap_or(None);
    Ok(Some(RolloutRecord {
        line,
        byte_count: byte_count as u64,
    }))
}

async fn find_rollout_paths(root: &Path) -> ThreadStoreResult<Vec<PathBuf>> {
    let mut directories = vec![root.to_path_buf()];
    let mut paths = Vec::new();

    while let Some(directory) = directories.pop() {
        let mut entries = match tokio::fs::read_dir(&directory).await {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(migration_error(error)),
        };
        while let Some(entry) = entries.next_entry().await.map_err(migration_error)? {
            let kind = entry.file_type().await.map_err(migration_error)?;
            if kind.is_dir() {
                directories.push(entry.path());
                continue;
            }
            if !kind.is_file() {
                continue;
            }
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if name.starts_with("rollout-")
                && (name.ends_with(".jsonl") || name.ends_with(".jsonl.zst"))
            {
                let path = entry.path();
                if name.ends_with(".jsonl.zst")
                    && tokio::fs::try_exists(codex_rollout::plain_rollout_path(&path))
                        .await
                        .map_err(migration_error)?
                {
                    continue;
                }
                paths.push(path);
            }
        }
    }

    paths.sort_by(|left, right| right.cmp(left));
    Ok(paths)
}

fn matches_selection(selected: &[ThreadId], actual: Option<ThreadId>) -> bool {
    selected.is_empty() || actual.is_some_and(|thread_id| selected.contains(&thread_id))
}

fn rollout_path_is_compressed(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".jsonl.zst"))
}

fn thread_id_from_rollout_filename(path: &Path) -> Option<ThreadId> {
    let name = path.file_name()?.to_str()?;
    let stem = name
        .strip_suffix(".jsonl.zst")
        .or_else(|| name.strip_suffix(".jsonl"))?;
    let start = stem.len().checked_sub(36)?;
    ThreadId::from_string(stem.get(start..)?).ok()
}

fn migration_outcome(
    thread_id: ThreadId,
    rollout_path: PathBuf,
    result: ThreadStoreResult<RolloutMigrationStatus>,
    bytes_processed: u64,
) -> RolloutMigrationOutcome {
    match result {
        Ok(status) => RolloutMigrationOutcome {
            thread_id: Some(thread_id),
            rollout_path,
            status,
            bytes_processed,
            message: None,
        },
        Err(error) => RolloutMigrationOutcome {
            thread_id: Some(thread_id),
            rollout_path,
            status: RolloutMigrationStatus::Failed,
            bytes_processed,
            message: Some(error.to_string()),
        },
    }
}

fn skipped_busy_outcome(
    thread_id: ThreadId,
    rollout_path: PathBuf,
    message: String,
    bytes_processed: u64,
) -> RolloutMigrationOutcome {
    RolloutMigrationOutcome {
        thread_id: Some(thread_id),
        rollout_path,
        status: RolloutMigrationStatus::SkippedBusy,
        bytes_processed,
        message: Some(message),
    }
}

fn migration_error(error: impl std::fmt::Display) -> ThreadStoreError {
    ThreadStoreError::Internal {
        message: format!("rollout migration failed: {error}"),
    }
}

#[cfg(test)]
#[path = "rollout_migration_tests.rs"]
mod tests;
