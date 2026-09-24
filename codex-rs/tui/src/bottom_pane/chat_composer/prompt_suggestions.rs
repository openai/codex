//! Keep suggestions outside the draft until Tab accepts them. Typing hides them,
//! Escape cancels them, and stale or empty results cannot capture input.
//! Transcript interactions hide suggestion text and its reserved height without cancelling it.

use super::*;
use crate::prompt_suggestions::PromptSuggestion;
use crate::prompt_suggestions::SuggestionRequest;

impl ChatComposer {
    pub(crate) fn set_prompt_suggestion(&mut self, request: SuggestionRequest) {
        self.prompt_suggestion = Some(PromptSuggestion {
            request,
            text: None,
        });
    }

    pub(crate) fn has_prompt_suggestion(&self) -> bool {
        self.prompt_suggestion
            .as_ref()
            .is_some_and(|suggestion| !suggestion.request.cancellation.is_cancelled())
    }

    pub(crate) fn clear_prompt_suggestion(&mut self) {
        self.prompt_suggestion = None;
    }

    pub(crate) fn apply_prompt_suggestion(
        &mut self,
        request: &SuggestionRequest,
        text: Option<String>,
    ) {
        if let Some(suggestion) = &mut self.prompt_suggestion
            && suggestion.request.id == request.id
            && !request.cancellation.is_cancelled()
        {
            suggestion.text = text;
            if suggestion.text.is_none() {
                self.clear_prompt_suggestion();
            }
        }
    }

    pub(super) fn visible_prompt_suggestion(&self) -> Option<&str> {
        let suggestion = self.prompt_suggestion.as_ref()?;
        (self.has_focus
            && self.sparkle.terminal_focused
            && self.draft.input_enabled
            && !self.blocks_direct_input
            && !self.is_task_running
            && !self.queue_submissions
            && self.is_empty()
            && !self.popup_active()
            && !self.is_in_paste_burst()
            && !suggestion.request.cancellation.is_cancelled())
        .then_some(suggestion.text.as_deref())
        .flatten()
    }

    pub(super) fn prompt_suggestion_lines(
        &self,
        width: u16,
        options: ComposerRenderOptions<'_>,
    ) -> Option<Vec<Line<'static>>> {
        if options.footer.is_some_and(|footer| footer.is_interactive) {
            return None;
        }
        Some(
            textwrap::wrap(self.visible_prompt_suggestion()?, usize::from(width.max(1)))
                .into_iter()
                .map(|line| Line::from(line.into_owned().dim()))
                .collect(),
        )
    }

    pub(super) fn handle_prompt_suggestion_key(&mut self, key: KeyEvent) -> bool {
        if key.modifiers != KeyModifiers::NONE {
            return false;
        }
        if key.code == KeyCode::Esc && self.has_prompt_suggestion() {
            self.clear_prompt_suggestion();
            return true;
        }
        if key.code == KeyCode::Tab
            && ![
                KeymapContext::Editor,
                KeymapContext::VimNormal,
                KeymapContext::Composer,
            ]
            .into_iter()
            .any(|context| {
                self.suggestion_tab_reserved.contains(context)
                    && self.keymap_contexts().contains(context)
            })
            && let Some(text) = self.visible_prompt_suggestion().map(str::to_owned)
        {
            self.clear_prompt_suggestion();
            self.insert_str(&text);
            self.suggestion_tab_accepted = true;
            return true;
        }
        false
    }
}

#[cfg(test)]
#[path = "prompt_suggestions_tests.rs"]
mod tests;
