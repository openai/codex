use std::future::Future;
use std::future::pending;

use tokio::task::JoinError;
use tokio::task::JoinSet;
use tracing::warn;

pub(crate) struct ConnectionCleanupTasks {
    tasks: JoinSet<()>,
}

impl ConnectionCleanupTasks {
    pub(crate) fn new() -> Self {
        Self {
            tasks: JoinSet::new(),
        }
    }

    pub(crate) fn spawn(&mut self, future: impl Future<Output = ()> + Send + 'static) {
        self.tasks.spawn(future);
    }

    pub(crate) async fn reap_next(&mut self) {
        if self.tasks.is_empty() {
            pending::<()>().await;
        }
        if let Some(result) = self.tasks.join_next().await {
            log_cleanup_result(result);
        }
    }

    pub(crate) async fn drain(&mut self) {
        while let Some(result) = self.tasks.join_next().await {
            log_cleanup_result(result);
        }
    }

    pub(crate) fn abort(&mut self) {
        self.tasks.abort_all();
    }
}

fn log_cleanup_result(result: Result<(), JoinError>) {
    if let Err(err) = result
        && !err.is_cancelled()
    {
        warn!("connection cleanup task failed: {err}");
    }
}
