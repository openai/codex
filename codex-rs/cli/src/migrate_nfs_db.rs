//! Confirms explicit database migration and reports the shared migrator's results.
//! Other SQLite clients must remain stopped during replacement, and NFS must enforce
//! cross-client POSIX locks.

use anyhow::Context;
use anyhow::ensure;
use clap::Parser;
use codex_config::LoaderOverrides;
use codex_core::config::ConfigBuilder;
use codex_state::NfsDbMigration;
use codex_state::NfsDbMigrationOutcome;
use codex_utils_cli::CliConfigOverrides;
use owo_colors::OwoColorize;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    after_help = "Stop SQLite clients on every host and keep them stopped through cutover.\nThe NFS mount must support cross-client POSIX locks (no nolock/local_lock=all).\nThis command replaces configured databases in place and retains their WAL backups."
)]
pub(crate) struct MigrateNfsDbCommand {
    /// Use a local directory with space for the database and WAL files.
    /// The default is the system temporary directory.
    #[arg(long, default_value_os_t = std::env::temp_dir())]
    staging_dir: PathBuf,
}

pub(crate) async fn run(
    command: MigrateNfsDbCommand,
    config_overrides: CliConfigOverrides,
    loader_overrides: LoaderOverrides,
) -> anyhow::Result<()> {
    ensure!(
        cfg!(unix),
        "migrate-nfs-db currently requires Unix filesystem locking and directory fsync"
    );
    let warning = "WARNING: This operation is unsafe under concurrency.";
    if supports_color::on(supports_color::Stream::Stderr).is_some() {
        eprintln!("{}", warning.red().bold());
    } else {
        eprintln!("{warning}");
    }
    eprintln!(
        "Stop all Codex instances on every host that uses these databases.\nKeep them stopped until the migration completes successfully."
    );
    let config = ConfigBuilder::default()
        .cli_overrides(
            config_overrides
                .parse_overrides()
                .map_err(anyhow::Error::msg)?,
        )
        .loader_overrides(loader_overrides)
        .build()
        .await?;
    // Check the merged config for an explicit log directory.
    let effective_config = config.config_layer_stack.effective_config();
    let skip_logs = effective_config.get("log_dir").is_some();
    let sqlite = config.sqlite_config();
    if !sqlite.home().try_exists()? {
        println!("No databases found.");
        return Ok(());
    }
    let mut needs_permission_confirmation = false;
    for database in sqlite.runtime_db_paths() {
        if skip_logs && database.path == sqlite.logs_db_path() {
            continue;
        }
        needs_permission_confirmation |=
            NfsDbMigration::needs_permission_confirmation(&database.path)?;
    }
    if needs_permission_confirmation {
        eprintln!(
            "Some database files have permissions other than 0600.\nMigration preserves file mode bits, including group and other access."
        );
    }
    eprintln!(
        "Replacement files belong to you; the original owner, group, and extended ACLs are not copied.\nThis can remove another user's access even when file permissions are 0600.\nOriginal files remain in a private backup directory.\nYou need write access to the database directory and its migration lock."
    );
    let prompt = if needs_permission_confirmation {
        "Migrate databases, including files with permissions other than 0600? [y/N]:"
    } else {
        "Continue? [y/N]:"
    };
    ensure!(
        crate::confirm(prompt)?,
        "Migration cancelled. No databases were changed."
    );
    let mut migration = NfsDbMigration::lock(sqlite.home())?;
    let mut databases_found = 0;
    for database in sqlite.runtime_db_paths() {
        if skip_logs && database.path == sqlite.logs_db_path() {
            println!("Skipped logs database: log_dir is configured.");
            continue;
        }
        let outcome = migration.migrate(&database.path, &command.staging_dir).await.with_context(|| format!(
            "migration failed for {}. Keep all Codex instances stopped. Correct the error and rerun codex migrate-nfs-db",
            database.path.display()
        ))?;
        match outcome {
            NfsDbMigrationOutcome::Missing => continue,
            NfsDbMigrationOutcome::AlreadyDelete => {
                println!("Already in DELETE mode: {}", database.path.display())
            }
            NfsDbMigrationOutcome::Migrated { backup_directory } => println!(
                "Migrated database: {}\nWAL backup: {}",
                database.path.display(),
                backup_directory.display()
            ),
        }
        databases_found += 1;
    }
    if databases_found == 0 {
        println!("No databases found.");
        return Ok(());
    }
    println!("Migration completed successfully. You can restart Codex.");
    Ok(())
}
