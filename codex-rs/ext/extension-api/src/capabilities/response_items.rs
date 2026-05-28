//! Host capability for injecting extension-owned model input into an active turn.

use std::future::Future;
use std::pin::Pin;

use crate::HiddenContext;
use codex_protocol::models::ResponseInputItem;

/// Extension-owned input to inject into an active model turn.
#[derive(Debug, Clone)]
pub enum ResponseInjectionItem {
    /// Hidden context whose marker wrapping is owned by the extension API.
    HiddenContext(HiddenContext),
    /// Raw response item for extensions that intentionally need unwrapped input.
    Raw(ResponseInputItem),
}

impl ResponseInjectionItem {
    pub fn into_response_input_item(self) -> ResponseInputItem {
        match self {
            Self::HiddenContext(context) => context.into_response_input_item(),
            Self::Raw(item) => item,
        }
    }
}

impl From<HiddenContext> for ResponseInjectionItem {
    fn from(context: HiddenContext) -> Self {
        Self::HiddenContext(context)
    }
}

impl From<ResponseInputItem> for ResponseInjectionItem {
    fn from(item: ResponseInputItem) -> Self {
        Self::Raw(item)
    }
}

/// Future returned when an extension asks the host to inject model input.
pub type ResponseItemInjectionFuture<'a> =
    Pin<Box<dyn Future<Output = Result<(), Vec<ResponseInjectionItem>>> + Send + 'a>>;

/// Thread-scoped host helper for extensions that need to steer the active model turn.
///
/// Implementations should inject the supplied items into the active turn for
/// the current thread when it can accept same-turn model input. If injection is
/// unavailable, they return the unchanged items to the caller.
pub trait ResponseItemInjector: Send + Sync {
    fn inject_response_items<'a>(
        &'a self,
        items: Vec<ResponseInjectionItem>,
    ) -> ResponseItemInjectionFuture<'a>;
}

/// Injector used when a host does not expose same-turn model steering.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopResponseItemInjector;

impl ResponseItemInjector for NoopResponseItemInjector {
    fn inject_response_items<'a>(
        &'a self,
        items: Vec<ResponseInjectionItem>,
    ) -> ResponseItemInjectionFuture<'a> {
        Box::pin(std::future::ready(Err(items)))
    }
}
