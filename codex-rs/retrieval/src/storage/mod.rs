//! Storage module.
//!
//! Provides SQLite and LanceDB storage backends.
//!
//! LanceDB is used for vector storage and search with extended metadata.
//! SQLite is used for lock/checkpoint management and query caching.

pub mod lancedb;
pub mod lancedb_types;
pub mod snippets;
pub mod sqlite;

pub use lancedb::LanceDbStore;
pub use lancedb_types::FileMetadata;
pub use lancedb_types::IndexPolicy;
pub use lancedb_types::IndexPolicyConfig;
pub use lancedb_types::IndexStatus;
pub use snippets::SnippetStorage;
pub use snippets::StoredSnippet;
pub use snippets::SymbolQuery;
pub use sqlite::SqliteStore;
