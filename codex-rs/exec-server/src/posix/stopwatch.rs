use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use tokio::sync::Mutex;
use tokio::sync::Notify;
use tokio::sync::oneshot;

#[derive(Clone, Debug)]
pub(crate) struct Stopwatch {
    limit: Option<Duration>,
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
    pub(crate) fn new(limit_ms: Option<u64>) -> Self {
        let limit = limit_ms.map(Duration::from_millis);
        Self {
            inner: Arc::new(Mutex::new(StopwatchState {
                elapsed: Duration::ZERO,
                running_since: limit.map(|_| Instant::now()),
                active_pauses: 0,
            })),
            notify: Arc::new(Notify::new()),
            limit,
        }
    }

    pub(crate) fn cancellation_receiver(&self) -> Option<oneshot::Receiver<()>> {
        let limit = self.limit?;
        let (tx, rx) = oneshot::channel();
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

                if remaining.is_zero() {
                    break;
                }

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
            let _ = tx.send(());
        });
        Some(rx)
    }

    /// Runs `fut`, pausing the stopwatch while the future is pending. The clock resumes
    /// automatically when the future completes. Nested calls are reference-counted so the
    /// stopwatch only resumes when every pause is lifted.
    pub(crate) async fn pause_for<F, T>(&self, fut: F) -> T
    where
        F: Future<Output = T>,
    {
        if self.limit.is_none() {
            return fut.await;
        }
        self.pause().await;
        let result = fut.await;
        self.resume().await;
        result
    }

    async fn pause(&self) {
        if self.limit.is_none() {
            return;
        }
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
        if self.limit.is_none() {
            return;
        }
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
        let stopwatch = Stopwatch::new(Some(50));
        let rx = stopwatch
            .cancellation_receiver()
            .expect("stopwatch should have cancellation receiver");
        let start = Instant::now();
        rx.await.expect("cancellation should fire");
        assert!(start.elapsed() >= Duration::from_millis(50));
    }

    #[tokio::test]
    async fn pause_prevents_timeout_until_resumed() {
        let stopwatch = Stopwatch::new(Some(50));
        let mut rx = stopwatch
            .cancellation_receiver()
            .expect("stopwatch should have cancellation receiver");

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

        assert!(timeout(Duration::from_millis(30), &mut rx).await.is_err());

        pause_handle.await.expect("pause task should finish");

        rx.await
            .expect("cancellation should eventually fire after resume");
    }
}
