/// Lock management CLI commands
///
/// Provides `codex lock status` and `codex lock remove` for managing
/// repository-level locks.

use anyhow::Result;
use clap::Parser;
use codex_common::CliConfigOverrides;
use codex_core::lock::RepositoryLock;
use std::path::{Path, PathBuf};

#[derive(Debug, Parser)]
pub struct LockCli {
    #[clap(skip)]
    #[allow(dead_code)]
    pub config_overrides: CliConfigOverrides,

    #[clap(subcommand)]
    pub command: LockCommand,
}

#[derive(Debug, Parser)]
pub enum LockCommand {
    /// Show current lock status
    Status(StatusCommand),
    /// Remove the lock
    Remove(RemoveCommand),
}

#[derive(Debug, Parser)]
pub struct StatusCommand {}

#[derive(Debug, Parser)]
pub struct RemoveCommand {
    /// Force remove even if lock appears active
    #[arg(long)]
    pub force: bool,
}

pub fn run_lock_status(_status_cmd: StatusCommand) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let repo_root = find_repo_root(&cwd)?;
    let lock = RepositoryLock::new(&repo_root)?;
    handle_status(lock)
}

pub fn run_lock_remove(remove_cmd: RemoveCommand) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let repo_root = find_repo_root(&cwd)?;
    let lock = RepositoryLock::new(&repo_root)?;
    handle_remove(lock, remove_cmd.force)
}

// Legacy async wrapper (for compatibility)
#[allow(dead_code)]
pub async fn handle_lock_command(cmd: LockCommand, cwd: PathBuf) -> Result<()> {
    let repo_root = find_repo_root(&cwd)?;
    let lock = RepositoryLock::new(&repo_root)?;

    match cmd {
        LockCommand::Status(_) => {
            handle_status(lock)?;
        }
        LockCommand::Remove(remove_cmd) => {
            handle_remove(lock, remove_cmd.force)?;
        }
    }

    Ok(())
}

fn handle_status(lock: RepositoryLock) -> Result<()> {
    match lock.status()? {
        Some(metadata) => {
            println!("🔒 Lock is held");
            println!("  PID: {}", metadata.pid);
            if let Some(ppid) = metadata.ppid {
                println!("  Parent PID: {}", ppid);
            }
            #[cfg(unix)]
            if let Some(uid) = metadata.uid {
                println!("  User ID: {}", uid);
            }
            if let Some(hostname) = metadata.hostname {
                println!("  Hostname: {}", hostname);
            }
            println!("  Repository: {}", metadata.repo_path);
            println!("  Started at: {} (Unix timestamp)", metadata.started_at);
            if let Some(expires_at) = metadata.expires_at {
                println!("  Expires at: {} (Unix timestamp)", expires_at);
                
                use std::time::{SystemTime, UNIX_EPOCH};
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                
                if now < expires_at {
                    let remaining = expires_at - now;
                    println!("  Time remaining: {} seconds", remaining);
                } else {
                    println!("  ⚠️  Lock has expired (stale)");
                }
            }
            println!("  Version: {}", metadata.version);
        }
        None => {
            println!("✅ No lock is currently held");
        }
    }

    Ok(())
}

fn handle_remove(lock: RepositoryLock, force: bool) -> Result<()> {
    // Check if lock exists and is alive
    if let Some(metadata) = lock.status()? {
        if !force && lock.is_lock_alive(&metadata) {
            return Err(anyhow::anyhow!(
                "Lock is still active (PID {} on {}). Use --force to remove anyway.",
                metadata.pid,
                metadata.hostname.as_deref().unwrap_or("unknown")
            ));
        }

        lock.release()?;
        
        if force {
            println!("🗑️  Force removed lock (was held by PID {})", metadata.pid);
        } else {
            println!("🗑️  Removed stale lock (PID {})", metadata.pid);
        }
    } else {
        println!("ℹ️  No lock to remove");
    }

    Ok(())
}

/// Find repository root by searching for .git directory
fn find_repo_root(start: &Path) -> Result<PathBuf> {
    let mut current = start;
    
    loop {
        let git_dir = current.join(".git");
        if git_dir.exists() {
            return Ok(current.to_path_buf());
        }

        match current.parent() {
            Some(parent) => current = parent,
            None => return Err(anyhow::anyhow!(
                "Not in a git repository (no .git directory found)"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_find_repo_root() {
        let temp = TempDir::new().unwrap();
        let repo_root = temp.path();
        let git_dir = repo_root.join(".git");
        fs::create_dir(&git_dir).unwrap();

        let sub_dir = repo_root.join("src").join("deep");
        fs::create_dir_all(&sub_dir).unwrap();

        let found_root = find_repo_root(&sub_dir).unwrap();
        assert_eq!(found_root, repo_root);
    }

    #[test]
    fn test_find_repo_root_not_in_repo() {
        let temp = TempDir::new().unwrap();
        let result = find_repo_root(temp.path());
        assert!(result.is_err());
    }
}
