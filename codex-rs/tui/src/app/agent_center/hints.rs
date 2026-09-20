//! A compact footer advertises primary actions; help lists the configured task shortcuts.

use super::*;
use crate::key_hint;
use crate::style::footer_hint_label_style;

pub(super) fn hint_line(items: &[(String, String)]) -> Line<'static> {
    let mut line = Line::default();
    for (key, label) in items.iter().filter(|(key, _)| !key.is_empty()) {
        if !line.spans.is_empty() {
            line.spans.push("  ".dim());
        }
        line.spans.extend(key_hint::key_label_spans(key));
        line.spans.push(Span::styled(
            format!(" {label}"),
            footer_hint_label_style().not_bold(),
        ));
    }
    line
}

impl AgentsOverviewView {
    fn center_list_hint(&self, action: ListAction) -> Option<ShortcutHint> {
        self.keymap.primary_hint(action).filter(|hint| {
            !matches!(hint, ShortcutHint::Single(binding) if [
                &self.agents_keymap.resume,
                &self.agents_keymap.search,
                &self.agents_keymap.new_task,
                &self.agents_keymap.new_worktree,
                &self.agents_keymap.rename,
                &self.agents_keymap.stop,
                &self.agents_keymap.archive,
                &self.agents_keymap.delete,
                &self.agents_keymap.hide,
                &self.agents_keymap.toggle_grouping,
            ].into_iter().any(|bindings| bindings.contains(binding)))
        })
    }

    pub(super) fn center_filter_hint(&self) -> String {
        [key_hint::plain(KeyCode::Tab), key_hint::shift(KeyCode::Tab)]
            .into_iter()
            .filter(|hint| {
                let (code, modifiers) = hint.parts();
                !self
                    .center_shortcut_keys
                    .is_pressed(KeyEvent::new(code, modifiers))
            })
            .map(|hint| hint.display_label())
            .collect::<Vec<_>>()
            .join("/")
    }

    pub(super) fn center_task_hints(&self) -> Vec<(String, String)> {
        let mut hints = vec![(self.center_filter_hint(), "filter".into())];
        for (action, bindings, label) in [
            ("new_task", &self.agents_keymap.new_task, "new"),
            (
                "new_worktree",
                &self.agents_keymap.new_worktree,
                "new worktree",
            ),
            ("resume", &self.agents_keymap.resume, "resume"),
            ("search", &self.agents_keymap.search, "search"),
            (
                "toggle_grouping",
                &self.agents_keymap.toggle_grouping,
                "group",
            ),
            ("rename", &self.agents_keymap.rename, "rename"),
            ("stop", &self.agents_keymap.stop, "stop"),
            ("archive", &self.agents_keymap.archive, "archive"),
            ("hide", &self.agents_keymap.hide, "hide"),
            ("delete", &self.agents_keymap.delete, "delete"),
        ] {
            if (action != "new_worktree" || self.worktrees_enabled)
                && let Some(hint) = self.agents_keymap.primary_hint(action, bindings)
            {
                hints.push((hint.display_label(), label.into()));
            }
        }
        for (action, label) in [
            (ListAction::MoveUp, "up"),
            (ListAction::MoveDown, "down"),
            (ListAction::Accept, "open"),
            (ListAction::PageUp, "page up"),
            (ListAction::PageDown, "page down"),
        ] {
            if let Some(hint) = self.center_list_hint(action) {
                hints.push((hint.display_label(), label.into()));
            }
        }
        hints.push((
            key_hint::ctrl(KeyCode::Char('c')).display_label(),
            "quit".into(),
        ));
        hints
    }

    pub(super) fn center_footer_hints(&self) -> Vec<(String, String)> {
        let state = self.state();
        if let Some(items) = &state.key_chord_hint {
            return items.clone();
        }
        let mut hints = Vec::new();
        if !state.help
            && !state.editing_metadata()
            && ![KeyModifiers::NONE, KeyModifiers::SHIFT]
                .into_iter()
                .any(|modifiers| {
                    self.center_shortcut_keys
                        .is_pressed(KeyEvent::new(KeyCode::Char('?'), modifiers))
                })
        {
            hints.push(("?".into(), "help".into()));
        }
        let cancel = if state.help {
            self.keymap.primary_hint(ListAction::Cancel)
        } else {
            self.center_list_hint(ListAction::Cancel)
        };
        if let Some(hint) = cancel {
            hints.push((hint.display_label(), "back".into()));
        }
        if state.help {
            return hints;
        }
        if let Some(hint) = self.center_list_hint(ListAction::Accept) {
            hints.push((
                hint.display_label(),
                if state.rename_target.is_some() {
                    "rename"
                } else {
                    "open"
                }
                .into(),
            ));
        }
        if !state.editing_metadata() {
            for (action, bindings, label) in [
                ("new_task", &self.agents_keymap.new_task, "new"),
                (
                    "new_worktree",
                    &self.agents_keymap.new_worktree,
                    "new worktree",
                ),
            ] {
                if (action != "new_worktree" || self.worktrees_enabled)
                    && let Some(hint) = self.agents_keymap.primary_hint(action, bindings)
                {
                    hints.push((hint.display_label(), label.into()));
                }
            }
        }
        hints
    }
}
