use crate::AnnotatedContent;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::InternalChatMessageMetadataPassthrough;
use codex_protocol::models::ResponseItem;

/// A rendered contextual fragment and the role that owns its annotated content.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderedFragment {
    role: &'static str,
    content: AnnotatedContent,
}

impl RenderedFragment {
    /// Creates a rendered fragment without separating its role and annotated content.
    pub fn new(role: &'static str, content: AnnotatedContent) -> Self {
        Self { role, content }
    }

    /// Returns the response role associated with this fragment.
    pub fn role(&self) -> &'static str {
        self.role
    }

    /// Returns this fragment's model-visible content and classification.
    pub fn annotated_content(&self) -> &AnnotatedContent {
        &self.content
    }

    /// Separates the role and annotated content at an API boundary.
    pub fn into_parts(self) -> (&'static str, AnnotatedContent) {
        (self.role, self.content)
    }
}

impl From<RenderedFragment> for ResponseItem {
    fn from(fragment: RenderedFragment) -> Self {
        let (role, annotated_content) = fragment.into_parts();
        let (content, content_kind) = annotated_content.into_parts();

        Self::Message {
            id: None,
            role: role.to_string(),
            content: vec![content],
            phase: None,
            internal_chat_message_metadata_passthrough: Some(
                InternalChatMessageMetadataPassthrough {
                    content_item_kinds: Some(vec![content_kind]),
                    ..Default::default()
                },
            ),
        }
    }
}

/// Context payload that is injected as a message fragment.
///
/// Implementations own the response role and provide the exact fragment body.
/// Marked fragments also provide start/end markers used to recognize injected
/// context later. `render()` concatenates markers and body without adding
/// separators, so implementations should include any whitespace they need
/// between tags in `body()`. Unmarked fragments should leave both markers empty,
/// in which case the default helpers render only the body and never match
/// arbitrary text.
pub trait ContextualUserFragment {
    fn role(&self) -> &'static str;

    /// Returns a stable `<feature>.<name>` classification, using `generic` for shared fragments.
    fn content_kind(&self) -> ContentItemKind;

    /// Whether this fragment must be recorded as its own response item.
    fn requires_separate_message(&self) -> bool {
        false
    }

    fn markers(&self) -> (&'static str, &'static str);

    fn body(&self) -> String;

    fn type_markers() -> (&'static str, &'static str)
    where
        Self: Sized;

    fn matches_text(text: &str) -> bool
    where
        Self: Sized,
    {
        let (start_marker, end_marker) = Self::type_markers();
        matches_marked_text(start_marker, end_marker, text)
    }

    fn render(&self) -> String {
        let (start_marker, end_marker) = self.markers();
        let body = self.body();
        if start_marker.is_empty() && end_marker.is_empty() {
            return body;
        }

        format!("{start_marker}{body}{end_marker}")
    }

    /// Renders the role, model-visible content, and classification together.
    fn render_fragment(&self) -> RenderedFragment {
        RenderedFragment::new(
            self.role(),
            AnnotatedContent::input_text(self.render(), self.content_kind()),
        )
    }

    fn into(self) -> ResponseItem
    where
        Self: Sized,
    {
        ResponseItem::from(self.render_fragment())
    }

    fn into_boxed_response_item(self: Box<Self>) -> ResponseItem {
        ResponseItem::from(self.render_fragment())
    }
}

pub(crate) fn matches_marked_text(start_marker: &str, end_marker: &str, text: &str) -> bool {
    if start_marker.is_empty() || end_marker.is_empty() {
        return false;
    }

    let trimmed = text.trim_start();
    let starts_with_marker = trimmed
        .get(..start_marker.len())
        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(start_marker));
    let trimmed = trimmed.trim_end();
    let ends_with_marker = trimmed
        .get(trimmed.len().saturating_sub(end_marker.len())..)
        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(end_marker));
    starts_with_marker && ends_with_marker
}
