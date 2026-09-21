use std::sync::Arc;
use std::time::Duration;

use codex_models_manager::manager::RefreshStrategy;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::model_catalog::ModelCatalog;

const MODELS_REFRESH_INTERVAL: Duration = Duration::from_secs(4 * 60 + 30);

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

pub(crate) fn spawn(model_catalog: &Arc<ModelCatalog>) -> ModelsRefreshWorker {
    spawn_with_interval(model_catalog, MODELS_REFRESH_INTERVAL)
}

fn spawn_with_interval(
    model_catalog: &Arc<ModelCatalog>,
    refresh_interval: Duration,
) -> ModelsRefreshWorker {
    let model_catalog = Arc::downgrade(model_catalog);
    let shutdown = CancellationToken::new();
    let worker_shutdown = shutdown.clone();
    let task = tokio::spawn(async move {
        loop {
            if worker_shutdown.is_cancelled() {
                break;
            }
            let Some(model_catalog) = model_catalog.upgrade() else {
                break;
            };
            if let Err(err) = model_catalog.list_models(RefreshStrategy::Online).await {
                // Parser diagnostics can include provider credentials from the source TOML.
                tracing::warn!(error_kind = ?err.kind(), "model catalog refresh blocked by provider requirements");
            }
            drop(model_catalog);

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
