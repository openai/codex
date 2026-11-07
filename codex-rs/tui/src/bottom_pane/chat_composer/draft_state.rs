//! Editable composer draft state kept separate from composer control flow.

use std::cell::RefCell;
use std::collections::HashMap;

use crate::bottom_pane::MentionBinding;
use crate::bottom_pane::paste_burst::PasteBurst;
use crate::bottom_pane::textarea::TextArea;
use crate::bottom_pane::textarea::TextAreaState;

pub(super) struct DraftState {
    pub(super) textarea: TextArea,
    pub(super) textarea_state: RefCell<TextAreaState>,
    pub(super) is_bash_mode: bool,
    pub(super) pending_pastes: Vec<(String, String)>,
    pub(super) input_enabled: bool,
    pub(super) input_disabled_placeholder: Option<String>,
    pub(super) paste_burst: PasteBurst,
    pub(super) disable_paste_burst: bool,
    /// Byte offset of the space added by the last skill completion, until the next key is resolved.
    pub(super) skill_completion_space: Option<usize>,
    pub(super) mention_bindings: HashMap<u64, ComposerMentionBinding>,
    pub(super) recent_submission_mention_bindings: Vec<MentionBinding>,
}

impl DraftState {
    pub(super) fn new() -> Self {
        Self {
            textarea: TextArea::new(),
            textarea_state: RefCell::new(TextAreaState::default()),
            is_bash_mode: false,
            pending_pastes: Vec::new(),
            input_enabled: true,
            input_disabled_placeholder: None,
            paste_burst: PasteBurst::default(),
            disable_paste_burst: false,
            skill_completion_space: None,
            mention_bindings: HashMap::new(),
            recent_submission_mention_bindings: Vec::new(),
        }
    }

    pub(super) fn skill_completion_space_for_char(&self, ch: char) -> Option<usize> {
        self.skill_completion_space.filter(|&space| {
            ",.;:!?)]}。，、！？》".contains(ch)
                && self.textarea.allows_paste_burst()
                && !self.textarea.is_vim_replace_mode()
                && self.textarea.cursor() == space + 1
                && self.textarea.mouse_selection_range().is_none()
                && self.textarea.text().get(space..space + 1) == Some(" ")
        })
    }
}

#[derive(Clone, Debug)]
pub(super) struct ComposerMentionBinding {
    pub(super) sigil: char,
    pub(super) mention: String,
    pub(super) path: String,
}
