//! Durable user-submission queue and idle-dispatch extension.

use std::sync::Arc;

use codex_extension_api::ExtensionRegistryBuilder;

mod service;
mod types;

pub use service::QueueServiceError;
pub use service::QueuedItemService;
pub use types::QueuedItem;
pub use types::QueuedItemProvenance;
pub use types::QueuedItemStatus;

/// Installs durable queue dispatch into the host's thread and turn lifecycle.
pub fn install<C>(registry: &mut ExtensionRegistryBuilder<C>, service: Arc<QueuedItemService>)
where
    C: Send + Sync + 'static,
{
    registry.thread_lifecycle_contributor(service);
}
