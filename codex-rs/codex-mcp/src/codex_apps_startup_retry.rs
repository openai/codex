use std::sync::Arc;
use std::sync::OnceLock;

use futures::FutureExt;

use crate::rmcp_client::ManagedClientFuture;
use crate::rmcp_client::StartupOutcomeError;

// Every recreated startup performs a fresh initialize + uncached tools/list and
// rewrites the tools cache on success. Bound recovery to two shared attempts
// before the manager keeps serving its startup snapshot.
pub(crate) struct CodexAppsStartupRetry {
    factory: Arc<dyn Fn() -> ManagedClientFuture + Send + Sync>,
    retry: OnceLock<ManagedClientFuture>,
}

impl CodexAppsStartupRetry {
    pub(crate) fn new(factory: Arc<dyn Fn() -> ManagedClientFuture + Send + Sync>) -> Self {
        Self {
            factory,
            retry: OnceLock::new(),
        }
    }

    pub(crate) fn replacement(&self) -> Option<ManagedClientFuture> {
        self.retry.get().cloned()
    }

    pub(crate) async fn retry(&self) {
        let retry = self
            .retry
            .get_or_init(|| {
                let factory = Arc::clone(&self.factory);
                async move {
                    match (factory)().await {
                        Ok(client) => Ok(client),
                        Err(StartupOutcomeError::Failed { .. }) => (factory)().await,
                        Err(StartupOutcomeError::Cancelled) => Err(StartupOutcomeError::Cancelled),
                    }
                }
                .boxed()
                .shared()
            })
            .clone();
        let _ = retry.await;
    }
}
