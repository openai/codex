//! File ignore service for consistent filtering across codex tools.
//!
//! This crate provides a unified service for handling ignore files:
//! - `.gitignore` - Standard Git ignore rules
//! - `.ignore` - ripgrep native ignore rules
//! - Custom exclude patterns
//!
//! # Usage
//!
//! ```rust,no_run
//! use codex_file_ignore::{IgnoreService, IgnoreConfig};
//! use std::path::Path;
//!
//! // Default: respects both .gitignore and .ignore
//! let service = IgnoreService::with_defaults();
//! let walker = service.create_walk_builder(Path::new("."));
//!
//! for entry in walker.build() {
//!     // Process filtered files
//! }
//! ```
//!
//! # Configuration
//!
//! ```rust
//! use codex_file_ignore::{IgnoreService, IgnoreConfig};
//!
//! let config = IgnoreConfig {
//!     respect_gitignore: true,
//!     respect_ignore: true,
//!     include_hidden: false,
//!     follow_links: false,
//!     custom_excludes: vec!["*.log".to_string()],
//! };
//! let service = IgnoreService::new(config);
//! ```

mod config;
mod matcher;
mod patterns;
mod service;

// Primary API
pub use config::IgnoreConfig;
pub use matcher::PatternMatcher;
pub use service::IgnoreService;

// Standalone functions for external tool integration
pub use service::IGNORE_FILES;
pub use service::find_ignore_files;

// Pattern constants for consumers who need them
pub use patterns::BINARY_FILE_PATTERNS;
pub use patterns::COMMON_DIRECTORY_EXCLUDES;
pub use patterns::COMMON_IGNORE_PATTERNS;
pub use patterns::SYSTEM_FILE_EXCLUDES;
pub use patterns::get_all_default_excludes;

// Re-export WalkBuilder for convenience
// Consumers don't need to add `ignore` as a direct dependency
pub use ignore::WalkBuilder;
