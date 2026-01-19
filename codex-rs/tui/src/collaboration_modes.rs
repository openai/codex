use crate::key_hint;
use codex_core::models_manager::manager::ModelsManager;
use codex_protocol::config_types::CollaborationMode;
use crossterm::event::KeyCode;
use ratatui::style::Stylize;
use ratatui::text::Line;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Selection {
    Plan,
    PairProgramming,
    Execute,
}

impl Default for Selection {
    fn default() -> Self {
        Self::PairProgramming
    }
}

impl Selection {
    pub(crate) fn next(self) -> Self {
        match self {
            Self::Plan => Self::PairProgramming,
            Self::PairProgramming => Self::Execute,
            Self::Execute => Self::Plan,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Plan => "Plan",
            Self::PairProgramming => "Pair Programming",
            Self::Execute => "Execute",
        }
    }
}

pub(crate) fn parse_selection(input: &str) -> Option<Selection> {
    let normalized: String = input
        .chars()
        .filter(|c| !c.is_ascii_whitespace() && *c != '-' && *c != '_')
        .flat_map(|c| c.to_lowercase())
        .collect();

    match normalized.as_str() {
        "plan" => Some(Selection::Plan),
        "pair" | "pairprogramming" | "pp" => Some(Selection::PairProgramming),
        "execute" | "exec" => Some(Selection::Execute),
        _ => None,
    }
}

pub(crate) fn resolve_mode(
    models_manager: &ModelsManager,
    selection: Selection,
) -> Option<CollaborationMode> {
    match selection {
        Selection::Plan => models_manager
            .list_collaboration_modes()
            .into_iter()
            .find(|mode| matches!(mode, CollaborationMode::Plan(_))),
        Selection::PairProgramming => models_manager
            .list_collaboration_modes()
            .into_iter()
            .find(|mode| matches!(mode, CollaborationMode::PairProgramming(_))),
        Selection::Execute => models_manager
            .list_collaboration_modes()
            .into_iter()
            .find(|mode| matches!(mode, CollaborationMode::Execute(_))),
    }
}

pub(crate) fn flash_line(selection: Selection) -> Line<'static> {
    Line::from(vec![
        selection.label().bold(),
        " (".dim(),
        key_hint::shift(KeyCode::Tab).into(),
        " to change mode)".dim(),
    ])
}
