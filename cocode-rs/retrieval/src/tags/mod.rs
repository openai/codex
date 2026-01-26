//! Tag extraction module.
//!
//! Uses tree-sitter-tags to extract function, class, and method definitions.

pub mod extractor;
pub mod languages;

pub use extractor::CodeTag;
pub use extractor::TagExtractor;
pub use extractor::TagKind;
pub use extractor::find_parent_impl;
pub use extractor::find_parent_symbol;
pub use extractor::get_parent_context;
pub use languages::SupportedLanguage;
