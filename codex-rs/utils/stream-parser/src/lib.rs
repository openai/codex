//! Streaming parsers for text that arrives in chunks.
//!
//! This crate is intentionally small and dependency-free. It provides:
//! - a generic [`StreamTextParser`] trait for incremental text parsers, and
//! - reusable parsers for hidden inline tags such as `<citation>...</citation>`.
//!
//! See the crate `README.md` for usage examples and extension guidance.

mod citation;
mod inline_hidden_tag;
mod stream_text;
mod utf8_stream;

pub use citation::CitationStreamParser;
pub use citation::strip_citations;
pub use inline_hidden_tag::ExtractedInlineTag;
pub use inline_hidden_tag::InlineHiddenTagParser;
pub use inline_hidden_tag::InlineTagSpec;
pub use stream_text::StreamTextChunk;
pub use stream_text::StreamTextParser;
pub use utf8_stream::Utf8StreamParser;
pub use utf8_stream::Utf8StreamParserError;
