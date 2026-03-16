use ratatui::style::Stylize;
use ratatui::text::Line;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TurnContextSnapshot {
    pub permissions: String,
    pub mode: String,
    pub model: String,
    pub personality: String,
    pub cwd: String,
}

impl TurnContextSnapshot {
    pub(crate) fn diff_lines(&self, current: &Self) -> Option<Vec<Line<'static>>> {
        let mut diffs: Vec<(&'static str, &str, &str)> = Vec::new();

        if self.permissions != current.permissions {
            diffs.push(("permissions", &self.permissions, &current.permissions));
        }
        if self.mode != current.mode {
            diffs.push(("mode", &self.mode, &current.mode));
        }
        if self.model != current.model {
            diffs.push(("model", &self.model, &current.model));
        }
        if self.personality != current.personality {
            diffs.push(("personality", &self.personality, &current.personality));
        }
        if self.cwd != current.cwd {
            diffs.push(("directory", &self.cwd, &current.cwd));
        }

        if diffs.is_empty() {
            return None;
        }

        let mut lines = vec![vec!["  ".into(), "Context changed since this turn:".bold()].into()];

        for (label, then_value, now_value) in diffs {
            lines.push(
                vec![
                    "  ".into(),
                    format!("{label}: ").dim(),
                    then_value.to_string().dim(),
                    " -> ".dim(),
                    now_value.to_string().cyan(),
                ]
                .into(),
            );
        }
        lines.push("".into());

        Some(lines)
    }

    pub(crate) fn rollback_diff_messages(&self, current: &Self) -> Vec<String> {
        let mut messages = Vec::new();

        if self.permissions != current.permissions {
            messages.push(format!("Permissions updated to {}", current.permissions));
        }

        let mode_changed = self.mode != current.mode;
        let model_changed = self.model != current.model;
        if model_changed {
            let mut message = format!("Model changed to {}", current.model);
            if mode_changed {
                message.push_str(" for ");
                message.push_str(&current.mode);
                message.push_str(" mode.");
            }
            messages.push(message);
        } else if mode_changed {
            messages.push(format!("Switched to {} mode.", current.mode));
        }

        if self.personality != current.personality {
            messages.push(format!("Personality set to {}", current.personality));
        }

        if self.cwd != current.cwd {
            messages.push(format!("Directory changed to {}", current.cwd));
        }

        messages
    }
}

#[cfg(test)]
mod tests {
    use super::TurnContextSnapshot;
    use pretty_assertions::assert_eq;

    fn render_lines(lines: &[ratatui::text::Line<'static>]) -> Vec<String> {
        lines.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn diff_lines_returns_none_when_context_matches() {
        let snapshot = TurnContextSnapshot {
            permissions: "Default".to_string(),
            mode: "Default".to_string(),
            model: "gpt-5.4 xhigh fast".to_string(),
            personality: "Default".to_string(),
            cwd: "~/code/codex".to_string(),
        };

        assert_eq!(snapshot.diff_lines(&snapshot), None);
    }

    #[test]
    fn diff_lines_lists_changed_fields_in_stable_order() {
        let then = TurnContextSnapshot {
            permissions: "Default".to_string(),
            mode: "Default".to_string(),
            model: "gpt-5.4 high".to_string(),
            personality: "Default".to_string(),
            cwd: "~/code/old".to_string(),
        };
        let now = TurnContextSnapshot {
            permissions: "Smart Approvals".to_string(),
            mode: "Plan".to_string(),
            model: "gpt-5.4 xhigh fast".to_string(),
            personality: "Pragmatic".to_string(),
            cwd: "~/code/new".to_string(),
        };

        let lines = then.diff_lines(&now).expect("expected diff lines");

        assert_eq!(
            render_lines(&lines),
            vec![
                "  Context changed since this turn:".to_string(),
                "  permissions: Default -> Smart Approvals".to_string(),
                "  mode: Default -> Plan".to_string(),
                "  model: gpt-5.4 high -> gpt-5.4 xhigh fast".to_string(),
                "  personality: Default -> Pragmatic".to_string(),
                "  directory: ~/code/old -> ~/code/new".to_string(),
                String::new(),
            ]
        );
    }

    #[test]
    fn rollback_diff_messages_use_existing_tui_phrasing() {
        let then = TurnContextSnapshot {
            permissions: "Default".to_string(),
            mode: "Default".to_string(),
            model: "gpt-5.4 high".to_string(),
            personality: "Default".to_string(),
            cwd: "~/code/old".to_string(),
        };
        let now = TurnContextSnapshot {
            permissions: "Smart Approvals".to_string(),
            mode: "Plan".to_string(),
            model: "gpt-5.4 xhigh fast".to_string(),
            personality: "Pragmatic".to_string(),
            cwd: "~/code/new".to_string(),
        };

        assert_eq!(
            then.rollback_diff_messages(&now),
            vec![
                "Permissions updated to Smart Approvals".to_string(),
                "Model changed to gpt-5.4 xhigh fast for Plan mode.".to_string(),
                "Personality set to Pragmatic".to_string(),
                "Directory changed to ~/code/new".to_string(),
            ]
        );
    }
}
