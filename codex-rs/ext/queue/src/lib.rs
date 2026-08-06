//! Durable, storage-neutral user-message queue and idle dispatch.

use std::sync::Arc;

use codex_extension_api::ExtensionRegistryBuilder;

mod service;

pub use service::QueueServiceError;
pub use service::QueuedItem;
pub use service::QueuedItemService;

/// Registers the caller-owned queue before lower-priority idle contributors.
pub fn install<C>(registry: &mut ExtensionRegistryBuilder<C>, service: Arc<QueuedItemService>)
where
    C: Send + Sync + 'static,
{
    registry.thread_lifecycle_contributor(service);
}
