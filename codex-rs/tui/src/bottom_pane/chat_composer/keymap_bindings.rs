//! Apply key bindings together so editing, submission, and suggestion acceptance agree.

use super::*;

impl ChatComposer {
    /// Replace composer, editor, and footer-hint key bindings from one runtime snapshot.
    ///
    /// Submit and queue bindings are cached here because composer dispatch must
    /// check them before generic textarea editing. The embedded textarea receives
    /// the same snapshot's editor bindings so a live remap cannot leave submit
    /// keys updated while cursor/editing keys still use old defaults.
    pub(crate) fn set_keymap_bindings(&mut self, keymap: &RuntimeKeymap) {
        self.suggestion_tab_reserved = crate::keymap::keymap_action_ids()
            .filter(|action| {
                matches!(
                    action.context,
                    KeymapContext::Editor | KeymapContext::VimNormal | KeymapContext::Composer
                ) && !(action.context == KeymapContext::Composer && action.action == "queue")
                    && crate::keymap::bindings_for_action(
                        keymap,
                        action.context.config_name(),
                        action.action,
                    )
                    .is_some_and(|bindings| bindings.is_pressed(KeyCode::Tab.into()))
            })
            .fold(KeymapContextSet::default(), |contexts, action| {
                contexts.with(action.context)
            });
        self.submit_keys = keymap.composer.submit.clone();
        self.queue_keys = keymap.composer.queue.clone();
        self.toggle_shortcuts_keys = keymap.composer.toggle_shortcuts.clone();
        self.history_search_previous_keys = keymap.composer.history_search_previous.clone();
        self.history_search_next_keys = keymap.composer.history_search_next.clone();
        self.editor_keymap = keymap.editor.clone();
        self.vim_normal_keymap = keymap.vim_normal.clone();
        self.draft.textarea.set_keymap_bindings(keymap);
        self.footer.external_editor_key =
            keymap.primary_hint(KeymapContext::Global, "open_external_editor");
        self.footer.show_warnings_key = keymap.primary_hint(KeymapContext::Global, "open_warnings");
        self.footer.show_transcript_key =
            keymap.primary_hint(KeymapContext::Global, "open_transcript");
        self.footer.find_transcript_key =
            keymap.primary_hint(KeymapContext::Global, "find_transcript");
        self.footer.focus_activity_key =
            keymap.primary_hint(KeymapContext::Global, "focus_activity");
        self.footer.insert_newline_key =
            match keymap.primary_hint(KeymapContext::Editor, "insert_newline") {
                hint @ Some(ShortcutHint::Chord { .. }) => hint,
                _ => footer_insert_newline_key(
                    &keymap.editor.insert_newline,
                    self.footer.use_shift_enter_hint,
                )
                .map(ShortcutHint::from),
            };
        self.footer.queue_key = keymap.primary_hint(KeymapContext::Composer, "queue");
        self.footer.toggle_shortcuts_key =
            keymap.primary_hint(KeymapContext::Composer, "toggle_shortcuts");
        self.footer.history_search_key =
            keymap.primary_hint(KeymapContext::Composer, "history_search_previous");
        self.footer.reasoning_down_key =
            keymap.primary_hint(KeymapContext::Chat, "decrease_reasoning_effort");
        self.footer.reasoning_up_key =
            keymap.primary_hint(KeymapContext::Chat, "increase_reasoning_effort");
        self.footer.toggle_voice_key = keymap.primary_hint(KeymapContext::Chat, "toggle_voice");
    }
}
