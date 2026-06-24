use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;

/// Type-erased registration for a contextual user fragment.
///
/// Implementations are used by context filtering code to recognize injected
/// fragments without constructing the concrete context payload.
pub trait FragmentRegistration: Sync {
    fn matches_text(&self, text: &str) -> bool;
}

pub struct FragmentRegistrationProxy<T> {
    _marker: std::marker::PhantomData<fn() -> T>,
}

impl<T> FragmentRegistrationProxy<T> {
    pub const fn new() -> Self {
        Self {
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T> Default for FragmentRegistrationProxy<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: ContextualUserFragment> FragmentRegistration for FragmentRegistrationProxy<T> {
    fn matches_text(&self, text: &str) -> bool {
        T::matches_text(text)
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

    fn into(self) -> ResponseItem
    where
        Self: Sized,
    {
        ResponseItem::Message {
            id: None,
            role: self.role().to_string(),
            content: vec![ContentItem::InputText {
                text: self.render(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        }
    }

    fn into_boxed_response_item(self: Box<Self>) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: self.role().to_string(),
            content: vec![ContentItem::InputText {
                text: self.render(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        }
    }

    fn into_response_input_item(self) -> ResponseInputItem
    where
        Self: Sized,
    {
        ResponseInputItem::Message {
            role: self.role().to_string(),
            content: vec![ContentItem::InputText {
                text: self.render(),
            }],
            phase: None,
        }
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
