//! Runtime-independent cleanup for children whose termination is externally owned.
//!
//! Initialize the shared worker before launch so dropping a live child never
//! creates a thread or waits for its exit. Poll only transferred PIDs, retaining
//! ownership until reaped, so one live child cannot delay cleanup of another.

use std::io;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::ptr;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::sync::mpsc;
use std::time::Duration;

/// Exclusive ownership transferred by a child handle after its caller drops it.
pub(crate) enum ChildToReap {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    Native(libc::pid_t),
    Tokio(tokio::process::Child),
}

static REAPER: Mutex<Option<mpsc::Sender<ChildToReap>>> = Mutex::new(None);
const REAP_BATCH_SIZE: usize = 64;

/// Reserve the shared reaper, returning thread creation failures before launch.
pub(crate) fn sender() -> io::Result<mpsc::Sender<ChildToReap>> {
    let mut reaper = REAPER.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(sender) = &*reaper {
        return Ok(sender.clone());
    }
    let (sender, receiver) = mpsc::channel();
    std::thread::Builder::new()
        .name("codex-child-reaper".into())
        .spawn(move || {
            let mut pending = Vec::new();
            loop {
                if pending.is_empty() {
                    let Ok(pid) = receiver.recv() else { return };
                    pending.push(pid);
                } else {
                    match receiver.recv_timeout(Duration::from_millis(10)) {
                        Ok(pid) => pending.push(pid),
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(mpsc::RecvTimeoutError::Disconnected) => return,
                    }
                }
                // Amortize scans across bursts without starving reaping when drops continue.
                pending.extend(receiver.try_iter().take(REAP_BATCH_SIZE - 1));
                pending.retain_mut(|child| match child {
                    #[cfg(any(target_os = "macos", target_os = "linux"))]
                    ChildToReap::Native(pid) => {
                        // SAFETY: Drop transferred exclusive ownership of this PID.
                        let result = unsafe { libc::waitpid(*pid, ptr::null_mut(), libc::WNOHANG) };
                        result == 0
                            || (result == -1
                                && io::Error::last_os_error().kind() == io::ErrorKind::Interrupted)
                    }
                    ChildToReap::Tokio(child) => match child.try_wait() {
                        Ok(None) => true,
                        Ok(Some(_)) => false,
                        Err(error) => error.kind() == io::ErrorKind::Interrupted,
                    },
                });
            }
        })?;
    *reaper = Some(sender.clone());
    Ok(sender)
}
