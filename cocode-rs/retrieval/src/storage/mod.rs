//! Storage module.
//!
//! Provides SQLite-vec vector store and SQLite metadata backends.

pub mod chunk_types;
pub mod snippets;
pub mod sqlite;
pub mod sqlite_vec;
pub mod vector_store;

pub use chunk_types::FileMetadata;
pub use chunk_types::IndexPolicy;
pub use chunk_types::IndexPolicyConfig;
pub use chunk_types::IndexStatus;
pub use snippets::SnippetStorage;
pub use snippets::StoredSnippet;
pub use snippets::SymbolQuery;
pub use sqlite::SqliteStore;
pub use sqlite_vec::SqliteVecStore;
pub use vector_store::VectorStore;
