//! Typed, bounded v2 memory context.

use super::ContextualUserFragment;
use codex_protocol::models::ContentItemKind;
use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_output_truncation::truncate_text;

/// Memory-owned model context, capped below 10k tokens even for byte-heavy text.
pub enum MemoryContextFragment {
    ReadInstructions(String),
    ExtractionEvidence(String),
}

impl ContextualUserFragment for MemoryContextFragment {
    fn role(&self) -> &'static str {
        match self {
            Self::ReadInstructions(_) => "developer",
            Self::ExtractionEvidence(_) => "user",
        }
    }
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind(
            match self {
                Self::ReadInstructions(_) => "memories.instructions",
                Self::ExtractionEvidence(_) => "memories.extraction_evidence",
            }
            .to_string(),
        )
    }
    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }
    fn type_markers() -> (&'static str, &'static str) {
        ("", "")
    }
    fn body(&self) -> String {
        let (Self::ReadInstructions(text) | Self::ExtractionEvidence(text)) = self;
        truncate_text(text, TruncationPolicy::Bytes(8_900))
    }
}
