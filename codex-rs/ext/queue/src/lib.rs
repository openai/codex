//! Durable user-submission queue and idle-dispatch extension.

mod service;
mod types;

pub use service::QueueServiceError;
pub use service::QueuedItemService;
pub use types::QueuedItem;
pub use types::QueuedItemProvenance;
pub use types::QueuedItemStatus;
