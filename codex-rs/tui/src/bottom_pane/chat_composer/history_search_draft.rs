//! Read and update the saved composer draft while history search owns the visible preview.
//! Background edits are restored on cancellation and discarded when a history match is accepted.
//! The unchanged fallback preview is copied only before the first background edit.
//! Temporary restores preserve command eligibility; the actual background edit updates it.

use super::super::AttachmentState;
use super::super::ChatComposer;
use super::super::ComposerDraftSnapshot;
use super::super::TextArea;
use super::VimPersistentState;

impl ChatComposer {
    pub(crate) fn draft_snapshot(&self) -> ComposerDraftSnapshot {
        let draft = self.history_search.as_ref().map_or_else(
            || self.snapshot_draft(),
            |search| search.original_draft.clone(),
        );
        let mut attachments = AttachmentState::default();
        let mut textarea = TextArea::new();
        attachments.set_remote_image_urls(draft.remote_image_urls.clone(), &mut textarea);
        attachments.reset_local_images(draft.local_image_paths, &mut textarea);
        ComposerDraftSnapshot {
            text: draft.text,
            cursor: draft.cursor,
            text_elements: draft.text_elements,
            local_images: attachments.local_images(),
            remote_image_urls: draft.remote_image_urls,
            mention_bindings: draft.mention_bindings,
            pending_pastes: draft.pending_pastes,
            startup_local_history: self.history.startup_local_history().to_vec(),
            last_composer_activity_at: None,
            sparkle_draft: self.sparkle.draft.get(),
        }
    }

    /// Apply incoming draft changes behind an active search, preserving its query and preview.
    pub(in crate::bottom_pane) fn edit_stored_draft(&mut self, edit: impl FnOnce(&mut Self)) {
        let Some(mut search) = self.history_search.take() else {
            edit(self);
            return;
        };
        search
            .preview_draft
            .get_or_insert_with(|| search.original_draft.clone());
        let preview = self.snapshot_draft();
        let preview_vim_history = std::mem::take(&mut self.vim_history);
        let mut preview_vim_state = VimPersistentState::default();
        self.draft
            .textarea
            .swap_vim_persistent_state(&mut preview_vim_state);
        self.with_sparkle_history_preview(|composer| composer.restore_draft(search.original_draft));
        self.draft
            .textarea
            .swap_vim_persistent_state(&mut search.original_vim_state);
        self.vim_history = search.original_vim_history;
        edit(self);
        search.original_draft = self.snapshot_draft();
        search.original_vim_history = std::mem::take(&mut self.vim_history);
        self.draft
            .textarea
            .swap_vim_persistent_state(&mut search.original_vim_state);
        self.history_search = Some(search);
        self.with_sparkle_history_preview(|composer| composer.restore_draft(preview));
        self.draft
            .textarea
            .swap_vim_persistent_state(&mut preview_vim_state);
        self.vim_history = preview_vim_history;
    }
}
