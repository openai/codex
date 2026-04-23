//! Storage-neutral thread persistence interfaces.
//!
//! Application code should treat [`codex_protocol::ThreadId`] as the only durable thread handle.
//! Implementations are responsible for resolving that id to local rollout files, RPC requests, or
//! any other backing store.

mod error;
mod live_thread;
mod local;
mod remote;
mod store;
#[cfg(any(test, debug_assertions))]
mod test_registry;
mod types;

pub use error::ThreadStoreError;
pub use error::ThreadStoreResult;
pub use live_thread::LiveThread;
pub use live_thread::LiveThreadInitGuard;
pub use local::LocalThreadStore;
pub use remote::RemoteThreadStore;
pub use store::ThreadStore;
#[cfg(any(test, debug_assertions))]
pub use test_registry::register_test_thread_store;
#[cfg(any(test, debug_assertions))]
pub use test_registry::remove_test_thread_store;
#[cfg(any(test, debug_assertions))]
pub use test_registry::test_thread_store;
pub use types::AppendThreadItemsParams;
pub use types::ArchiveThreadParams;
pub use types::CreateThreadParams;
pub use types::GitInfoPatch;
pub use types::ListThreadsParams;
pub use types::LoadThreadHistoryParams;
pub use types::OptionalStringPatch;
pub use types::ReadThreadByRolloutPathParams;
pub use types::ReadThreadParams;
pub use types::ResumeThreadParams;
pub use types::SortDirection;
pub use types::StoredThread;
pub use types::StoredThreadHistory;
pub use types::ThreadEventPersistenceMode;
pub use types::ThreadMetadataPatch;
pub use types::ThreadPage;
pub use types::ThreadSortKey;
pub use types::UpdateThreadMetadataParams;
