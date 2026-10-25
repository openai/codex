//! Initializes writable connections while preserving existing database settings.
//! Only empty databases adopt the requested journal mode; DELETE connections use EXTRA sync.
//! Managed DELETE requirements also reject other modes on every connection.

#![expect(
    clippy::disallowed_methods,
    reason = "centralized SQLite connection initialization"
)]

use sqlx::Error;
use sqlx::SqlitePool;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqlitePoolOptions;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum JournalModePolicy {
    InitializeWal,
    InitializeDelete,
    RequireDelete,
}

pub(super) async fn connect_once(
    options: &SqliteConnectOptions,
    journal_mode_policy: JournalModePolicy,
) -> Result<SqlitePool, Error> {
    // SQLx retries after_connect errors, eventually replacing them with PoolTimedOut.
    // Return the first initialization error directly while opening this pool.
    let (init_error_tx, mut init_error_rx) = tokio::sync::mpsc::channel(/*buffer*/ 1);
    let pool_options =
        SqlitePoolOptions::new()
            .max_connections(5)
            .after_connect(move |connection, _metadata| {
                let init_error_tx = init_error_tx.clone();
                Box::pin(async move {
                    let result = async {
                        let journal: String = sqlx::query_scalar("PRAGMA journal_mode")
                            .fetch_one(&mut *connection)
                            .await?;
                        if journal_mode_policy == JournalModePolicy::RequireDelete
                            && journal != "delete"
                        {
                            return Err(Error::Protocol(format!(
                                "NFS database requires DELETE journaling, found {journal}"
                            )));
                        }
                        if journal == "delete" {
                            // If the filesystem honors sync requests, EXTRA syncs the directory
                            // after SQLite deletes a rollback journal.
                            sqlx::query("PRAGMA synchronous = EXTRA")
                                .execute(connection)
                                .await?;
                        }
                        Ok(())
                    }
                    .await;
                    result.map_err(|error| match init_error_tx.try_send(error) {
                        // The opener owns the original error and cancels connection retries.
                        Ok(()) => Error::PoolClosed,
                        // After opening, lazy connections retain SQLx's normal error handling.
                        Err(error) => error.into_inner(),
                    })
                })
            });
    let pool = tokio::select! {
        biased;
        Some(error) = init_error_rx.recv() => Err(error),
        result = pool_options.connect_with(options.clone()) => result,
    }?;
    // Database settings persist on disk.
    // Initialize these settings once before callers can use the pool.
    // Connections that open later need only settings that apply to each connection.
    let initialized: Result<(), Error> = async {
        let mut connection = pool.acquire().await?;
        // Auto-vacuum alone can write a header to disk.
        // If creation stops before the schema exists, startup must initialize the file.
        let empty =
            sqlx::query_scalar::<_, bool>("SELECT NOT EXISTS (SELECT 1 FROM sqlite_schema)")
                .fetch_one(&mut *connection)
                .await?;
        if empty {
            let mode: i64 = sqlx::query_scalar("PRAGMA auto_vacuum")
                .fetch_one(&mut *connection)
                .await?;
            if mode == 0 {
                sqlx::query("PRAGMA auto_vacuum = INCREMENTAL")
                    .execute(&mut *connection)
                    .await?;
            }
            let pragma = match journal_mode_policy {
                JournalModePolicy::InitializeWal => "PRAGMA journal_mode = WAL",
                JournalModePolicy::InitializeDelete | JournalModePolicy::RequireDelete => {
                    "PRAGMA journal_mode = DELETE"
                }
            };
            sqlx::query(pragma).execute(&mut *connection).await?;
        }
        // Initialization or another process can change the journal mode after after_connect.
        // Match this connection's synchronization setting to the resulting journal mode.
        let journal: String = sqlx::query_scalar("PRAGMA journal_mode")
            .fetch_one(&mut *connection)
            .await?;
        if journal == "wal" {
            sqlx::query("PRAGMA synchronous = NORMAL")
                .execute(&mut *connection)
                .await?;
        } else if journal == "delete" {
            sqlx::query("PRAGMA synchronous = EXTRA")
                .execute(&mut *connection)
                .await?;
        }
        // Wait for the connection to return to the pool before startup requests it again.
        // An asynchronous drop can otherwise cause the pool to open a second connection.
        connection.return_to_pool().await;
        Ok(())
    }
    .await;
    if let Err(error) = initialized {
        pool.close().await;
        return Err(error);
    }
    Ok(pool)
}
