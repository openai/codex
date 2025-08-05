//! Generic selection popup for multiple use-cases (models, execution modes, etc.).
//!
//! Construct with `new_model` or `new_execution_modes`, pass key events to move
//! selection and update the query via `set_query`, then render with
//! `selection_popup_common::render_rows`.
use codex_core::protocol::AskForApproval;
use codex_core::protocol::SandboxPolicy;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::WidgetRef;

use super::popup_consts::MAX_POPUP_ROWS;
use super::selection_list::SelectionItem;
use super::selection_list::SelectionList;
use super::selection_popup_common::GenericDisplayRow;
use super::selection_popup_common::render_rows;
use crate::command_utils::ExecutionPreset;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SelectionKind {
    Model,
    Execution,
}

#[derive(Clone)]
pub(crate) enum SelectionValue {
    Model(String),
    Execution {
        approval: AskForApproval,
        sandbox: SandboxPolicy,
    },
}

pub(crate) struct SelectionPopup {
    kind: SelectionKind,
    list: SelectionList<SelectionValue>,
}

impl SelectionPopup {
    pub(crate) fn new_model(current_model: &str, options: Vec<String>) -> Self {
        let mut items: Vec<SelectionItem<SelectionValue>> = Vec::new();
        items.push(
            SelectionItem::new(
                SelectionValue::Model(current_model.to_string()),
                current_model.to_string(),
            )
            .mark_current(true),
        );
        for m in options.into_iter().filter(|m| m != current_model) {
            items.push(SelectionItem::new(SelectionValue::Model(m.clone()), m));
        }
        Self {
            kind: SelectionKind::Model,
            list: SelectionList::new(items),
        }
    }

    pub(crate) fn new_execution_modes(
        current_approval: AskForApproval,
        current_sandbox: &SandboxPolicy,
    ) -> Self {
        let presets: Vec<ExecutionPreset> = vec![
            ExecutionPreset::ReadOnly,
            ExecutionPreset::Untrusted,
            ExecutionPreset::Auto,
            ExecutionPreset::FullYolo,
        ];

        let mut items: Vec<SelectionItem<SelectionValue>> = Vec::new();
        for p in presets.into_iter() {
            let (a, s) = p.to_policies();
            let name = p.label().to_string();
            let desc = Some(p.description().to_string());
            let mut item = SelectionItem::new(
                SelectionValue::Execution {
                    approval: a,
                    sandbox: s.clone(),
                },
                name,
            )
            .with_description(desc)
            .with_aliases(match p {
                ExecutionPreset::ReadOnly => vec!["read-only".to_string(), "readonly".to_string()],
                ExecutionPreset::Untrusted => vec!["untrusted".to_string()],
                ExecutionPreset::Auto => vec!["auto".to_string()],
                ExecutionPreset::FullYolo => vec!["full-yolo".to_string(), "full yolo".to_string()],
            });
            if ExecutionPreset::from_policies(current_approval, current_sandbox) == Some(p) {
                item = item.mark_current(true);
            }
            items.push(item);
        }
        Self {
            kind: SelectionKind::Execution,
            list: SelectionList::new(items),
        }
    }

    pub(crate) fn kind(&self) -> SelectionKind {
        self.kind
    }

    pub(crate) fn set_query(&mut self, query: &str) {
        self.list.set_query(query);
    }
    pub(crate) fn move_up(&mut self) {
        self.list.move_up();
    }
    pub(crate) fn move_down(&mut self) {
        self.list.move_down();
    }
    pub(crate) fn calculate_required_height(&self) -> u16 {
        self.list.visible_rows().len().clamp(1, MAX_POPUP_ROWS) as u16
    }
    pub(crate) fn selected_value(&self) -> Option<SelectionValue> {
        self.list.selected_value()
    }

    fn visible_rows(&self) -> Vec<GenericDisplayRow> {
        self.list
            .visible_rows()
            .into_iter()
            .map(|(row, _)| row)
            .collect()
    }
}

impl WidgetRef for &SelectionPopup {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let rows_all = self.visible_rows();
        render_rows(area, buf, &rows_all, &self.list.state, MAX_POPUP_ROWS);
    }
}

#[cfg(test)]
mod tests {
    use codex_core::protocol::AskForApproval;
    use codex_core::protocol::SandboxPolicy;

    #[test]
    fn execution_selector_includes_full_yolo() {
        // Set a benign current mode; we only care about rows.
        let popup = super::SelectionPopup::new_execution_modes(
            AskForApproval::OnFailure,
            &SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![],
                network_access: false,
                include_default_writable_roots: true,
            },
        );
        let rows = popup.visible_rows();
        let labels: Vec<String> = rows.into_iter().map(|r| r.name).collect();
        assert!(
            labels.iter().any(|l| l.contains("Danger")),
            "selector should include 'Danger'"
        );
    }
}
