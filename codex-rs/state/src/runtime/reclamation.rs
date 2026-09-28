//! Reclaims unused SQLite pages in opted-in databases, reducing batches after stalled passes.
use crate::RuntimeDbPath;
use crate::SqliteConfig;
use log::LevelFilter;
use sqlx::ConnectOptions;
use sqlx::Connection;
use sqlx::SqliteConnection;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqliteSynchronous;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::watch;

const IDLE_INTERVAL: Duration = Duration::from_secs(60);
const ACTIVE_INTERVAL: Duration = Duration::from_millis(50);
const CONTENTION_INTERVAL: Duration = Duration::from_millis(500);
const PASS_DURATION: Duration = Duration::from_millis(100);
const MIN_FREE_BYTES: i64 = 64 * 1024 * 1024;
const RESERVE_BYTES: i64 = 16 * 1024 * 1024;
const PASS_PAGES: u32 = 1024;
const BATCH_PAGES: u32 = 64;

/// Owns background reclamation for opted-in databases registered with `SqliteConfig`.
///
/// A per-home file lock elects one active worker, which visits databases serially
/// on individual retry schedules. Shutdown waits for the dedicated SQLite
/// connection to close before the worker releases the lock.
pub(crate) struct SqliteReclamationWorker {
    shutdown: watch::Sender<()>,
    finished: watch::Receiver<()>,
}

impl SqliteReclamationWorker {
    pub(crate) fn spawn(sqlite: SqliteConfig) -> Arc<Self> {
        let (shutdown, mut receiver) = watch::channel(());
        let (finish, finished) = watch::channel(());
        tokio::spawn(async move {
            // Declared first so completion is signaled after all worker resources drop.
            let _finish = finish;
            let mut scheduled = sqlite
                .runtime_db_paths()
                .into_iter()
                .filter_map(|db| {
                    if db.background_reclamation {
                        Some((db, Instant::now(), ReclamationState::default()))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            let mut owner = None;
            let mut delay = IDLE_INTERVAL;
            loop {
                tokio::select! {
                    _ = receiver.changed() => break,
                    _ = tokio::time::sleep(delay) => {}
                }
                if owner.is_none() {
                    owner = try_ownership(sqlite.home()).await.ok().flatten();
                }
                if owner.is_none() {
                    continue;
                }
                delay = visit(&mut scheduled, &receiver).await;
            }
        });

        Arc::new(Self { shutdown, finished })
    }

    pub(crate) async fn close(&self) {
        self.shutdown.send_replace(());
        // Every caller waits for task completion, even if another close is canceled.
        let _ = self.finished.clone().changed().await;
    }
}

async fn visit(
    scheduled: &mut [(RuntimeDbPath, Instant, ReclamationState)],
    shutdown: &watch::Receiver<()>,
) -> Duration {
    for (db, due, state) in &mut *scheduled {
        let started = Instant::now();
        if shutdown.has_changed().unwrap_or(true) {
            break;
        }
        if *due > started {
            continue;
        }
        let result = reclaim(
            &db.path,
            ReclamationOptions {
                budget: Budget {
                    deadline: Some(started + PASS_DURATION),
                    pages: PASS_PAGES,
                },
                batch_pages: state.batch_pages,
            },
            shutdown,
        )
        .await;
        crate::telemetry::record_reclamation(db.label, started.elapsed(), &result);
        *due = Instant::now() + state.retry_after(&result);
    }
    scheduled
        .iter()
        .map(|(_, due, _)| due.saturating_duration_since(Instant::now()))
        .min()
        .unwrap_or(IDLE_INTERVAL)
}

async fn try_ownership(home: &Path) -> std::io::Result<Option<File>> {
    let lock = tokio::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(home.join(".sqlite-maintenance.lock"))
        .await?
        .into_std()
        .await;
    match lock.try_lock() {
        Ok(()) => Ok(Some(lock)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(error)) => Err(error),
    }
}

pub(crate) struct ReclamationPass {
    pub(crate) pages: u32,
    outcome: PassOutcome,
}

#[derive(Debug, PartialEq, Eq)]
enum PassOutcome {
    Idle,
    Active,
    Contended,
    Interrupted,
    Shutdown,
}

#[derive(Debug, PartialEq, Eq)]
struct ReclamationState {
    batch_pages: u32,
    stalled_passes: u32,
}

impl Default for ReclamationState {
    fn default() -> Self {
        Self {
            batch_pages: BATCH_PAGES,
            stalled_passes: 0,
        }
    }
}

impl ReclamationState {
    fn retry_after(&mut self, result: &anyhow::Result<ReclamationPass>) -> Duration {
        if let Ok(pass) = result
            && pass.outcome == PassOutcome::Interrupted
            && pass.pages == 0
        {
            self.stalled_passes = self.stalled_passes.saturating_add(1);
            self.batch_pages = (self.batch_pages / 2).max(1);

            // exponentially backoffs from CONTENTION_INTERVAL to IDLE_INTERVAL (500ms -> 60s)
            return (CONTENTION_INTERVAL * (1 << (self.stalled_passes - 1).min(7)))
                .min(IDLE_INTERVAL);
        }

        // Keep the smaller batch after progress; growing it would repeat the stall.
        self.stalled_passes = 0;
        match result {
            Ok(pass) => match pass.outcome {
                PassOutcome::Idle | PassOutcome::Shutdown => IDLE_INTERVAL,
                PassOutcome::Active | PassOutcome::Interrupted => ACTIVE_INTERVAL,
                PassOutcome::Contended => CONTENTION_INTERVAL,
            },
            Err(_) => IDLE_INTERVAL,
        }
    }
}

struct Budget {
    deadline: Option<Instant>,
    /// Amount of pages to reclaim per transaction
    pages: u32,
}

struct ReclamationOptions {
    budget: Budget,
    batch_pages: u32,
}

impl From<Budget> for ReclamationOptions {
    fn from(budget: Budget) -> Self {
        Self {
            budget,
            batch_pages: BATCH_PAGES,
        }
    }
}

async fn reclaim(
    path: &Path,
    options: impl Into<ReclamationOptions>,
    shutdown: &watch::Receiver<()>,
) -> anyhow::Result<ReclamationPass> {
    if !tokio::fs::try_exists(path).await? {
        return Ok(ReclamationPass {
            pages: 0,
            outcome: PassOutcome::Idle,
        });
    }
    #[expect(
        clippy::disallowed_methods,
        reason = "maintenance needs a dedicated connection with no busy wait or automatic checkpoints"
    )]
    let mut connection = match SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(false)
            .busy_timeout(Duration::ZERO)
            .synchronous(SqliteSynchronous::Normal)
            // Keep commits from doing checkpoint I/O while holding up writers.
            .pragma("wal_autocheckpoint", "0")
            // A 16 MiB cache reduced CPU time in large-database measurements.
            .pragma("cache_size", "-16384")
            .log_statements(LevelFilter::Off)
            .log_slow_statements(LevelFilter::Off, Duration::ZERO),
    )
    .await
    {
        Ok(connection) => connection,
        Err(error) => match outcome_after_error(&error) {
            Some(outcome) => {
                return Ok(ReclamationPass { pages: 0, outcome });
            }
            None => return Err(error.into()),
        },
    };

    let result = reclaim_pages(&mut connection, options, shutdown).await;
    connection.close().await?;
    result
}

/// Reclaims free pages in short transactions while preserving a reusable reserve.
///
/// Each batch checks the reserve under `BEGIN IMMEDIATE` and commits before
/// checkpointing or sleeping. The pass bounds reclaimed freelist pages and checks
/// a cooperative deadline; storage I/O can overrun that deadline.
async fn reclaim_pages(
    connection: &mut SqliteConnection,
    options: impl Into<ReclamationOptions>,
    shutdown: &watch::Receiver<()>,
) -> anyhow::Result<ReclamationPass> {
    let ReclamationOptions {
        budget,
        batch_pages,
    } = options.into();
    if budget
        .deadline
        .is_some_and(|deadline| Instant::now() >= deadline)
        || shutdown.has_changed().unwrap_or(true)
    {
        return Ok(ReclamationPass {
            pages: 0,
            outcome: if shutdown.has_changed().unwrap_or(true) {
                PassOutcome::Shutdown
            } else {
                PassOutcome::Interrupted
            },
        });
    }
    let mut reclaimed = 0;
    let mut outcome = PassOutcome::Contended;
    let result: Result<(), sqlx::Error> = async {
        let stop = shutdown.clone();
        connection
            .lock_handle()
            .await?
            .set_progress_handler(/*num_ops*/ 1, move || {
                !stop.has_changed().unwrap_or(true)
                    && budget
                        .deadline
                        .is_none_or(|deadline| Instant::now() < deadline)
            });

        let (mode, page_size, pages, free): (i64, i64, i64, i64) = sqlx::query_as(
            "SELECT auto_vacuum, page_size, page_count, freelist_count \
         FROM pragma_auto_vacuum, pragma_page_size, pragma_page_count, pragma_freelist_count",
        )
        .fetch_one(&mut *connection)
        .await?;

        // Require incremental auto-vacuum and at least 64 MiB and 25% free space.
        if mode != 2 || free * page_size < MIN_FREE_BYTES || free < pages / 4 {
            outcome = PassOutcome::Idle;
            return Ok(());
        }
        if !checkpoint_complete(connection).await? {
            return Ok(());
        }

        outcome = PassOutcome::Active;
        let reserve = (RESERVE_BYTES / page_size).max(pages / 10);
        let version: i64 = sqlx::query_scalar("PRAGMA data_version")
            .fetch_one(&mut *connection)
            .await?;

        while reclaimed < budget.pages
            && budget
                .deadline
                .is_none_or(|deadline| Instant::now() < deadline)
            && !shutdown.has_changed().unwrap_or(true)
        {
            // Check inside the same write transaction, so a foreground writer cannot
            // consume the reserve between our free-page check and vacuum.
            let mut transaction = connection.begin_with("BEGIN IMMEDIATE").await?;
            let free: i64 = sqlx::query_scalar("PRAGMA freelist_count")
                .fetch_one(&mut *transaction)
                .await?;
            if free <= reserve {
                transaction.rollback().await?;
                outcome = PassOutcome::Idle;
                break;
            }

            let batch = i64::from(batch_pages.min(budget.pages - reclaimed)).min(free - reserve);
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "PRAGMA incremental_vacuum({batch})"
            )))
            .execute(&mut *transaction)
            .await?;
            transaction.commit().await?;
            reclaimed += batch as u32;
            // A reader arriving after admission can pin at most one batch.
            if !checkpoint_complete(connection).await? {
                outcome = PassOutcome::Contended;
                break;
            }

            tokio::time::sleep(Duration::from_millis(5)).await;
            // Back off if another connection committed during this pass.
            let current: i64 = sqlx::query_scalar("PRAGMA data_version")
                .fetch_one(&mut *connection)
                .await?;
            if current != version {
                outcome = PassOutcome::Contended;
                break;
            }
        }
        Ok(())
    }
    .await;
    // Contention and deadlines defer work, including when a prefix has committed.
    // The connection is dedicated and closed by the caller, so an interrupted
    // transaction cannot leak into foreground work.
    match result {
        Ok(()) => {}
        Err(error) => match outcome_after_error(&error) {
            Some(reason) => outcome = reason,
            None => return Err(error.into()),
        },
    }
    if shutdown.has_changed().unwrap_or(true) {
        outcome = PassOutcome::Shutdown;
    } else if outcome == PassOutcome::Active
        && budget
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
    {
        // The deadline can also expire between statements, without SQLITE_INTERRUPT.
        outcome = PassOutcome::Interrupted;
    }

    Ok(ReclamationPass {
        pages: reclaimed,
        outcome,
    })
}

fn outcome_after_error(error: &sqlx::Error) -> Option<PassOutcome> {
    error
        .as_database_error()
        .and_then(sqlx::error::DatabaseError::code)
        .and_then(
            |code| match code.parse::<i32>().ok().map(|code| code & 0xff) {
                Some(libsqlite3_sys::SQLITE_BUSY | libsqlite3_sys::SQLITE_LOCKED) => {
                    Some(PassOutcome::Contended)
                }
                Some(libsqlite3_sys::SQLITE_INTERRUPT) => Some(PassOutcome::Interrupted),
                _ => None,
            },
        )
}

async fn checkpoint_complete(connection: &mut SqliteConnection) -> Result<bool, sqlx::Error> {
    // PASSIVE does not take the writer lock or wait for readers. It can still
    // exceed the cooperative deadline on slow storage. Do not reject a large
    // WAL forever: a quiet database may have no other writer to checkpoint it.
    let (busy, frames, done): (i64, i64, i64) = sqlx::query_as("PRAGMA wal_checkpoint(PASSIVE)")
        .fetch_one(connection)
        .await?;
    Ok(busy == 0 && frames >= 0 && frames == done)
}

#[cfg(test)]
#[path = "reclamation_tests.rs"]
mod tests;
