use std::sync::Arc;
use std::time::Duration;

use codex_models_manager::manager::RefreshStrategy;
use codex_models_manager::manager::SharedModelsManager;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

const MODELS_REFRESH_INTERVAL: Duration = Duration::from_secs(3 * 60);

#[derive(Debug)]
pub(crate) struct ModelsRefreshWorker {
    shutdown: CancellationToken,
    _task: JoinHandle<()>,
}

impl ModelsRefreshWorker {
    pub(crate) fn shutdown(&self) {
        self.shutdown.cancel();
    }
}

impl Drop for ModelsRefreshWorker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub(crate) fn spawn(models_manager: &SharedModelsManager) -> ModelsRefreshWorker {
    spawn_with_interval(models_manager, MODELS_REFRESH_INTERVAL)
}

fn spawn_with_interval(
    models_manager: &SharedModelsManager,
    refresh_interval: Duration,
) -> ModelsRefreshWorker {
    let models_manager = Arc::downgrade(models_manager);
    let shutdown = CancellationToken::new();
    let worker_shutdown = shutdown.clone();
    let task = tokio::spawn(async move {
        loop {
            if worker_shutdown.is_cancelled() {
                break;
            }
            let Some(models_manager) = models_manager.upgrade() else {
                break;
            };
            models_manager.list_models(RefreshStrategy::Online).await;
            drop(models_manager);

            tokio::select! {
                _ = worker_shutdown.cancelled() => break,
                _ = tokio::time::sleep(refresh_interval) => {}
            }
        }
    });
    ModelsRefreshWorker {
        shutdown,
        _task: task,
    }
}

#[cfg(test)]
#[path = "models_refresh_worker_tests.rs"]
mod tests;
