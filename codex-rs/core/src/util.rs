use std::sync::Arc;
use std::time::Duration;

use rand::Rng;
use tokio::sync::Notify;
use tracing::debug;

/// Make a CancellationToken that is fulfilled when SIGINT occurs.
pub fn notify_on_sigint() -> Arc<Notify> {
    let notify = Arc::new(Notify::new());

    tokio::spawn({
        let notify = Arc::clone(&notify);
        async move {
            loop {
                tokio::signal::ctrl_c().await.ok();
                debug!("Keyboard interrupt");
                notify.notify_waiters();
            }
        }
    });

    notify
}

/// Default exponential back‑off schedule: 200ms → 400ms → 800ms → 1600ms.
pub(crate) fn backoff(attempt: u64) -> Duration {
    let base_delay_ms = 200u64 * (1u64 << (attempt - 1));
    let jitter = rand::rng().random_range(0.8..1.2);
    let delay_ms = (base_delay_ms as f64 * jitter) as u64;
    Duration::from_millis(delay_ms)
}

/// Return `true` if the current working directory is inside a Git repository.
///
/// The check walks up the directory hierarchy looking for a `.git` folder. This
/// approach does **not** require the `git` binary or the `git2` crate and is
/// therefore fairly lightweight.  It intentionally only looks for the
/// presence of a *directory* named `.git` – this is good enough for regular
/// work‑trees and bare repos that live inside a work‑tree (common for
/// developers running Codex locally).
///
/// Note that this does **not** detect *work‑trees* created with
/// `git worktree add` where the checkout lives outside the main repository
/// directory.  If you need Codex to work from such a checkout simply pass the
/// `--allow-no-git-exec` CLI flag that disables the repo requirement.
pub fn is_inside_git_repo() -> bool {
    // Best‑effort: any IO error is treated as "not a repo" – the caller can
    // decide what to do with the result.
    let mut dir = match std::env::current_dir() {
        Ok(d) => d,
        Err(_) => return false,
    };

    loop {
        if dir.join(".git").exists() {
            return true;
        }

        // Pop one component (go up one directory).  `pop` returns false when
        // we have reached the filesystem root.
        if !dir.pop() {
            break;
        }
    }

    false
}
