//! Carry one startup submission intent across draft handoff without consuming editable text early.
//!
//! Confirmation applies to the visible draft only. Editing, cancellation, or disconnect removes
//! the intent; normal submission parsing runs once the session and protected-input gates are ready.

use super::*;
use crate::bottom_pane::ComposerDraftSnapshot;

impl ChatWidget {
    /// Keep recovery current while startup still owns an unsent, handed-off draft.
    pub(super) fn refresh_startup_recovery(&self) {
        crate::startup_recovery::refresh_after_handoff(|| {
            self.bottom_pane.composer_recovery_snapshot()
        });
    }

    /// Restore startup text first, then submit its confirmed contents once the session is ready.
    pub(crate) fn restore_startup_input_when_ready(
        &mut self,
        pending_draft: &mut Option<ComposerDraftSnapshot>,
        pending_submission: &mut bool,
    ) {
        let had_draft = pending_draft.is_some();
        let destination_has_content = !self.bottom_pane.composer_is_empty()
            || !self.bottom_pane.composer_pending_pastes().is_empty();
        self.restore_startup_draft_when_ready(pending_draft);
        if self.blocks_direct_input {
            *pending_submission = false;
            self.cancel_startup_submission();
            return;
        }
        if had_draft
            && pending_draft.is_none()
            && std::mem::take(pending_submission)
            && !destination_has_content
        {
            self.input_queue.startup_submission = Some(self.bottom_pane.composer_draft_snapshot());
            self.bottom_pane.set_footer_hint_override(Some(vec![
                ("Waiting for startup".into(), String::new()),
                ("esc".into(), "cancel".into()),
            ]));
        }
        if pending_draft.is_some()
            || !self.is_session_configured()
            || self
                .effective_collaboration_mode()
                .model()
                .trim()
                .is_empty()
            || !self.bottom_pane.composer_input_enabled()
            || self.startup_submission_has_protected_input()
            || self.is_plan_streaming_in_tui()
            || self.is_user_turn_pending_or_running()
            || self.input_queue.suppress_queue_autosend
            || self.input_queue.rate_limit_recovery_pending
            || self.input_queue.recovered_queue
        {
            return;
        }
        #[cfg(any(target_os = "windows", test))]
        if self.windows_sandbox_host == crate::app::WindowsSandboxHost::Local
            && self.elevated_windows_sandbox_setup_required()
        {
            return;
        }
        let Some(confirmed) = self.input_queue.startup_submission.take() else {
            crate::startup_recovery::clear();
            return;
        };
        self.bottom_pane.set_footer_hint_override(/*items*/ None);
        let current = self.bottom_pane.composer_draft_snapshot();
        if current.text != confirmed.text
            || current.text_elements != confirmed.text_elements
            || current.local_images != confirmed.local_images
            || current.remote_image_urls != confirmed.remote_image_urls
            || current.mention_bindings != confirmed.mention_bindings
            || current.pending_pastes != confirmed.pending_pastes
        {
            return;
        }
        let result = self.bottom_pane.prepare_startup_submission();
        crate::startup_recovery::submitted(&result);
        self.handle_composer_input_result(result, /*had_modal_or_popup*/ false);
    }

    /// Cancel only the intent; the editor keeps the complete draft for editing or recovery.
    pub(crate) fn cancel_startup_submission(&mut self) {
        if self.input_queue.startup_submission.take().is_some() {
            self.bottom_pane.set_footer_hint_override(/*items*/ None);
            self.refresh_startup_recovery();
        }
    }

    pub(crate) fn handle_startup_submission_key(&mut self, key: KeyEvent) -> bool {
        if self.input_queue.startup_submission.is_none()
            || self.startup_submission_has_protected_input()
            || !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
        {
            return false;
        }
        if self.bottom_pane.is_startup_submit_key(key) {
            return true;
        }
        if key.code == KeyCode::Esc {
            self.cancel_startup_submission();
            return true;
        }
        if !self.bottom_pane.is_startup_cursor_key(key) {
            self.cancel_startup_submission();
        }
        false
    }

    pub(super) fn startup_submission_has_protected_input(&self) -> bool {
        self.has_active_view()
            || self
                .bottom_pane
                .questions
                .as_ref()
                .is_some_and(|q| q.expanded)
            || self.has_pending_protected_request()
    }
}
