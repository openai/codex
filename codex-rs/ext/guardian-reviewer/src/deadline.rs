//! Bounds session startup and context work when a review deadline is configured.
//! Unlimited reviews still honor cancellation at every stage.

use crate::GuardianReviewSessionOutcome;
use std::future::Future;
use std::time::Duration;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

/// One shared review deadline, optionally disabled without disabling cancellation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewDeadline {
    At(Instant),
    Unlimited,
}

impl ReviewDeadline {
    pub fn from_timeout(timeout: Option<Duration>) -> Self {
        match timeout {
            Some(timeout) => Self::At(Instant::now() + timeout),
            None => Self::Unlimited,
        }
    }

    pub(crate) fn is_elapsed(self) -> bool {
        match self {
            Self::At(deadline) => Instant::now() >= deadline,
            Self::Unlimited => false,
        }
    }

    /// Retain local bounds, such as retry backoff and attribution lookup timeouts.
    pub fn cap(self, local_deadline: Instant) -> Instant {
        match self {
            Self::At(deadline) => deadline.min(local_deadline),
            Self::Unlimited => local_deadline,
        }
    }

    pub async fn wait(self) {
        match self {
            Self::At(deadline) => tokio::time::sleep_until(deadline).await,
            Self::Unlimited => std::future::pending::<()>().await,
        }
    }
}

pub async fn run_before_review_deadline<T>(
    deadline: ReviewDeadline,
    external_cancel: Option<&CancellationToken>,
    future: impl Future<Output = T>,
) -> Result<T, GuardianReviewSessionOutcome> {
    tokio::select! {
        _ = deadline.wait() => Err(GuardianReviewSessionOutcome::TimedOut),
        result = future => Ok(result),
        _ = async {
            if let Some(cancel_token) = external_cancel {
                cancel_token.cancelled().await;
            } else {
                std::future::pending::<()>().await;
            }
        } => Err(GuardianReviewSessionOutcome::Aborted),
    }
}

#[cfg(test)]
#[path = "deadline_tests.rs"]
mod tests;
