use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::posix::stopwatch::Stopwatch;

#[derive(Clone, Debug, Default)]
pub(crate) struct StopwatchController {
    state: Arc<Mutex<StopwatchControllerState>>,
    operation_lock: Arc<Mutex<()>>,
}

#[derive(Debug, Default)]
struct StopwatchControllerState {
    paused: bool,
    next_stopwatch_id: u64,
    stopwatches: HashMap<u64, Stopwatch>,
}

impl StopwatchController {
    pub(crate) async fn register(&self, stopwatch: Stopwatch) -> u64 {
        let _operation_guard = self.operation_lock.lock().await;
        let (stopwatch_id, paused) = {
            let mut guard = self.state.lock().await;
            let stopwatch_id = guard.next_stopwatch_id;
            guard.next_stopwatch_id += 1;
            guard.stopwatches.insert(stopwatch_id, stopwatch.clone());
            (stopwatch_id, guard.paused)
        };

        if paused {
            stopwatch.pause().await;
        }

        stopwatch_id
    }

    pub(crate) async fn unregister(&self, stopwatch_id: u64) {
        let _operation_guard = self.operation_lock.lock().await;
        let (stopwatch, paused) = {
            let mut guard = self.state.lock().await;
            (guard.stopwatches.remove(&stopwatch_id), guard.paused)
        };

        if paused && let Some(stopwatch) = stopwatch {
            stopwatch.resume().await;
        }
    }

    pub(crate) async fn set_paused(&self, paused: bool) {
        let _operation_guard = self.operation_lock.lock().await;
        let stopwatches = {
            let mut guard = self.state.lock().await;
            if guard.paused == paused {
                return;
            }
            guard.paused = paused;
            guard.stopwatches.values().cloned().collect::<Vec<_>>()
        };

        for stopwatch in stopwatches {
            if paused {
                stopwatch.pause().await;
            } else {
                stopwatch.resume().await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::StopwatchController;
    use crate::posix::stopwatch::Stopwatch;
    use tokio::time::Duration;
    use tokio::time::timeout;

    #[tokio::test]
    async fn pausing_controller_pauses_registered_stopwatch() {
        let controller = StopwatchController::default();
        let stopwatch = Stopwatch::new(Duration::from_millis(50));
        let token = stopwatch.cancellation_token();

        let stopwatch_id = controller.register(stopwatch).await;
        controller.set_paused(true).await;

        assert!(
            timeout(Duration::from_millis(30), token.cancelled())
                .await
                .is_err()
        );

        controller.set_paused(false).await;
        controller.unregister(stopwatch_id).await;
        token.cancelled().await;
    }

    #[tokio::test]
    async fn registering_while_paused_starts_paused() {
        let controller = StopwatchController::default();
        controller.set_paused(true).await;

        let stopwatch = Stopwatch::new(Duration::from_millis(50));
        let token = stopwatch.cancellation_token();

        let stopwatch_id = controller.register(stopwatch).await;

        assert!(
            timeout(Duration::from_millis(30), token.cancelled())
                .await
                .is_err()
        );

        controller.set_paused(false).await;
        controller.unregister(stopwatch_id).await;
        token.cancelled().await;
    }

    #[tokio::test]
    async fn unregistering_while_paused_resumes_controller_pause() {
        let controller = StopwatchController::default();
        let stopwatch = Stopwatch::new(Duration::from_millis(50));
        let token = stopwatch.cancellation_token();

        let stopwatch_id = controller.register(stopwatch).await;
        controller.set_paused(true).await;

        assert!(
            timeout(Duration::from_millis(30), token.cancelled())
                .await
                .is_err()
        );

        controller.unregister(stopwatch_id).await;

        assert!(
            timeout(Duration::from_millis(120), token.cancelled())
                .await
                .is_ok()
        );
    }
}
