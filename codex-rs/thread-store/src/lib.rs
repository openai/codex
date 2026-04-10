//! Storage-neutral thread persistence interfaces.
//!
//! Application code should treat [`codex_protocol::ThreadId`] as the only durable thread handle.
//! Implementations are responsible for resolving that id to local rollout files, HTTP requests, or
//! any other backing store.

mod error;
mod local;
mod recorder;
mod store;
mod types;

pub use error::ThreadStoreError;
pub use error::ThreadStoreResult;
pub use local::LocalThreadStore;
pub use recorder::ThreadRecorder;
pub use store::ThreadStore;
pub use types::AppendThreadItemsParams;
pub use types::ArchiveThreadParams;
pub use types::CreateThreadParams;
pub use types::DynamicToolsParams;
pub use types::FindThreadByNameParams;
pub use types::FindThreadSpawnByPathParams;
pub use types::GitInfoPatch;
pub use types::ListThreadSpawnEdgesParams;
pub use types::ListThreadsParams;
pub use types::LoadThreadHistoryParams;
pub use types::OptionalStringPatch;
pub use types::ReadThreadParams;
pub use types::ResolveLegacyPathParams;
pub use types::ResumeThreadRecorderParams;
pub use types::SetThreadMemoryModeParams;
pub use types::SetThreadNameParams;
pub use types::StoredThread;
pub use types::StoredThreadHistory;
pub use types::ThreadEventPersistenceMode;
pub use types::ThreadMemoryModeParams;
pub use types::ThreadMetadataPatch;
pub use types::ThreadOwner;
pub use types::ThreadPage;
pub use types::ThreadSortKey;
pub use types::ThreadSpawnEdge;
pub use types::ThreadSpawnEdgeStatus;
pub use types::UpdateThreadMetadataParams;
