//! Validation helpers for `/goal` objective text.

use super::*;
use crate::bottom_pane::ChatComposer;
use codex_protocol::num_format::format_with_separators;
use codex_protocol::protocol::MAX_THREAD_GOAL_OBJECTIVE_CHARS;

const GOAL_TOO_LONG_FILE_HINT: &str = "Put longer instructions in a file and refer to that file in the goal, for example: /goal follow the instructions in docs/goal.md.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GoalObjectiveValidationSource {
    Live,
    Queued,
}

impl ChatWidget {
    pub(super) fn goal_objective_with_pending_pastes_is_allowed(
        &mut self,
        args: &str,
        text_elements: &[TextElement],
    ) -> bool {
        let pending_pastes = self.bottom_pane.composer_pending_pastes();
        let objective_chars = if pending_pastes.is_empty() {
            args.trim().chars().count()
        } else {
            let (expanded, _) =
                ChatComposer::expand_pending_pastes(args, text_elements.to_vec(), &pending_pastes);
            expanded.trim().chars().count()
        };
        self.goal_objective_char_count_is_allowed(
            objective_chars,
            GoalObjectiveValidationSource::Live,
        )
    }

    pub(super) fn goal_objective_is_allowed(
        &mut self,
        objective: &str,
        source: GoalObjectiveValidationSource,
    ) -> bool {
        self.goal_objective_char_count_is_allowed(objective.chars().count(), source)
    }

    fn goal_objective_char_count_is_allowed(
        &mut self,
        actual_chars: usize,
        source: GoalObjectiveValidationSource,
    ) -> bool {
        if actual_chars <= MAX_THREAD_GOAL_OBJECTIVE_CHARS {
            return true;
        }
        let actual_chars = format_with_separators(actual_chars as i64);
        let max_chars = format_with_separators(MAX_THREAD_GOAL_OBJECTIVE_CHARS as i64);
        self.add_error_message(format!(
            "Goal objective is too long: {actual_chars} characters. Limit: {max_chars} characters. {GOAL_TOO_LONG_FILE_HINT}"
        ));
        if source == GoalObjectiveValidationSource::Live {
            self.bottom_pane
                .set_composer_text(String::new(), Vec::new(), Vec::new());
            self.bottom_pane.drain_pending_submission_state();
        }
        false
    }
}
