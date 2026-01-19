//! Collaboration mode selection + rendering helpers for the TUI.
//!
//! This module is intentionally UI-focused:
//! - It owns the user-facing set of selectable collaboration modes and how they cycle.
//! - It parses `/collab <mode>` arguments into a selection.
//! - It resolves a `Selection` to a concrete `codex_protocol::config_types::CollaborationMode` by
//!   picking from the `ModelsManager` builtin collaboration presets.
//! - It builds the small footer "flash" line shown after changing modes.
//!
//! The `ChatWidget` owns the session state and decides *when* selection/mode changes are allowed
//! (feature flag, task running, modals open, etc.). This module just provides the building blocks.

use crate::key_hint;
use codex_core::models_manager::manager::ModelsManager;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::Settings;
use codex_protocol::openai_models::ReasoningEffort;
use crossterm::event::KeyCode;
use ratatui::style::Stylize;
use ratatui::text::Line;

/// The user-facing collaboration mode choices supported by the TUI.
///
/// This is distinct from `CollaborationMode`: it represents a stable UI selection and the cycling
/// order, while `CollaborationMode` can carry nested settings/prompt configuration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Selection {
    Plan,
    #[default]
    PairProgramming,
    Execute,
}

impl Selection {
    /// Cycle to the next selection.
    ///
    /// The TUI cycles through a small, fixed set of presets.
    pub(crate) fn next(self) -> Self {
        match self {
            Self::Plan => Self::PairProgramming,
            Self::PairProgramming => Self::Execute,
            Self::Execute => Self::Plan,
        }
    }

    /// User-facing label used in UI surfaces like `/status` and the footer flash.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Plan => "Plan",
            Self::PairProgramming => "Pair Programming",
            Self::Execute => "Execute",
        }
    }
}

/// Parse a user argument (e.g. `/collab plan`, `/collab pair_programming`) into a selection.
///
/// The parser is forgiving: it strips whitespace, `-`, and `_`, and matches case-insensitively.
pub(crate) fn parse_selection(input: &str) -> Option<Selection> {
    let normalized: String = input
        .chars()
        .filter(|c| !c.is_ascii_whitespace() && *c != '-' && *c != '_')
        .flat_map(char::to_lowercase)
        .collect();

    match normalized.as_str() {
        "plan" => Some(Selection::Plan),
        "pair" | "pairprogramming" | "pp" => Some(Selection::PairProgramming),
        "execute" | "exec" => Some(Selection::Execute),
        _ => None,
    }
}

/// Resolve a selection to a concrete collaboration mode preset.
///
/// `ModelsManager::list_collaboration_modes()` is expected to return a builtin set of presets; this
/// function selects the first preset of the desired variant.
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

/// Resolve a selection to a concrete collaboration mode preset, falling back to a synthesized mode
/// when the desired preset is unavailable.
///
/// This keeps the TUI behavior stable when collaboration presets are missing (for example, when
/// running in offline/unit-test contexts): if the feature flag is enabled, every submission carries
/// an explicit collaboration mode so core doesn't fall back to `Custom`.
pub(crate) fn resolve_mode_or_fallback(
    models_manager: &ModelsManager,
    selection: Selection,
    fallback_model: &str,
    fallback_effort: Option<ReasoningEffort>,
) -> CollaborationMode {
    resolve_mode(models_manager, selection).unwrap_or_else(|| {
        let settings = Settings {
            model: fallback_model.to_string(),
            reasoning_effort: fallback_effort,
            developer_instructions: None,
        };

        match selection {
            Selection::Plan => CollaborationMode::Plan(settings),
            Selection::PairProgramming => CollaborationMode::PairProgramming(settings),
            Selection::Execute => CollaborationMode::Execute(settings),
        }
    })
}

/// Build a 1-line footer "flash" that is shown after switching modes.
///
/// The `ChatWidget` controls when to show this and how long it should remain visible.
pub(crate) fn flash_line(selection: Selection) -> Line<'static> {
    Line::from(vec![
        selection.label().bold(),
        " (".dim(),
        key_hint::shift(KeyCode::Tab).into(),
        " to change mode)".dim(),
    ])
}
