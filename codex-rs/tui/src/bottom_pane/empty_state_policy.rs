//! Only the ordinary composer opts into fresh-thread decoration; other views stay undecorated.

use super::*;
use crate::empty_state_animation::ComposerState;

impl BottomPane {
    pub(crate) fn empty_state_composer(&self) -> Option<ComposerState> {
        match (
            self.view_stack.as_slice(),
            self.warnings_view.as_ref(),
            self.questions
                .as_ref()
                .filter(|questions| questions.expanded),
            self.inline_banner_accepts_dismissal(),
        ) {
            ([], None, None, false) => self.composer.empty_state_composer(),
            _ => None,
        }
    }
}
