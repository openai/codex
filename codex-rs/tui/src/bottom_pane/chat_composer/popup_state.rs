//! Popup lifecycle state for the chat composer.
//! Tracks the single active popup plus dismissal/query state used to synchronize it.

use crate::bottom_pane::command_popup::CommandPopup;
use crate::bottom_pane::file_search_popup::FileSearchPopup;
use crate::bottom_pane::mentions_v2::MentionV2Popup;
use crate::bottom_pane::skill_popup::SkillPopup;

#[derive(Default)]
pub(super) struct PopupState {
    pub(super) active: ActivePopup,
    pub(super) dismissed_file_token: Option<String>,
    pub(super) current_file_query: Option<String>,
    pub(super) dismissed_mention_token: Option<String>,
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
