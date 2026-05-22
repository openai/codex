//! Projects stored next-prompt suggestions into the composer placeholder.
//!
//! `ChatWidget` owns only presentation state: the latest model text is
//! stored separately from the draft and becomes placeholder text only while the
//! composer is empty and the rest of the chat surface is idle. The app layer owns
//! generation, cancellation, and thread identity checks.

use super::*;

impl ChatWidget {
    #[cfg_attr(not(test), allow(dead_code))]
    /// Replaces the stored suggestion and refreshes placeholder visibility.
    pub(crate) fn set_next_prompt_suggestion(&mut self, suggestion: Option<String>) {
        self.next_prompt_suggestion = suggestion;
        self.refresh_composer_placeholder();
    }

    /// Removes the stored suggestion and reports whether anything changed.
    pub(crate) fn clear_next_prompt_suggestion(&mut self) -> bool {
        if self.next_prompt_suggestion.take().is_none() {
            return false;
        }
        self.refresh_composer_placeholder();
        true
    }

    /// Removes and returns the stored suggestion for composer acceptance.
    pub(crate) fn take_next_prompt_suggestion(&mut self) -> Option<String> {
        let suggestion = self.next_prompt_suggestion.take()?;
        self.refresh_composer_placeholder();
        Some(suggestion)
    }

    #[cfg(test)]
    pub(crate) fn next_prompt_suggestion(&self) -> Option<&str> {
        self.next_prompt_suggestion.as_deref()
    }

    /// Reports whether Tab acceptance has concrete suggestion text to move.
    pub(crate) fn has_next_prompt_suggestion(&self) -> bool {
        self.next_prompt_suggestion.is_some()
    }

    /// Reports whether ghost text is allowed in the current composer surface.
    ///
    /// Suggestions are hidden while the user has a draft, a modal or popup owns
    /// focus, a side conversation or plan mode changes composer semantics, a recent
    /// error/rate-limit needs attention, or Codex is still responding.
    pub(crate) fn can_show_next_prompt_suggestion(&self) -> bool {
        self.bottom_pane.composer_is_empty()
            && self.no_modal_or_popup_active()
            && !self.active_side_conversation
            && self.active_mode_kind() != ModeKind::Plan
            && self.last_non_retry_error.is_none()
            && self.codex_rate_limit_reached_type.is_none()
            && !self.bottom_pane.is_task_running()
    }

    /// Recomputes the visible placeholder from side, suggestion, and fallback state.
    pub(crate) fn refresh_composer_placeholder(&mut self) {
        let placeholder = if self.active_side_conversation {
            self.side_placeholder_text.clone()
        } else if self.can_show_next_prompt_suggestion()
            && let Some(suggestion) = self.next_prompt_suggestion.as_ref()
        {
            suggestion.clone()
        } else {
            self.normal_placeholder_text.clone()
        };
        self.bottom_pane.set_placeholder_text(placeholder);
    }
}
