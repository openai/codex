//! Hidden user-context wrappers owned by extensions.
//!
//! Extensions can use one of these host-recognized tags when they need hidden
//! model-visible context to survive history filtering without teaching core
//! about the extension's domain concepts.

use super::ContextualUserFragment;
use super::fragment::matches_marked_text;

/// Host-recognized marker pair for extension-owned hidden context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtensionContextTag {
    start_marker: &'static str,
    end_marker: &'static str,
}

impl ExtensionContextTag {
    pub const DEFAULT: Self = Self::new("<extension_context>", "</extension_context>");
    pub const GOAL: Self = Self::new("<goal_context>", "</goal_context>");

    const fn new(start_marker: &'static str, end_marker: &'static str) -> Self {
        Self {
            start_marker,
            end_marker,
        }
    }
}

const EXTENSION_CONTEXT_TAGS: &[ExtensionContextTag] =
    &[ExtensionContextTag::DEFAULT, ExtensionContextTag::GOAL];

/// Hidden user-context fragment for extension-owned steering prompts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionContext {
    tag: ExtensionContextTag,
    body: String,
}

impl ExtensionContext {
    pub fn new(body: impl Into<String>) -> Self {
        Self::with_tag(ExtensionContextTag::DEFAULT, body)
    }

    pub fn with_tag(tag: ExtensionContextTag, body: impl Into<String>) -> Self {
        Self {
            tag,
            body: body.into(),
        }
    }
}

impl ContextualUserFragment for ExtensionContext {
    fn role() -> &'static str {
        "user"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        (self.tag.start_marker, self.tag.end_marker)
    }

    fn type_markers() -> (&'static str, &'static str) {
        (
            ExtensionContextTag::DEFAULT.start_marker,
            ExtensionContextTag::DEFAULT.end_marker,
        )
    }

    fn matches_text(text: &str) -> bool {
        EXTENSION_CONTEXT_TAGS
            .iter()
            .any(|tag| matches_marked_text(tag.start_marker, tag.end_marker, text))
    }

    fn body(&self) -> String {
        format!("\n{}\n", self.body)
    }
}
