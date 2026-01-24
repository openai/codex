//! Retrieval services module.
//!
//! Contains focused services for retrieval operations:
//!
//! - [`SearchService`] - Query and search operations
//! - [`SearchRequest`] - Unified search request type
//! - [`IndexService`] - Index management and pipeline control
//! - [`RecentFilesService`] - Recently accessed files tracking
//!
//! For facade construction, use [`crate::FacadeBuilder`] or [`crate::RetrievalFacade::for_workdir`].

pub mod index;
pub mod recent;
pub mod search;

pub use index::IndexService;
pub use recent::RecentFilesService;
pub use search::SearchRequest;
pub use search::SearchService;
