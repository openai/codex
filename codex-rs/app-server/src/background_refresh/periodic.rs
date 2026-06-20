use std::future::Future;
use std::time::Duration;

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RefreshControl {
    Continue,
    Stop,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum InitialRefresh {
    Immediate,
    AfterInterval,
}

#[derive(Debug)]
pub(crate) struct PeriodicRefreshWorker {
    shutdown: CancellationToken,
    _task: JoinHandle<()>,
}

impl PeriodicRefreshWorker {
    pub(crate) fn shutdown(&self) {
        self.shutdown.cancel();
    }
}

impl Drop for PeriodicRefreshWorker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub(super) fn spawn<F, Fut>(
    refresh_interval: Duration,
    initial_refresh: InitialRefresh,
    mut refresh: F,
) -> PeriodicRefreshWorker
where
    F: FnMut() -> Fut + Send + 'static,
    Fut: Future<Output = RefreshControl> + Send + 'static,
{
    let shutdown = CancellationToken::new();
    let worker_shutdown = shutdown.clone();
    let task = tokio::spawn(async move {
        let mut delay_before_refresh = initial_refresh == InitialRefresh::AfterInterval;
        loop {
            if delay_before_refresh {
                tokio::select! {
                    _ = worker_shutdown.cancelled() => break,
                    _ = tokio::time::sleep(refresh_interval) => {}
                }
            }
            let refresh_control = tokio::select! {
                _ = worker_shutdown.cancelled() => break,
                refresh_control = refresh() => refresh_control,
            };
            if refresh_control == RefreshControl::Stop {
                break;
            }
            delay_before_refresh = true;
        }
    });
    PeriodicRefreshWorker {
        shutdown,
        _task: task,
    }
}

#[cfg(test)]
#[path = "periodic_tests.rs"]
mod tests;
