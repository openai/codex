use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use tokio::sync::Mutex;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Debug)]
pub(crate) struct Stopwatch {
    limit: Duration,
    inner: Arc<Mutex<StopwatchState>>,
    notify: Arc<Notify>,
}

#[derive(Debug)]
struct StopwatchState {
    elapsed: Duration,
    running_since: Option<Instant>,
    active_pauses: u32,
}

impl Stopwatch {
    pub(crate) fn new(limit: Duration) -> Self {
        Self {
            inner: Arc::new(Mutex::new(StopwatchState {
                elapsed: Duration::ZERO,
                running_since: Some(Instant::now()),
                active_pauses: 0,
            })),
            notify: Arc::new(Notify::new()),
            limit,
        }
    }

    pub(crate) fn cancellation_token(&self) -> CancellationToken {
        let limit = self.limit;
        let token = CancellationToken::new();
        let cancel = token.clone();
        let inner = Arc::clone(&self.inner);
        let notify = Arc::clone(&self.notify);
        tokio::spawn(async move {
            loop {
                let (remaining, running) = {
                    let guard = inner.lock().await;
                    let elapsed = guard.elapsed
                        + guard
                            .running_since
                            .map(|since| since.elapsed())
                            .unwrap_or_default();
                    if elapsed >= limit {
                        break;
                    }
                    (limit - elapsed, guard.running_since.is_some())
                };

                if !running {
                    notify.notified().await;
                    continue;
                }

                let sleep = tokio::time::sleep(remaining);
                tokio::pin!(sleep);
                tokio::select! {
                    _ = &mut sleep => {
                        break;
                    }
                    _ = notify.notified() => {
                        continue;
                    }
                }
            }
            cancel.cancel();
        });
        token
    }

    /// Runs `fut`, pausing the stopwatch while the future is pending. The clock
    /// resumes automatically when the future completes. Nested/overlapping
    /// calls are reference-counted so the stopwatch only resumes when every
    /// pause is lifted.
    pub(crate) async fn pause_for<F, T>(&self, fut: F) -> T
    where
        F: Future<Output = T>,
    {
        self.pause().await;
        let result = fut.await;
        self.resume().await;
        result
    }

    async fn pause(&self) {
        let mut guard = self.inner.lock().await;
        guard.active_pauses += 1;
        if guard.active_pauses == 1
            && let Some(since) = guard.running_since.take()
        {
            guard.elapsed += since.elapsed();
            self.notify.notify_waiters();
        }
    }

    async fn resume(&self) {
        let mut guard = self.inner.lock().await;
        if guard.active_pauses == 0 {
            return;
        }
        guard.active_pauses -= 1;
        if guard.active_pauses == 0 && guard.running_since.is_none() {
            guard.running_since = Some(Instant::now());
            self.notify.notify_waiters();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Stopwatch;
    use tokio::time::Duration;
    use tokio::time::Instant;
    use tokio::time::sleep;
    use tokio::time::timeout;

    #[tokio::test]
    async fn cancellation_receiver_fires_after_limit() {
        let stopwatch = Stopwatch::new(Duration::from_millis(50));
        let token = stopwatch.cancellation_token();
        let start = Instant::now();
        token.cancelled().await;
        assert!(start.elapsed() >= Duration::from_millis(50));
    }

    #[tokio::test]
    async fn pause_prevents_timeout_until_resumed() {
        let stopwatch = Stopwatch::new(Duration::from_millis(50));
        let token = stopwatch.cancellation_token();

        let pause_handle = tokio::spawn({
            let stopwatch = stopwatch.clone();
            async move {
                stopwatch
                    .pause_for(async {
                        sleep(Duration::from_millis(100)).await;
                    })
                    .await;
            }
        });

        assert!(
            timeout(Duration::from_millis(30), token.cancelled())
                .await
                .is_err()
        );

        pause_handle.await.expect("pause task should finish");

        token.cancelled().await;
    }

    #[tokio::test]
    async fn overlapping_pauses_only_resume_once() {
        let stopwatch = Stopwatch::new(Duration::from_millis(50));
        let token = stopwatch.cancellation_token();

        // First pause.
        let pause1 = {
            let stopwatch = stopwatch.clone();
            tokio::spawn(async move {
                stopwatch
                    .pause_for(async {
                        sleep(Duration::from_millis(80)).await;
                    })
                    .await;
            })
        };

        // Overlapping pause that ends sooner.
        let pause2 = {
            let stopwatch = stopwatch.clone();
            tokio::spawn(async move {
                stopwatch
                    .pause_for(async {
                        sleep(Duration::from_millis(30)).await;
                    })
                    .await;
            })
        };

        // While both pauses are active, the cancellation should not fire.
        assert!(
            timeout(Duration::from_millis(40), token.cancelled())
                .await
                .is_err()
        );

        pause2.await.expect("short pause should complete");

        // Still paused because the long pause is active.
        assert!(
            timeout(Duration::from_millis(30), token.cancelled())
                .await
                .is_err()
        );

        pause1.await.expect("long pause should complete");

        // Now the stopwatch should resume and hit the limit shortly after.
        token.cancelled().await;
    }
}
