//! Own question-editor creation, restoration, and the collapsed entry point.

use super::*;

impl BottomPane {
    pub(crate) fn push_async_questions(
        &mut self,
        message_id: &str,
        questions: &[codex_protocol::items::AsyncUserInputQuestion],
    ) {
        self.question_editor().append(message_id, questions);
        self.schedule_active_view_frame();
        self.request_redraw();
    }

    fn question_editor(&mut self) -> &mut AsyncQuestions {
        self.questions.get_or_insert_with(|| {
            let mut questions = AsyncQuestions::new(
                self.app_event_tx.clone(),
                self.has_input_focus,
                self.enhanced_keys_supported,
                self.disable_paste_burst,
                self.keymap.clone(),
            );
            questions.set_vim_enabled(self.composer.is_vim_enabled());
            questions.next_hint = self.pending_input_preview.edit_binding;
            Box::new(questions)
        })
    }

    pub(super) fn question_summary(&self) -> Option<Line<'static>> {
        let questions = self
            .questions
            .as_ref()
            .filter(|q| !q.expanded && q.unanswered_count() > 0)?;
        let count = questions.unanswered_count();
        let hint = self
            .pending_input_preview
            .edit_binding
            .map(|key| format!(" · {} to answer", key.display_label()))
            .unwrap_or_default();
        Some(Line::from(vec![
            "  ? ".dim(),
            Span::styled(
                format!("{count} question{}", if count == 1 { "" } else { "s" }),
                crate::style::accent_style(),
            )
            .bold(),
            hint.dim(),
        ]))
    }
}
