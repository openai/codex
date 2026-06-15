//! Interactive repair for incompatible runtime database migration metadata.

use std::io::IsTerminal;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::bail;
use codex_app_server_daemon::LifecycleCommand;
use codex_core::config::Config;
use codex_state::RuntimeDbMigrationInspection;
use codex_state::RuntimeDbMigrationStatus;
use sysinfo::Pid;
use sysinfo::ProcessesToUpdate;
use sysinfo::Signal;
use sysinfo::System;

use super::state_migrations::incompatible_database_paths;

const PROCESS_STOP_TIMEOUT: Duration = Duration::from_secs(5);
const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(100);
const PROCESS_STABLE_WINDOW: Duration = Duration::from_secs(1);

#[derive(Clone, Debug, Eq, PartialEq)]
struct AppServerProcess {
    pid: Pid,
    executable: PathBuf,
}

pub(super) async fn run_migration_fix(
    config: &Config,
    inspections: &[RuntimeDbMigrationInspection],
) -> anyhow::Result<bool> {
    let database_paths = incompatible_database_paths(inspections);
    if database_paths.is_empty() {
        return Ok(false);
    }
    if !(std::io::stdin().is_terminal() && std::io::stderr().is_terminal()) {
        bail!(
            "cannot repair state databases without an interactive terminal; rerun `codex doctor --fix` in a terminal"
        );
    }

    let processes = app_server_processes();
    if !confirm_repair(&database_paths, &processes)? {
        eprintln!("No changes made.");
        return Ok(false);
    }

    stop_managed_daemon().await;
    stop_app_server_processes().await?;

    let mut backup_folders = Vec::new();
    for path in &database_paths {
        let backups = codex_state::backup_runtime_db_for_fresh_start(path)
            .await
            .with_context(|| format!("failed to back up {}", path.display()))?;
        if let Some(folder) = backups
            .first()
            .and_then(|backup| backup.backup_path.parent())
            .map(Path::to_path_buf)
            && !backup_folders.contains(&folder)
        {
            backup_folders.push(folder);
        }
    }

    let runtime = codex_state::StateRuntime::init(
        config.sqlite_home.clone(),
        config.model_provider_id.clone(),
    )
    .await
    .context("failed to rebuild Codex runtime databases")?;
    runtime.close().await;

    let post_repair = codex_state::inspect_runtime_db_migrations(&config.sqlite_home).await;
    if let Some(inspection) = post_repair.iter().find(|inspection| {
        matches!(
            inspection.status,
            RuntimeDbMigrationStatus::Incompatible(_) | RuntimeDbMigrationStatus::Unreadable(_)
        )
    }) {
        bail!(
            "rebuilt {} still has incompatible migration metadata at {}",
            inspection.label,
            inspection.path.display()
        );
    }

    eprintln!("Codex rebuilt the affected runtime database.");
    for backup_folder in backup_folders {
        eprintln!("Backup folder: {}", backup_folder.display());
    }
    eprintln!("App servers remain stopped.");
    eprintln!("Reopen the Codex Desktop App or run `codex app-server daemon start` when ready.");
    Ok(true)
}

fn confirm_repair(
    database_paths: &[PathBuf],
    processes: &[AppServerProcess],
) -> std::io::Result<bool> {
    let mut stderr = std::io::stderr().lock();
    writeln!(
        stderr,
        "Codex found incompatible database migration history:"
    )?;
    for path in database_paths {
        writeln!(stderr, "  - {}", path.display())?;
    }
    writeln!(
        stderr,
        "The affected database and SQLite sidecars will be moved to a backup folder before a fresh database is created."
    )?;
    if processes.is_empty() {
        writeln!(
            stderr,
            "No running Codex app-server processes were detected."
        )?;
    } else {
        writeln!(
            stderr,
            "The following Codex app-server processes will be stopped, interrupting active work:"
        )?;
        for process in processes {
            writeln!(
                stderr,
                "  - PID {}: {}",
                process.pid,
                process.executable.display()
            )?;
        }
    }
    write!(stderr, "Continue? [y/N]: ")?;
    stderr.flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let answer = input.trim();
    Ok(answer.eq_ignore_ascii_case("y") || answer.eq_ignore_ascii_case("yes"))
}

async fn stop_managed_daemon() {
    if let Err(err) = codex_app_server_daemon::run(LifecycleCommand::Stop).await {
        eprintln!("Could not stop the managed app-server daemon directly: {err}");
    }
}

async fn stop_app_server_processes() -> anyhow::Result<()> {
    signal_app_server_processes(/*force*/ false)?;
    if wait_for_app_servers_to_stop(PROCESS_STOP_TIMEOUT).await {
        return Ok(());
    }

    signal_app_server_processes(/*force*/ true)?;
    if wait_for_app_servers_to_stop(PROCESS_STOP_TIMEOUT).await {
        return Ok(());
    }

    let remaining = app_server_processes();
    let remaining_pids = remaining
        .iter()
        .map(|process| process.pid.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    bail!(
        "Codex app-server processes are still running or were restarted (PIDs: {remaining_pids}). Quit the Codex Desktop App and any editor integrations, then rerun `codex doctor --fix`. No database files were changed."
    )
}

fn signal_app_server_processes(force: bool) -> anyhow::Result<()> {
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All, /*remove_dead_processes*/ true);
    let current_pid = Pid::from_u32(std::process::id());
    let mut failed = Vec::new();
    for (pid, process) in system.processes() {
        if *pid == current_pid
            || !is_app_server_process(process.name(), process.exe(), process.cmd())
        {
            continue;
        }
        let signaled = if force {
            process.kill()
        } else {
            process
                .kill_with(Signal::Term)
                .unwrap_or_else(|| process.kill())
        };
        if !signaled {
            failed.push(pid.to_string());
        }
    }
    if failed.is_empty() {
        Ok(())
    } else {
        bail!(
            "failed to stop Codex app-server processes with PIDs: {}",
            failed.join(", ")
        )
    }
}

async fn wait_for_app_servers_to_stop(timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    let mut empty_since = None;
    loop {
        if app_server_processes().is_empty() {
            let empty_since = empty_since.get_or_insert_with(Instant::now);
            if empty_since.elapsed() >= PROCESS_STABLE_WINDOW {
                return true;
            }
        } else {
            empty_since = None;
        }
        if Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(PROCESS_POLL_INTERVAL).await;
    }
}

fn app_server_processes() -> Vec<AppServerProcess> {
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All, /*remove_dead_processes*/ true);
    let current_pid = Pid::from_u32(std::process::id());
    system
        .processes()
        .iter()
        .filter_map(|(pid, process)| {
            if *pid == current_pid
                || !is_app_server_process(process.name(), process.exe(), process.cmd())
            {
                return None;
            }
            Some(AppServerProcess {
                pid: *pid,
                executable: process
                    .exe()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| PathBuf::from(process.name())),
            })
        })
        .collect()
}

fn is_app_server_process(
    process_name: &std::ffi::OsStr,
    executable: Option<&Path>,
    command: &[std::ffi::OsString],
) -> bool {
    let executable_name = executable
        .and_then(Path::file_name)
        .unwrap_or(process_name)
        .to_string_lossy()
        .to_ascii_lowercase();
    if executable_name.contains("codex-app-server") {
        return true;
    }
    if !executable_name.starts_with("codex") {
        return false;
    }

    let args = command
        .iter()
        .map(|arg| arg.to_string_lossy().to_ascii_lowercase())
        .collect::<Vec<_>>();
    let Some(app_server_index) = args.iter().position(|arg| arg == "app-server") else {
        return false;
    };
    args.get(app_server_index + 1)
        .is_none_or(|arg| arg != "daemon")
}

#[cfg(test)]
#[path = "fix_tests.rs"]
mod tests;
