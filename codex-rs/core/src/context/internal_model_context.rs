//! Hidden user-context fragment for extension-owned model steering.

use super::ContextualUserFragment;
use std::error::Error;
use std::fmt;

const CONTEXT_START_MARKER: &str = "<codex_internal_context";
const CONTEXT_END_MARKER: &str = "</codex_internal_context>";
const LEGACY_GOAL_CONTEXT_START_MARKER: &str = "<goal_context>";
const LEGACY_GOAL_CONTEXT_END_MARKER: &str = "</goal_context>";
const SOURCE_ATTR_START: &str = " source=\"";
const SOURCE_ATTR_END: &str = "\">";

/// Source label for hidden internal model context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InternalContextSource(String);

impl InternalContextSource {
    /// Creates a source label for an internal model-context fragment.
    ///
    /// Sources are intentionally constrained so the value can be embedded in the
    /// wrapper without escaping and still remain easy to audit in stored history.
    pub fn new(source: impl Into<String>) -> Result<Self, InvalidInternalContextSource> {
        let source = source.into();
        if is_valid_source(&source) {
            Ok(Self(source))
        } else {
            Err(InvalidInternalContextSource { source })
        }
    }

    /// Creates a source label from a trusted static string.
    pub fn from_static(source: &'static str) -> Self {
        Self::new(source)
            .unwrap_or_else(|_| panic!("invalid static internal context source: {source}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Error returned when an internal model-context source is invalid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidInternalContextSource {
    source: String,
}

impl fmt::Display for InvalidInternalContextSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let source = &self.source;
        write!(
            f,
            "invalid internal model context source {source:?}; expected [a-z][a-z0-9_]*"
        )
    }
}

impl Error for InvalidInternalContextSource {}

/// Hidden runtime-owned context injected into model input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InternalModelContextFragment {
    source: InternalContextSource,
    body: String,
}

impl InternalModelContextFragment {
    /// Creates hidden model context with an extension-owned source label.
    pub fn new(source: InternalContextSource, body: impl Into<String>) -> Self {
        Self {
            source,
            body: body.into(),
        }
    }

    pub(crate) fn parse_canonical(text: &str) -> Option<Self> {
        let rest = text.strip_prefix(CONTEXT_START_MARKER)?;
        let rest = rest.strip_prefix(SOURCE_ATTR_START)?;
        let (source, body_and_close) = rest.split_once(SOURCE_ATTR_END)?;
        if !is_valid_source(source) {
            return None;
        }

        let body = body_and_close.strip_suffix(CONTEXT_END_MARKER)?;
        let body = body.strip_prefix('\n')?;
        let body = body.strip_suffix('\n')?;

        Some(Self::new(
            InternalContextSource(source.to_string()),
            body.to_string(),
        ))
    }

    pub(crate) fn source(&self) -> &InternalContextSource {
        &self.source
    }

    pub(crate) fn body(&self) -> &str {
        &self.body
    }
}

impl ContextualUserFragment for InternalModelContextFragment {
    fn role(&self) -> &'static str {
        "user"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        (CONTEXT_START_MARKER, CONTEXT_END_MARKER)
    }

    fn matches_text(text: &str) -> bool {
        let trimmed = text.trim();
        if matches_legacy_goal_context(trimmed) {
            return true;
        }

        Self::parse_canonical(trimmed).is_some()
    }

    fn body(&self) -> String {
        let source = self.source().as_str();
        let body = self.body();
        format!(" source=\"{source}\">\n{body}\n")
    }
}

fn matches_legacy_goal_context(text: &str) -> bool {
    text.starts_with(LEGACY_GOAL_CONTEXT_START_MARKER)
        && text.ends_with(LEGACY_GOAL_CONTEXT_END_MARKER)
}

fn is_valid_source(source: &str) -> bool {
    let mut chars = source.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_lowercase()
        && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
}
