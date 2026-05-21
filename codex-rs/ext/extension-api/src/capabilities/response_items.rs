use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;

use codex_protocol::models::ResponseInputItem;

/// Future returned when an extension asks the host to inject model-visible input.
pub type ResponseItemInjectionFuture<'a> =
    Pin<Box<dyn Future<Output = Result<(), Vec<ResponseInputItem>>> + Send + 'a>>;

/// Host-provided helper for extensions that need to steer the active model turn.
///
/// Implementations should inject the supplied response items into the active turn
/// when one can accept same-turn model input. If injection is unavailable, they
/// return the unchanged items to the caller.
pub trait ResponseItemInjector: Send + Sync {
    fn inject_response_items<'a>(
        &'a self,
        items: Vec<ResponseInputItem>,
    ) -> ResponseItemInjectionFuture<'a>;
}

/// Thread-scoped slot that lets the host publish a late-bound response-item
/// injector after extension thread-start hooks have already run.
#[derive(Default)]
pub struct ResponseItemInjectorSlot {
    injector: Mutex<Option<Arc<dyn ResponseItemInjector>>>,
}

impl std::fmt::Debug for ResponseItemInjectorSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResponseItemInjectorSlot")
            .finish_non_exhaustive()
    }
}

impl ResponseItemInjectorSlot {
    /// Replaces the published injector for this thread scope.
    pub fn set(&self, injector: Arc<dyn ResponseItemInjector>) {
        *self.injector() = Some(injector);
    }

    /// Returns the published injector, if the host has provided one yet.
    pub fn get(&self) -> Option<Arc<dyn ResponseItemInjector>> {
        self.injector().clone()
    }

    fn injector(&self) -> std::sync::MutexGuard<'_, Option<Arc<dyn ResponseItemInjector>>> {
        self.injector.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Injector used when a host does not expose same-turn model steering.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopResponseItemInjector;

impl ResponseItemInjector for NoopResponseItemInjector {
    fn inject_response_items<'a>(
        &'a self,
        items: Vec<ResponseInputItem>,
    ) -> ResponseItemInjectionFuture<'a> {
        Box::pin(std::future::ready(Err(items)))
    }
}
