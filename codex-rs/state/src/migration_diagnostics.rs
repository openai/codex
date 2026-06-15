//! Read-only inspection of SQLx migration metadata for runtime databases.

use std::path::Path;
use std::path::PathBuf;

use sqlx::Row;
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqlitePoolOptions;

use crate::migrations::runtime_goals_migrator;
use crate::migrations::runtime_logs_migrator;
use crate::migrations::runtime_memories_migrator;
use crate::migrations::runtime_state_migrator;
use crate::runtime::goals_db_path;
use crate::runtime::logs_db_path;
use crate::runtime::memories_db_path;
use crate::runtime::state_db_path;

/// Migration compatibility for one Codex runtime database.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeDbMigrationInspection {
    pub label: &'static str,
    pub path: PathBuf,
    pub status: RuntimeDbMigrationStatus,
}

/// Read-only compatibility status for a runtime database's migration ledger.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeDbMigrationStatus {
    Missing,
    Compatible { applied: usize, pending: usize },
    Incompatible(Vec<RuntimeDbMigrationIssue>),
    Unreadable(String),
}

/// An applied migration state that the current runtime cannot safely use.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeDbMigrationIssue {
    Dirty { version: i64 },
    ChecksumMismatch { version: i64 },
}

struct RuntimeDbMigrationSpec {
    label: &'static str,
    path: PathBuf,
    migrator: Migrator,
}

/// Inspect every runtime database without applying pending migrations.
pub async fn inspect_runtime_db_migrations(
    sqlite_home: &Path,
) -> Vec<RuntimeDbMigrationInspection> {
    let specs = [
        RuntimeDbMigrationSpec {
            label: "state DB",
            path: state_db_path(sqlite_home),
            migrator: runtime_state_migrator(),
        },
        RuntimeDbMigrationSpec {
            label: "log DB",
            path: logs_db_path(sqlite_home),
            migrator: runtime_logs_migrator(),
        },
        RuntimeDbMigrationSpec {
            label: "goals DB",
            path: goals_db_path(sqlite_home),
            migrator: runtime_goals_migrator(),
        },
        RuntimeDbMigrationSpec {
            label: "memories DB",
            path: memories_db_path(sqlite_home),
            migrator: runtime_memories_migrator(),
        },
    ];

    let mut inspections = Vec::with_capacity(specs.len());
    for spec in specs {
        inspections.push(inspect_runtime_db_migration(spec).await);
    }
    inspections
}

async fn inspect_runtime_db_migration(
    spec: RuntimeDbMigrationSpec,
) -> RuntimeDbMigrationInspection {
    if !spec.path.is_file() {
        return RuntimeDbMigrationInspection {
            label: spec.label,
            path: spec.path,
            status: RuntimeDbMigrationStatus::Missing,
        };
    }

    let status = match inspect_existing_database(&spec.path, &spec.migrator).await {
        Ok(status) => status,
        Err(err) => RuntimeDbMigrationStatus::Unreadable(err.to_string()),
    };
    RuntimeDbMigrationInspection {
        label: spec.label,
        path: spec.path,
        status,
    }
}

async fn inspect_existing_database(
    path: &Path,
    migrator: &Migrator,
) -> anyhow::Result<RuntimeDbMigrationStatus> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(false)
        .read_only(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await?;
    let table_exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
    )
    .fetch_one(&pool)
    .await?
        > 0;
    if !table_exists {
        pool.close().await;
        return Ok(RuntimeDbMigrationStatus::Compatible {
            applied: 0,
            pending: migrator.iter().count(),
        });
    }

    let rows = sqlx::query("SELECT version, success, checksum FROM _sqlx_migrations")
        .fetch_all(&pool)
        .await?;
    pool.close().await;

    let mut applied_versions = Vec::with_capacity(rows.len());
    let mut issues = Vec::new();
    for row in &rows {
        let version = row.try_get::<i64, _>("version")?;
        applied_versions.push(version);
        if !row.try_get::<bool, _>("success")? {
            issues.push(RuntimeDbMigrationIssue::Dirty { version });
            continue;
        }
        let Some(migration) = migrator
            .iter()
            .find(|migration| migration.version == version)
        else {
            // Runtime migrators intentionally tolerate migrations from a newer binary.
            continue;
        };
        let checksum = row.try_get::<Vec<u8>, _>("checksum")?;
        if checksum.as_slice() != migration.checksum.as_ref() {
            issues.push(RuntimeDbMigrationIssue::ChecksumMismatch { version });
        }
    }

    if !issues.is_empty() {
        return Ok(RuntimeDbMigrationStatus::Incompatible(issues));
    }

    let pending = migrator
        .iter()
        .filter(|migration| !applied_versions.contains(&migration.version))
        .count();
    Ok(RuntimeDbMigrationStatus::Compatible {
        applied: rows.len(),
        pending,
    })
}

#[cfg(test)]
#[path = "migration_diagnostics_tests.rs"]
mod tests;
