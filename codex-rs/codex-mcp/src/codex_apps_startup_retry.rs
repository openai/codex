use std::sync::Arc;
use std::sync::OnceLock;

use futures::FutureExt;

use crate::rmcp_client::ManagedClientFuture;
use crate::rmcp_client::StartupOutcomeError;

// Recreated startup bypasses the tools cache and rewrites it on success. The
// first attempt recreates the client; the second is the one final cache-refresh
// retry before the manager keeps serving its startup snapshot.
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
