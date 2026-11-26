//! Shared rename editing and task shortcuts respect configured editor and list bindings.

use super::*;
use crossterm::event::KeyEventKind;

impl AgentsOverviewView {
    pub(in crate::app::agents_overview_view) fn rename_key(&mut self, key: KeyEvent) -> bool {
        let mut state = self.state();
        if state.rename_target.is_none() {
            return false;
        }
        let input = &mut state.input;
        let editing = input.should_handle_vim_insert_escape(key) || input.is_vim_operator_pending();
        if !editing {
            match self.keymap.action_for(key) {
                Some(ListAction::Cancel) => {
                    drop(state);
                    self.on_ctrl_c();
                    return true;
                }
                Some(ListAction::Accept) if !is_plain_text_key_event(key) => {
                    let online = state.connection_notice.is_none() && !state.creating_worktree;
                    drop(state);
                    if online {
                        self.activate();
                    }
                    return true;
                }
                _ => {}
            }
        }
        if editing || self.rename_editor_owns_key(input, key) {
            input.input(key);
            return !crate::keymap::is_dispatch_token_event(key);
        }
        false
    }

    fn rename_editor_owns_key(&self, input: &TextArea, key: KeyEvent) -> bool {
        let editor_binding = crate::keymap::keymap_action_ids()
            .filter(|action| action.context == input.keymap_context())
            .any(|action| {
                crate::keymap::bindings_for_action(
                    &self.editor_keymap,
                    action.context.config_name(),
                    action.action,
                )
                .is_some_and(|bindings| bindings.is_pressed(key))
            });
        is_plain_text_key_event(key) || editor_binding || !self.center_shortcut_keys.is_pressed(key)
    }

    // Rename handles list accept/cancel before editor bindings. Search consumes
    // printable keys before list accept or agent actions.
    pub(super) fn editing_allows_hint(
        &self,
        state: &AgentsOverviewViewState,
        hint: ShortcutHint,
    ) -> bool {
        if !state.editing_metadata() {
            return true;
        }
        let renaming = state.rename_target.is_some();
        if renaming && state.input.is_vim_operator_pending() {
            return false;
        }
        let ShortcutHint::Single(binding) = hint else {
            return true;
        };
        let (code, modifiers) = binding.parts();
        let key = KeyEvent::new(code, modifiers);
        if renaming && state.input.should_handle_vim_insert_escape(key) {
            return false;
        }
        match self.keymap.action_for(key) {
            Some(ListAction::Cancel) => true,
            Some(ListAction::Accept) => !is_plain_text_key_event(key),
            _ if renaming => !self.rename_editor_owns_key(&state.input, key),
            _ => {
                !is_plain_text_key_event(key)
                    && !(code == KeyCode::Backspace && modifiers.is_empty())
            }
        }
    }

    pub(super) fn search_sort_key(&self, key: KeyEvent) -> bool {
        key.code == KeyCode::Tab
            && key.modifiers.is_empty()
            && self.keymap.action_for(key).is_none()
            && !self.center_shortcut_keys.is_pressed(key)
    }

    pub(in crate::app::agents_overview_view) fn command_center_key(
        &mut self,
        key: KeyEvent,
    ) -> bool {
        if self.state().editing_metadata() {
            if self.search_sort_key(key) && key.kind == KeyEventKind::Press {
                let mut state = self.state();
                if state.searching
                    && state.rename_target.is_none()
                    && state.connection_notice.is_none()
                {
                    state.search_sort = match state.search_sort {
                        HistorySort::Updated => HistorySort::Created,
                        HistorySort::Created => HistorySort::Updated,
                    };
                    if let Some(search) = &mut state.history {
                        *search = HistorySearch {
                            id: search.id,
                            scan_id: uuid::Uuid::new_v4(),
                            retained: self.rows.iter().map(|row| row.thread_id).collect(),
                            ..Default::default()
                        };
                    }
                    state.scroll = 0;
                    drop(state);
                    self.load_search_matches(/*additional*/ 10);
                    return true;
                }
            }
            if self.keymap.action_for(key) == Some(ListAction::Cancel) {
                self.on_ctrl_c();
                return true;
            }
            return false;
        }
        let mut state = self.state();
        if state.help && self.keymap.action_for(key) == Some(ListAction::Cancel) {
            state.help = false;
            return true;
        }
        if state.help {
            if key.code == KeyCode::Char('?')
                && key.kind == KeyEventKind::Press
                && !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
            {
                state.help = false;
            }
            return true;
        }
        if self.center_shortcut_keys.is_pressed(key) {
            return false;
        }
        if key.code == KeyCode::Char('?')
            && key.kind == KeyEventKind::Press
            && !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            state.help = !state.help;
            return true;
        }
        if key.code == KeyCode::Tab
            && !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            let step = if key.modifiers.contains(KeyModifiers::SHIFT) {
                TASK_FILTERS.len() - 1
            } else {
                1
            };
            state.status_filter = (state.status_filter + step) % TASK_FILTERS.len();
            state.scroll = 0;
            drop(state);
            self.reconcile_command_center_selection();
            return true;
        }
        false
    }
}
