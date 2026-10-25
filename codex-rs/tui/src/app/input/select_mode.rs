//! Transcript selection chord routing kept outside the central application input path.

use super::super::*;

impl App {
    pub(crate) fn transcript_select_exit_key(event: &TuiEvent) -> Option<KeyBinding> {
        let TuiEvent::Key(key) = event else {
            return None;
        };
        let (code, modifiers) = crate::key_hint::normalize_key_parts(key.code, key.modifiers);
        (key.kind == KeyEventKind::Press
            && modifiers == KeyModifiers::NONE
            && matches!(code, KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q')))
        .then(|| KeyBinding::from_event(*key))
    }

    pub(super) fn select_mode_owns_chord(&self, tui: &tui::Tui, key_event: KeyEvent) -> bool {
        let select_mode = matches!(
            &self.overlay,
            Some(Overlay::Transcript(overlay)) if overlay.is_select_mode()
        ) || (tui.is_owned_screen()
            && self.overlay.is_none()
            && self.transcript_view.is_select_mode());
        if !select_mode {
            return false;
        }
        let contexts = self.active_keymap_contexts();
        let search_editing = matches!(
            &self.overlay,
            Some(Overlay::Transcript(overlay)) if overlay.is_search_editing()
        ) || (self.overlay.is_none()
            && self.transcript_view.is_search_editing());
        let (code, modifiers) =
            crate::key_hint::normalize_key_parts(key_event.code, key_event.modifiers);
        let fixed_search_control = search_editing
            && (code == KeyCode::Esc
                || (modifiers == KeyModifiers::NONE
                    && matches!(code, KeyCode::Enter | KeyCode::PageUp | KeyCode::PageDown))
                || (modifiers == KeyModifiers::CONTROL
                    && matches!(code, KeyCode::Char('c' | 'n' | 'p'))));
        let fixed_select_control = !search_editing
            && key_event.kind != KeyEventKind::Release
            && (crate::text_selection::is_copy_key(key_event)
                || matches!(
                    (code, modifiers),
                    (
                        KeyCode::Enter | KeyCode::Esc | KeyCode::Char('q' | '/' | 'o' | 'y'),
                        KeyModifiers::NONE
                    ) | (
                        KeyCode::Char(' '),
                        KeyModifiers::NONE | KeyModifiers::CONTROL
                    ) | (
                        KeyCode::Char('v' | 'n' | 'g' | '$'),
                        KeyModifiers::NONE | KeyModifiers::SHIFT
                    ) | (
                        KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down,
                        KeyModifiers::NONE | KeyModifiers::SHIFT
                    ) | (
                        KeyCode::Char('h' | 'l' | 'k' | 'j' | 'w' | 'b' | 'e' | '0'),
                        KeyModifiers::NONE
                    ) | (
                        KeyCode::Home | KeyCode::End,
                        KeyModifiers::NONE | KeyModifiers::CONTROL
                    ) | (KeyCode::PageUp | KeyCode::PageDown, KeyModifiers::NONE)
                        | (KeyCode::Char('b' | 'f' | 'u' | 'd'), KeyModifiers::CONTROL)
                ));
        let pager_motion = !search_editing
            && [
                &self.keymap.pager.scroll_up,
                &self.keymap.pager.scroll_down,
                &self.keymap.pager.page_up,
                &self.keymap.pager.page_down,
                &self.keymap.pager.half_page_up,
                &self.keymap.pager.half_page_down,
                &self.keymap.pager.jump_top,
                &self.keymap.pager.jump_bottom,
            ]
            .into_iter()
            .any(|bindings| bindings.is_pressed(key_event));
        let active_action = |action: crate::keymap::KeymapActionId| {
            contexts.contains_action(action)
                && if search_editing {
                    action.context == crate::keymap::KeymapContext::Editor
                        || matches!(
                            (action.context, action.action),
                            (crate::keymap::KeymapContext::Chat, "toggle_voice")
                        )
                } else {
                    matches!(
                        (action.context, action.action),
                        (crate::keymap::KeymapContext::Global, "find_transcript")
                            | (crate::keymap::KeymapContext::Chat, "toggle_voice")
                            | (
                                crate::keymap::KeymapContext::Pager,
                                "find"
                                    | "scroll_up"
                                    | "scroll_down"
                                    | "page_up"
                                    | "page_down"
                                    | "half_page_up"
                                    | "half_page_down"
                                    | "jump_top"
                                    | "jump_bottom"
                            )
                    ) || (self.overlay.is_some()
                        && action.context == crate::keymap::KeymapContext::Pager
                        && matches!(action.action, "close" | "close_transcript"))
                }
        };
        let completed_action = |dispatch| {
            self.keymap.chords.bindings.iter().any(|binding| {
                active_action(binding.action)
                    && crate::keymap::dispatch_event(binding.action) == Some(dispatch)
            })
        };
        if fixed_search_control || fixed_select_control || pager_motion {
            false
        } else if self.key_chord_matcher.is_pending() {
            match self
                .key_chord_matcher
                .clone()
                .advance(key_event, &self.keymap.chords, contexts)
            {
                crate::keymap::KeyChordMatch::Completed(dispatch) => completed_action(dispatch),
                crate::keymap::KeyChordMatch::Pending(prefix) => {
                    let (code, modifiers) = prefix.parts();
                    self.keymap.chords.bindings.iter().any(|binding| {
                        active_action(binding.action)
                            && crate::keymap::chord_stroke_matches(
                                binding.chord.prefix,
                                KeyEvent::new(code, modifiers),
                            )
                    })
                }
                crate::keymap::KeyChordMatch::Ignored => true,
                crate::keymap::KeyChordMatch::PassThrough
                | crate::keymap::KeyChordMatch::Cancelled => false,
            }
        } else {
            self.keymap.chords.bindings.iter().any(|binding| {
                active_action(binding.action)
                    && crate::keymap::chord_stroke_matches(binding.chord.prefix, key_event)
            })
        }
    }
}
