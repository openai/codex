//! Popup lifecycle state for the chat composer.
//! Tracks the single active popup plus dismissal/query state used to synchronize it.

use crate::bottom_pane::command_popup::CommandPopup;
use crate::bottom_pane::file_search_popup::FileSearchPopup;
use crate::bottom_pane::mentions_v2::MentionV2Popup;
use crate::bottom_pane::skill_popup::SkillPopup;
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
    pub(super) fn new(text: &str, range: Range<usize>, query: String) -> Self {
        let token = text[range.clone()].to_string();
        let occurrence = complete_token_occurrences_before(text, &token, range.start);
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
    pub(super) fn matches(&self, text: &str, range: &Range<usize>, query: &str) -> bool {
        if self.query != query || text.get(range.clone()) != Some(self.token.as_str()) {
            return false;
        }
        complete_token_occurrences_before(text, &self.token, range.start) == self.occurrence
    }
}

fn complete_token_occurrences_before(text: &str, token: &str, before: usize) -> usize {
    text[..before]
        .split_whitespace()
        .filter(|candidate| *candidate == token)
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
