//! Storage-neutral interfaces for loading config-layer documents.
//!
//! Implementations should report observations from their backing store. Codex config loading
//! remains responsible for applying precedence, project trust, path resolution, requirements, and
//! final layer merging.
//!
//! The request and response types in this crate may cross process or network boundaries. Keep them
//! wire-friendly: prefer primitive fields over Rust-specific error or filesystem types.

mod error;
mod local;
mod store;
mod types;

pub use error::ConfigStoreError;
pub use error::ConfigStoreResult;
pub use local::LocalConfigStore;
pub use store::ConfigDocumentStore;
pub use types::ConfigDocumentErrorSpan;
pub use types::ConfigDocumentRead;
pub use types::ReadConfigDocumentParams;
