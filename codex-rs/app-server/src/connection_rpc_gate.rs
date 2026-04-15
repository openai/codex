use std::future::Future;

use tokio::sync::Mutex;
use tokio_util::task::TaskTracker;

/// Per-connection gate for initialized RPC handler execution.
///
/// Closing the gate prevents queued handlers from starting while allowing
/// handlers that already acquired a token to finish.
#[derive(Debug)]
pub(crate) struct ConnectionRpcGate {
    accepting: Mutex<bool>,
    tasks: TaskTracker,
}

impl ConnectionRpcGate {
    pub(crate) fn new() -> Self {
        Self {
            accepting: Mutex::new(true),
            tasks: TaskTracker::new(),
        }
    }

    pub(crate) async fn run<F>(&self, future: F)
    where
        F: Future<Output = ()>,
    {
        let token = {
            let accepting = self.accepting.lock().await;
            if !*accepting {
                return;
            }
            self.tasks.token()
        };

        future.await;
        drop(token);
    }

    pub(crate) async fn shutdown(&self) {
        {
            let mut accepting = self.accepting.lock().await;
            *accepting = false;
            self.tasks.close();
        }
        self.tasks.wait().await;
    }

    #[cfg(test)]
    async fn is_accepting(&self) -> bool {
        *self.accepting.lock().await
    }

    #[cfg(test)]
    fn inflight_count(&self) -> usize {
        self.tasks.len()
    }
}

impl Default for ConnectionRpcGate {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;
    use tokio::sync::oneshot;
    use tokio::time::Duration;
    use tokio::time::timeout;

    #[tokio::test]
    async fn run_executes_while_open() {
        let gate = ConnectionRpcGate::new();
        let ran = Arc::new(AtomicBool::new(false));
        let ran_clone = Arc::clone(&ran);

        gate.run(async move {
            ran_clone.store(true, Ordering::Release);
        })
        .await;

        assert!(ran.load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn run_drops_future_without_polling_after_shutdown() {
        let gate = ConnectionRpcGate::new();
        gate.shutdown().await;
        let polled = Arc::new(AtomicBool::new(false));
        let polled_clone = Arc::clone(&polled);

        gate.run(async move {
            polled_clone.store(true, Ordering::Release);
        })
        .await;

        assert!(!polled.load(Ordering::Acquire));
        assert!(!gate.is_accepting().await);
    }

    #[tokio::test]
    async fn shutdown_waits_for_started_run_to_finish() {
        let gate = Arc::new(ConnectionRpcGate::new());
        let (started_tx, started_rx) = oneshot::channel();
        let (finish_tx, finish_rx) = oneshot::channel();
        let gate_for_run = Arc::clone(&gate);
        let run_task = tokio::spawn(async move {
            gate_for_run
                .run(async move {
                    started_tx.send(()).expect("receiver should be open");
                    let _ = finish_rx.await;
                })
                .await;
        });

        started_rx.await.expect("run should start");
        assert_eq!(gate.inflight_count(), 1);

        let gate_for_shutdown = Arc::clone(&gate);
        let shutdown_task = tokio::spawn(async move {
            gate_for_shutdown.shutdown().await;
        });

        timeout(Duration::from_millis(50), shutdown_task)
            .await
            .expect_err("shutdown should wait for the running future");

        finish_tx
            .send(())
            .expect("running future should be waiting");
        run_task.await.expect("run task should complete");
        gate.shutdown().await;
        assert_eq!(gate.inflight_count(), 0);
    }

    #[tokio::test]
    async fn shutdown_drops_late_runs_while_waiting_for_inflight_work() {
        let gate = Arc::new(ConnectionRpcGate::new());
        let (started_tx, started_rx) = oneshot::channel();
        let (finish_tx, finish_rx) = oneshot::channel();
        let gate_for_run = Arc::clone(&gate);
        let run_task = tokio::spawn(async move {
            gate_for_run
                .run(async move {
                    started_tx.send(()).expect("receiver should be open");
                    let _ = finish_rx.await;
                })
                .await;
        });

        started_rx.await.expect("run should start");
        let gate_for_shutdown = Arc::clone(&gate);
        let shutdown_task = tokio::spawn(async move {
            gate_for_shutdown.shutdown().await;
        });

        timeout(Duration::from_millis(50), shutdown_task)
            .await
            .expect_err("shutdown should wait for the running future");

        let late_polled = Arc::new(AtomicBool::new(false));
        let late_polled_clone = Arc::clone(&late_polled);
        gate.run(async move {
            late_polled_clone.store(true, Ordering::Release);
        })
        .await;

        assert!(!late_polled.load(Ordering::Acquire));

        finish_tx
            .send(())
            .expect("running future should still be waiting");
        run_task.await.expect("run task should complete");
        gate.shutdown().await;
        assert_eq!(gate.inflight_count(), 0);
    }

    #[tokio::test]
    async fn run_is_counted_before_handler_body_continues() {
        let gate = Arc::new(ConnectionRpcGate::new());
        let (entered_tx, entered_rx) = oneshot::channel();
        let (continue_tx, continue_rx) = oneshot::channel();
        let gate_for_run = Arc::clone(&gate);
        let run_task = tokio::spawn(async move {
            gate_for_run
                .run(async move {
                    entered_tx.send(()).expect("receiver should be open");
                    let _ = continue_rx.await;
                })
                .await;
        });

        entered_rx.await.expect("handler body should be entered");
        assert_eq!(gate.inflight_count(), 1);

        continue_tx
            .send(())
            .expect("handler body should still be waiting");
        run_task.await.expect("run task should complete");
        assert_eq!(gate.inflight_count(), 0);
    }
}
