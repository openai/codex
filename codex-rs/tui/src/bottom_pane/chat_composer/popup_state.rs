//! Popup lifecycle state for the chat composer.
//! Tracks the single active popup plus dismissal/query state used to synchronize it.

use crate::bottom_pane::command_popup::CommandPopup;
use crate::bottom_pane::file_search_popup::FileSearchPopup;
use crate::bottom_pane::mentions_v2::MentionV2Popup;
use crate::bottom_pane::skill_popup::SkillPopup;
use crate::bottom_pane::textarea::TextArea;
use std::ops::Range;

/// One token occurrence whose autocomplete popup should remain hidden.
pub(super) struct DismissedToken {
    /// Popup query text for the token, excluding its leading sigil.
    query: String,
    /// Exact token text, including its sigil, captured when the popup was dismissed.
    token: String,
    /// Zero-based ordinal among identical token strings in the draft at dismissal time.
    occurrence: usize,
}

impl DismissedToken {
    /// Captures the stable identity of the token at `range`.
    pub(super) fn new(textarea: &TextArea, range: Range<usize>, query: String) -> Self {
        let text = textarea.text();
        let token = text[range.clone()].to_string();
        let occurrence = complete_token_occurrences_before(textarea, &token, range.start);
        Self {
            query,
            token,
            occurrence,
        }
    }

    /// Returns whether `range` identifies the same token occurrence in the current draft.
    ///
    /// Byte offsets may shift under offset-only edits, while the token text and its ordinal keep
    /// later identical occurrences distinct.
    pub(super) fn matches(&self, textarea: &TextArea, range: &Range<usize>, query: &str) -> bool {
        let text = textarea.text();
        if self.query != query || text.get(range.clone()) != Some(self.token.as_str()) {
            return false;
        }
        complete_token_occurrences_before(textarea, &self.token, range.start) == self.occurrence
    }
}

/// Counts complete editable-token occurrences before `before` in a single ordered pass.
///
/// Whitespace and atomic element edges delimit occurrences; bare nested sigils do not. This keeps
/// dismissal identity aligned with the completion resolver without rescanning every element for
/// each match.
fn complete_token_occurrences_before(textarea: &TextArea, token: &str, before: usize) -> usize {
    let text = textarea.text();
    let mut elements_ending_before_matches = textarea.text_element_ranges().peekable();
    let mut elements_starting_after_matches = textarea.text_element_ranges().peekable();
    text[..before]
        .match_indices(token)
        .filter(|(start, _)| {
            let end = start + token.len();
            while elements_ending_before_matches
                .peek()
                .is_some_and(|element| element.end < *start)
            {
                elements_ending_before_matches.next();
            }
            while elements_starting_after_matches
                .peek()
                .is_some_and(|element| element.start < end)
            {
                elements_starting_after_matches.next();
            }
            let ends_at_boundary = text[end..].chars().next().is_none_or(char::is_whitespace)
                || elements_starting_after_matches
                    .peek()
                    .is_some_and(|element| element.start == end);
            let starts_at_boundary = *start == 0
                || text[..*start]
                    .chars()
                    .next_back()
                    .is_some_and(char::is_whitespace)
                || elements_ending_before_matches
                    .peek()
                    .is_some_and(|element| element.end == *start);
            starts_at_boundary && ends_at_boundary
        })
        .count()
}

#[derive(Default)]
pub(super) struct PopupState {
    pub(super) active: ActivePopup,
    pub(super) dismissed_file_token: Option<DismissedToken>,
    pub(super) current_file_query: Option<String>,
    pub(super) dismissed_mention_token: Option<DismissedToken>,
}

impl PopupState {
    pub(super) fn active(&self) -> bool {
        !matches!(self.active, ActivePopup::None)
    }
}

/// Popup state - at most one can be visible at any time.
#[derive(Default)]
pub(super) enum ActivePopup {
    #[default]
    None,
    Command(CommandPopup),
    File(FileSearchPopup),
    Skill(SkillPopup),
    MentionV2(MentionV2Popup),
}

#[cfg(test)]
#[path = "popup_state_tests.rs"]
mod tests;
