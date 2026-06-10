use std::future::Future;
use std::pin::Pin;

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
