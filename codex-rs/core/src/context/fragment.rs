use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use std::marker::PhantomData;

/// Type-erased registration for a contextual user fragment.
///
/// Implementations are used by context filtering code to recognize injected
/// fragments without constructing the concrete context payload.
pub(crate) trait FragmentRegistration: Sync {
    fn matches_text(&self, text: &str) -> bool;
}

pub(crate) struct FragmentRegistrationProxy<T> {
    _marker: PhantomData<fn() -> T>,
}

impl<T> FragmentRegistrationProxy<T> {
    pub(crate) const fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<T: ContextualUserFragment> FragmentRegistration for FragmentRegistrationProxy<T> {
    fn matches_text(&self, text: &str) -> bool {
        T::matches_text(text)
    }
}

/// Context payload that is injected as a user-authored message fragment.
///
/// Implementations own the response role, start/end markers used to recognize
/// the fragment, and provide the fragment body appended directly after the
/// start marker. The default helpers wrap that body and convert it into the
/// response item shape expected by model input assembly.
pub(crate) trait ContextualUserFragment {
    const ROLE: &'static str;
    const START_MARKER: &'static str;
    const END_MARKER: &'static str;

    fn body(&self) -> String;

    fn matches_text(text: &str) -> bool
    where
        Self: Sized,
    {
        let trimmed = text.trim_start();
        let starts_with_marker = trimmed
            .get(..Self::START_MARKER.len())
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(Self::START_MARKER));
        let trimmed = trimmed.trim_end();
        let ends_with_marker = trimmed
            .get(trimmed.len().saturating_sub(Self::END_MARKER.len())..)
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(Self::END_MARKER));
        starts_with_marker && ends_with_marker
    }

    fn render(&self) -> String {
        format!(
            "{}{}\n{}",
            Self::START_MARKER,
            self.body(),
            Self::END_MARKER
        )
    }

    fn into(self) -> ResponseItem
    where
        Self: Sized,
    {
        ResponseItem::Message {
            id: None,
            role: Self::ROLE.to_string(),
            content: vec![ContentItem::InputText {
                text: self.render(),
            }],
            end_turn: None,
            phase: None,
        }
    }
}
