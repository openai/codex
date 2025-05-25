use std::sync::Arc;
use std::time::Duration;

use rand::Rng;
use tokio::sync::Notify;
use tracing::debug;

use crate::config::Config;

const INITIAL_DELAY_MS: u64 = 200;
const BACKOFF_FACTOR: f64 = 1.3;

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

pub(crate) fn backoff(attempt: u64) -> Duration {
    let exp = BACKOFF_FACTOR.powi(attempt.saturating_sub(1) as i32);
    let base = (INITIAL_DELAY_MS as f64 * exp) as u64;
    let jitter = rand::rng().random_range(0.9..1.1);
    Duration::from_millis((base as f64 * jitter) as u64)
}

/// Return `true` if the project folder specified by the `Config` is inside a
/// Git repository.
///
/// The check walks up the directory hierarchy looking for a `.git` file or
/// directory (note `.git` can be a file that contains a `gitdir` entry). This
/// approach does **not** require the `git` binary or the `git2` crate and is
/// therefore fairly lightweight.
///
/// Note that this does **not** detect *work‑trees* created with
/// `git worktree add` where the checkout lives outside the main repository
/// directory. If you need Codex to work from such a checkout simply pass the
/// `--allow-no-git-exec` CLI flag that disables the repo requirement.
pub fn is_inside_git_repo(config: &Config) -> bool {
    let mut dir = config.cwd.to_path_buf();

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
