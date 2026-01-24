//! Tag extraction pipeline for RepoMap functionality.
//!
//! This module provides tag extraction infrastructure that is managed by
//! the UnifiedCoordinator alongside the search index pipeline.

mod pipeline;

pub use pipeline::SharedTagPipeline;
pub use pipeline::TagEventProcessor;
pub use pipeline::TagPipeline;
pub use pipeline::TagPipelineState;
pub use pipeline::TagReadiness;
pub use pipeline::TagStats;
pub use pipeline::TagStrictModeConfig;
pub use pipeline::TagWorkerPool;
