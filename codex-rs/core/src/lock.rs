//! Repository-level lock mechanism for serializing write operations.
//!
//! Provides atomic file-based locking with stale detection and metadata tracking.

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

const LOCK_FILENAME: &str = "lock.json";
const LOCK_VERSION: &str = "1.0";

/// Lock holder metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockMetadata {
    /// Lock format version
    pub version: String,
    /// Process ID of lock holder
    pub pid: u32,
    /// Parent process ID
    pub ppid: Option<u32>,
    /// User ID (Unix)
    #[cfg(unix)]
    pub uid: Option<u32>,
    /// Hostname
    pub hostname: Option<String>,
    /// Repository path
    pub repo_path: String,
    /// Lock acquisition timestamp (Unix epoch seconds)
    pub started_at: u64,
    /// Optional expiration timestamp (Unix epoch seconds)
    pub expires_at: Option<u64>,
}

/// Repository lock manager
pub struct RepositoryLock {
    lock_path: PathBuf,
    codex_dir: PathBuf,
}

impl RepositoryLock {
    /// Create a new lock manager for the given repository
    pub fn new<P: AsRef<Path>>(repo_root: P) -> Result<Self> {
        let codex_dir = repo_root.as_ref().join(".codex");
        fs::create_dir_all(&codex_dir)
            .context("Failed to create .codex directory")?;
        
        let lock_path = codex_dir.join(LOCK_FILENAME);
        
        Ok(Self {
            lock_path,
            codex_dir,
        })
    }

    /// Attempt to acquire the lock
    pub fn acquire(&self, ttl_secs: Option<u64>) -> Result<LockMetadata> {
        // Check for existing lock and clean up stale locks
        if self.lock_path.exists() {
            if let Ok(existing) = self.read_lock() {
                if self.is_lock_alive(&existing) {
                    return Err(anyhow!(
                        "Lock is held by PID {} on {} since {}",
                        existing.pid,
                        existing.hostname.as_deref().unwrap_or("unknown"),
                        existing.started_at
                    ));
                }
                // Stale lock, will be overwritten
                tracing::info!("Removing stale lock from PID {}", existing.pid);
            }
        }

        let metadata = self.create_lock_metadata(ttl_secs)?;
        self.write_lock(&metadata)?;
        
        Ok(metadata)
    }

    /// Release the lock
    pub fn release(&self) -> Result<()> {
        if !self.lock_path.exists() {
            return Ok(());
        }

        // Verify we own the lock before releasing
        let existing = self.read_lock()?;
        let current_pid = process::id();
        
        if existing.pid != current_pid {
            return Err(anyhow!(
                "Cannot release lock owned by PID {} (current PID: {})",
                existing.pid,
                current_pid
            ));
        }

        fs::remove_file(&self.lock_path)
            .context("Failed to remove lock file")?;
        
        Ok(())
    }

    /// Force remove the lock (use with caution)
    pub fn force_remove(&self) -> Result<()> {
        if !self.lock_path.exists() {
            return Ok(());
        }

        fs::remove_file(&self.lock_path)
            .context("Failed to force remove lock file")?;
        
        tracing::warn!("Lock forcibly removed");
        Ok(())
    }

    /// Get current lock status
    pub fn status(&self) -> Result<Option<LockMetadata>> {
        if !self.lock_path.exists() {
            return Ok(None);
        }

        let metadata = self.read_lock()?;
        Ok(Some(metadata))
    }

    /// Check if lock is currently held and alive
    pub fn is_locked(&self) -> bool {
        if let Ok(Some(metadata)) = self.status() {
            self.is_lock_alive(&metadata)
        } else {
            false
        }
    }

    // Private helper methods

    fn create_lock_metadata(&self, ttl_secs: Option<u64>) -> Result<LockMetadata> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_secs();

        let hostname = hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok());

        #[cfg(unix)]
        // SAFETY: libc::getuid() has no preconditions and cannot fail, so it is safe to call.
        let uid = Some(unsafe { libc::getuid() });

        let repo_path = self.codex_dir
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_string_lossy()
            .to_string();

        Ok(LockMetadata {
            version: LOCK_VERSION.to_string(),
            pid: process::id(),
            ppid: get_parent_pid(),
            #[cfg(unix)]
            uid,
            hostname,
            repo_path,
            started_at: now,
            expires_at: ttl_secs.map(|ttl| now + ttl),
        })
    }

    fn write_lock(&self, metadata: &LockMetadata) -> Result<()> {
        let json = serde_json::to_string_pretty(metadata)?;
        
        #[cfg(unix)]
        {
            // Use O_EXCL for atomic creation on Unix
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&self.lock_path)
                .or_else(|_| {
                    // If file exists, try to overwrite (stale lock)
                    OpenOptions::new()
                        .write(true)
                        .truncate(true)
                        .create(true)
                        .mode(0o600)
                        .open(&self.lock_path)
                })?;
            
            file.write_all(json.as_bytes())?;
            file.sync_all()?;
        }

        #[cfg(not(unix))]
        {
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&self.lock_path)?;
            
            file.write_all(json.as_bytes())?;
            file.sync_all()?;
        }

        Ok(())
    }

    fn read_lock(&self) -> Result<LockMetadata> {
        let mut file = File::open(&self.lock_path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        
        let metadata: LockMetadata = serde_json::from_str(&contents)
            .context("Failed to parse lock file")?;
        
        Ok(metadata)
    }

    pub fn is_lock_alive(&self, metadata: &LockMetadata) -> bool {
        // Check if process is alive
        if !is_process_alive(metadata.pid) {
            return false;
        }

        // Check TTL expiration
        if let Some(expires_at) = metadata.expires_at {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            
            if now > expires_at {
                return false;
            }
        }

        true
    }
}

/// Check if a process is alive
fn is_process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // Signal 0 checks if process exists without sending actual signal
        // Returns 0 if process exists, -1 otherwise
        // SAFETY: Calling libc::kill with signal 0 is a standard, no-op way to check if a process exists.
        // It does not send a signal or affect the target process, and cannot cause undefined behavior.
        let result = unsafe { libc::kill(pid as i32, 0) };
        if result == 0 {
            true
        } else {
            // Use std::io::Error to safely get errno
            let err = std::io::Error::last_os_error();
            // ESRCH means process doesn't exist
            err.raw_os_error() != Some(libc::ESRCH)
        }
    }

    #[cfg(windows)]
    {
        use std::process::Command;
        // On Windows, use tasklist to check
        Command::new("tasklist")
            .args(&["/FI", &format!("PID eq {}", pid)])
            .output()
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|output| output.contains(&pid.to_string()))
            .unwrap_or(false)
    }

    #[cfg(not(any(unix, windows)))]
    {
        // Assume alive on unknown platforms
        true
    }
}

/// Get parent process ID
fn get_parent_pid() -> Option<u32> {
    #[cfg(unix)]
    {
        // SAFETY: libc::getppid() has no preconditions and cannot fail, so it is safe to call.
        Some(unsafe { libc::getppid() as u32 })
    }

    #[cfg(not(unix))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_lock_acquire_and_release() {
        let temp_dir = TempDir::new().unwrap();
        let lock = RepositoryLock::new(temp_dir.path()).unwrap();

        // Should be able to acquire
        let metadata = lock.acquire(None).unwrap();
        assert_eq!(metadata.pid, process::id());

        // Should be locked
        assert!(lock.is_locked());

        // Should be able to release
        lock.release().unwrap();

        // Should not be locked anymore
        assert!(!lock.is_locked());
    }

    #[test]
    fn test_lock_prevents_double_acquisition() {
        let temp_dir = TempDir::new().unwrap();
        let lock = RepositoryLock::new(temp_dir.path()).unwrap();

        // First acquisition succeeds
        lock.acquire(None).unwrap();

        // Second acquisition should fail
        let result = lock.acquire(None);
        assert!(result.is_err());

        lock.release().unwrap();
    }

    #[test]
    fn test_force_remove() {
        let temp_dir = TempDir::new().unwrap();
        let lock = RepositoryLock::new(temp_dir.path()).unwrap();

        lock.acquire(None).unwrap();
        assert!(lock.is_locked());

        lock.force_remove().unwrap();
        assert!(!lock.is_locked());
    }
}
