//! Layout rendering and cursor placement for the agent dashboard.
//! Only search and rename reserve an editor row; list actions share a wrapped footer.

use super::*;

impl AgentsOverviewView {
    pub(super) fn footer_lines(&self, width: u16) -> Vec<Line<'static>> {
        let state = self.state();
        let message = if let Some(items) = &state.key_chord_hint {
            Some(
                items
                    .iter()
                    .map(|(key, label)| format!("{key} {label}"))
                    .collect::<Vec<_>>()
                    .join("  "),
            )
        } else if state.editing_metadata() && state.connection_notice.is_some() {
            Some("esc cancel · actions paused until reconnected".into())
        } else if state.editing_metadata() {
            Some(format!(
                "{} {}  esc cancel",
                self.keymap
                    .primary_hint(ListAction::Accept)
                    .map(crate::key_hint::ShortcutHint::display_label)
                    .unwrap_or_default(),
                if state.rename_target.is_some() {
                    "rename"
                } else {
                    "open"
                }
            ))
        } else if state.connection_notice.is_some() {
            Some("ctrl+c quit · actions paused until the list is refreshed".into())
        } else if state.creating_worktree {
            Some("Creating worktree…  ctrl+c quit".into())
        } else {
            None
        };
        drop(state);
        if let Some(message) = message {
            return textwrap::wrap(&message, usize::from(width.max(1)))
                .into_iter()
                .map(|line| line.into_owned().dim().into())
                .collect();
        }
        if width < 24 {
            let hints = [
                ("new_task", &self.agents_keymap.new_task, "new"),
                (
                    "new_worktree",
                    &self.agents_keymap.new_worktree,
                    "new worktree",
                ),
                ("search", &self.agents_keymap.search, "search"),
            ]
            .into_iter()
            .filter(|(action, _, _)| *action != "new_worktree" || self.worktrees_enabled)
            .filter_map(|(action, bindings, label)| {
                self.agents_keymap
                    .primary_hint(action, bindings)
                    .map(|hint| format!("{} {label}", hint.display_label()))
            });
            return hints
                .chain(["ctrl+c quit".into()])
                .flat_map(|hint| {
                    textwrap::wrap(&hint, usize::from(width.max(1)))
                        .into_iter()
                        .map(|line| line.into_owned().dim().into())
                        .collect::<Vec<_>>()
                })
                .collect();
        }
        let list_hint = |action| {
            self.keymap.primary_hint(action).filter(|hint| {
                !matches!(hint, ShortcutHint::Single(binding)
                if [
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
                ]
                .into_iter()
                .any(|bindings| bindings.contains(binding)))
            })
        };
        let navigation_hints = [ListAction::MoveUp, ListAction::MoveDown]
            .into_iter()
            .filter_map(list_hint)
            .map(|hint| hint.display_label().replace(" + ", "+"))
            .collect::<Vec<_>>();
        let navigation_hint = navigation_hints.join(
            if navigation_hints
                .iter()
                .all(|hint| hint.chars().count() == 1)
            {
                ""
            } else {
                " "
            },
        );
        let mut hints: Vec<Line<'static>> = Vec::new();
        if !navigation_hint.is_empty() {
            hints.push(vec![navigation_hint.bold(), " navigate".dim()].into());
        }
        let mut add_hint = |hint: Option<ShortcutHint>, label: &'static str, enabled: bool| {
            if let Some(hint) = hint {
                let key = hint.display_label().replace(" + ", "+");
                hints.push(
                    vec![
                        if enabled {
                            key.bold()
                        } else {
                            key.bold().dim()
                        },
                        format!(" {label}").dim(),
                    ]
                    .into(),
                );
            }
        };
        add_hint(
            self.agents_keymap
                .primary_hint("resume", &self.agents_keymap.resume),
            "resume",
            true,
        );
        let open_hint = list_hint(ListAction::Accept);
        add_hint(open_hint, "open", true);
        add_hint(
            self.agents_keymap
                .primary_hint("new_task", &self.agents_keymap.new_task),
            "new",
            true,
        );
        add_hint(
            self.agents_keymap
                .primary_hint("new_worktree", &self.agents_keymap.new_worktree),
            "new worktree",
            self.worktrees_enabled,
        );
        add_hint(
            self.agents_keymap
                .primary_hint("search", &self.agents_keymap.search),
            "search",
            true,
        );
        add_hint(
            self.agents_keymap
                .primary_hint("toggle_grouping", &self.agents_keymap.toggle_grouping),
            match self.state().grouping {
                AgentsOverviewGrouping::Project => "group: project",
                AgentsOverviewGrouping::Status => "group: status",
                AgentsOverviewGrouping::Model => "group: model",
            },
            true,
        );
        add_hint(
            self.agents_keymap
                .primary_hint("rename", &self.agents_keymap.rename),
            "rename",
            true,
        );
        add_hint(
            self.agents_keymap
                .primary_hint("stop", &self.agents_keymap.stop),
            "stop",
            self.selected_row()
                .is_some_and(|row| matches!(row.thread.status, ThreadStatus::Active { .. })),
        );
        for (action, bindings) in [
            ("hide", &self.agents_keymap.hide),
            ("archive", &self.agents_keymap.archive),
            ("delete", &self.agents_keymap.delete),
        ] {
            add_hint(
                self.agents_keymap.primary_hint(action, bindings),
                action,
                self.selected_row().is_some(),
            );
        }
        if self.state().editing_metadata() {
            add_hint(list_hint(ListAction::Cancel), "cancel", true);
        }
        hints.push(vec!["ctrl+c".bold(), " quit".dim()].into());
        let separator = if hints.iter().map(Line::width).sum::<usize>()
            + hints.len().saturating_sub(1) * 2
            <= usize::from(width)
        {
            "  "
        } else {
            " "
        };
        crate::footer_hint::wrap_hint_rows(hints, width, separator.len(), Line::width)
            .into_iter()
            .flat_map(|row| {
                let mut line = Line::default();
                for hint in row {
                    if !line.spans.is_empty() {
                        line.spans.push(separator.dim());
                    }
                    line.spans.extend(hint.spans);
                }
                // An individual custom chord may be wider than the entire terminal.
                crate::wrapping::word_wrap_lines([line], usize::from(width.max(1)))
            })
            .collect()
    }
}
