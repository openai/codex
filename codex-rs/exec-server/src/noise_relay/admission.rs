//! Listener-wide admission for inbound Noise handshakes. The budget is charged
//! before parsing a hybrid handshake and held through its ERS validation.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use tokio::sync::Notify;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::Semaphore;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

const MAX_CONCURRENT_HANDSHAKES: usize = 16;
const MAX_HANDSHAKES_PER_SECOND: usize = 32;
const HANDSHAKE_WINDOW: Duration = Duration::from_secs(1);

pub(crate) struct HandshakeBudget {
    permits: Arc<Semaphore>,
    attempts: Mutex<VecDeque<Instant>>,
}

impl Default for HandshakeBudget {
    fn default() -> Self {
        Self {
            permits: Arc::new(Semaphore::new(MAX_CONCURRENT_HANDSHAKES)),
            attempts: Mutex::new(VecDeque::new()),
        }
    }
}

impl HandshakeBudget {
    pub(crate) fn try_admit(&self) -> Option<OwnedSemaphorePermit> {
        let permit = Arc::clone(&self.permits).try_acquire_owned().ok()?;
        let now = Instant::now();
        let mut attempts = self
            .attempts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        while attempts
            .front()
            .is_some_and(|time| now.duration_since(*time) >= HANDSHAKE_WINDOW)
        {
            attempts.pop_front();
        }
        if attempts.len() >= MAX_HANDSHAKES_PER_SECOND {
            return None;
        }
        attempts.push_back(now);
        Some(permit)
    }
}

#[derive(Clone)]
pub(crate) struct InboundAdmission {
    pub(crate) budget: Arc<HandshakeBudget>,
    pub(crate) authorized: Arc<Notify>,
    pub(crate) shutdown: CancellationToken,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn budget_bounds_concurrency_and_reconnect_rate() {
        let budget = HandshakeBudget::default();
        let permits = (0..MAX_CONCURRENT_HANDSHAKES)
            .map(|_| budget.try_admit().unwrap())
            .collect::<Vec<_>>();
        assert!(budget.try_admit().is_none());
        drop(permits);
        for _ in MAX_CONCURRENT_HANDSHAKES..MAX_HANDSHAKES_PER_SECOND {
            drop(budget.try_admit().unwrap());
        }
        assert!(budget.try_admit().is_none());
        tokio::time::advance(HANDSHAKE_WINDOW).await;
        assert!(budget.try_admit().is_some());
    }
}
