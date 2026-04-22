//! Keyboard shortcuts for stepping the active model's reasoning effort.
//!
//! The main chat surface treats `Alt+,` and `Alt+.` as small adjustments to the
//! current model configuration. This module keeps that behavior separate from
//! the larger `ChatWidget` key dispatcher while still reusing the same
//! model-selection and Plan-mode scope paths as the settings popups.
//!
//! The shortcut state machine is deliberately narrow: it only handles key
//! presses when no modal or popup owns input, it anchors unset reasoning to the
//! current model preset's default, and it walks only efforts advertised by the
//! active model. Unsupported current efforts are not normalized eagerly; the
//! next shortcut moves to the nearest supported effort in the requested
//! direction.

use codex_protocol::config_types::ModeKind;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use strum::IntoEnumIterator;

use super::ChatWidget;
use crate::app_event::AppEvent;
use crate::key_hint::KeyBindingListExt;
use crate::keymap::ChatKeymap;

/// Direction requested by a reasoning-level shortcut.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ReasoningShortcutDirection {
    Lower,
    Raise,
}

impl ReasoningShortcutDirection {
    fn from_key_event(key_event: KeyEvent, keymap: &ChatKeymap) -> Option<Self> {
        if key_event.kind != KeyEventKind::Press {
            return None;
        }

        if keymap.reasoning_down.is_pressed(key_event) {
            Some(Self::Lower)
        } else if keymap.reasoning_up.is_pressed(key_event) {
            Some(Self::Raise)
        } else {
            None
        }
    }

    fn bound_message(self, effort: ReasoningEffortConfig) -> String {
        let label = ChatWidget::reasoning_effort_label(effort).to_lowercase();
        match self {
            Self::Lower => format!("Reasoning is already at the lowest level ({label})."),
            Self::Raise => format!("Reasoning is already at the highest level ({label})."),
        }
    }
}

impl ChatWidget {
    /// Handles main-surface reasoning shortcuts before general key dispatch.
    ///
    /// Returning `true` means the key was recognized as a reasoning shortcut and
    /// fully handled, even if handling only produced an informational message at
    /// a boundary. Returning `false` leaves the key available to the normal chat
    /// input flow, which is important while a popup or modal has focus.
    ///
    /// Callers should route recognized shortcuts through this method rather than
    /// directly mutating reasoning state. It applies normal-mode changes without
    /// persisting them. In Plan mode, shortcuts apply only to the active
    /// Plan-mode override and skip the global-vs-Plan scope prompt.
    pub(super) fn handle_reasoning_shortcut(&mut self, key_event: KeyEvent) -> bool {
        let Some(direction) =
            ReasoningShortcutDirection::from_key_event(key_event, &self.chat_keymap)
        else {
            return false;
        };

        if !self.bottom_pane.no_modal_or_popup_active() {
            return false;
        }

        if !self.is_session_configured() {
            self.add_info_message(
                "Reasoning shortcuts are disabled until startup completes.".to_string(),
                /*hint*/ None,
            );
            return true;
        }

        let current_model = self.current_model().to_string();
        let Some(preset) = self.current_model_preset() else {
            self.add_info_message(
                format!("Reasoning shortcuts are unavailable for {current_model}."),
                /*hint*/ None,
            );
            return true;
        };

        let choices = reasoning_choices(&preset);
        let current_effort = self
            .effective_reasoning_effort()
            .unwrap_or(preset.default_reasoning_effort);
        let Some(next_effort) = next_reasoning_effort(&choices, Some(current_effort), direction)
        else {
            self.add_info_message(direction.bound_message(current_effort), /*hint*/ None);
            return true;
        };

        if self.collaboration_modes_enabled() && self.active_mode_kind() == ModeKind::Plan {
            self.app_event_tx
                .send(AppEvent::UpdatePlanModeReasoningEffort(Some(next_effort)));
        } else {
            self.apply_model_and_effort_without_persist(current_model, Some(next_effort));
        }

        true
    }

    fn current_model_preset(&self) -> Option<ModelPreset> {
        let current_model = self.current_model();
        self.model_catalog
            .try_list_models()
            .ok()?
            .into_iter()
            .find(|preset| preset.model == current_model)
    }
}

fn reasoning_choices(preset: &ModelPreset) -> Vec<ReasoningEffortConfig> {
    let mut choices = Vec::new();
    for effort in ReasoningEffortConfig::iter() {
        if preset
            .supported_reasoning_efforts
            .iter()
            .any(|option| option.effort == effort)
        {
            choices.push(effort);
        }
    }
    if choices.is_empty() {
        choices.push(preset.default_reasoning_effort);
    }
    choices
}

fn next_reasoning_effort(
    choices: &[ReasoningEffortConfig],
    current_effort: Option<ReasoningEffortConfig>,
    direction: ReasoningShortcutDirection,
) -> Option<ReasoningEffortConfig> {
    let current_effort = current_effort?;
    if choices.is_empty() {
        return None;
    }

    let current_rank = effort_rank(current_effort);
    match direction {
        ReasoningShortcutDirection::Lower => choices
            .iter()
            .rev()
            .copied()
            .find(|choice| effort_rank(*choice) < current_rank),
        ReasoningShortcutDirection::Raise => choices
            .iter()
            .copied()
            .find(|choice| effort_rank(*choice) > current_rank),
    }
}

fn effort_rank(effort: ReasoningEffortConfig) -> i32 {
    match effort {
        ReasoningEffortConfig::None => 0,
        ReasoningEffortConfig::Minimal => 1,
        ReasoningEffortConfig::Low => 2,
        ReasoningEffortConfig::Medium => 3,
        ReasoningEffortConfig::High => 4,
        ReasoningEffortConfig::XHigh => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyModifiers;
    use pretty_assertions::assert_eq;

    #[test]
    fn reasoning_shortcut_direction_uses_configured_chat_bindings() {
        let keymap = ChatKeymap {
            edit_previous_message: Vec::new(),
            confirm_edit_previous_message: Vec::new(),
            reasoning_down: vec![crate::key_hint::ctrl(KeyCode::Char(','))],
            reasoning_up: vec![crate::key_hint::ctrl(KeyCode::Char('.'))],
        };

        assert_eq!(
            ReasoningShortcutDirection::from_key_event(
                KeyEvent::new(KeyCode::Char(','), KeyModifiers::CONTROL),
                &keymap,
            ),
            Some(ReasoningShortcutDirection::Lower)
        );
        assert_eq!(
            ReasoningShortcutDirection::from_key_event(
                KeyEvent::new(KeyCode::Char('.'), KeyModifiers::CONTROL),
                &keymap,
            ),
            Some(ReasoningShortcutDirection::Raise)
        );
        assert_eq!(
            ReasoningShortcutDirection::from_key_event(
                KeyEvent::new(KeyCode::Char(','), KeyModifiers::ALT),
                &keymap,
            ),
            None
        );
    }

    #[test]
    fn next_reasoning_effort_raises_from_default_anchor() {
        let choices = vec![
            ReasoningEffortConfig::Low,
            ReasoningEffortConfig::Medium,
            ReasoningEffortConfig::High,
            ReasoningEffortConfig::XHigh,
        ];

        assert_eq!(
            next_reasoning_effort(
                &choices,
                Some(ReasoningEffortConfig::Medium),
                ReasoningShortcutDirection::Raise,
            ),
            Some(ReasoningEffortConfig::High)
        );
    }

    #[test]
    fn next_reasoning_effort_lowers_from_default_anchor() {
        let choices = vec![
            ReasoningEffortConfig::Low,
            ReasoningEffortConfig::Medium,
            ReasoningEffortConfig::High,
        ];

        assert_eq!(
            next_reasoning_effort(
                &choices,
                Some(ReasoningEffortConfig::Medium),
                ReasoningShortcutDirection::Lower,
            ),
            Some(ReasoningEffortConfig::Low)
        );
    }

    #[test]
    fn next_reasoning_effort_skips_to_supported_level_from_unsupported_current() {
        let choices = vec![ReasoningEffortConfig::Low, ReasoningEffortConfig::High];

        assert_eq!(
            next_reasoning_effort(
                &choices,
                Some(ReasoningEffortConfig::Medium),
                ReasoningShortcutDirection::Raise,
            ),
            Some(ReasoningEffortConfig::High)
        );
        assert_eq!(
            next_reasoning_effort(
                &choices,
                Some(ReasoningEffortConfig::Medium),
                ReasoningShortcutDirection::Lower,
            ),
            Some(ReasoningEffortConfig::Low)
        );
    }

    #[test]
    fn next_reasoning_effort_clamps_at_bounds() {
        let choices = vec![
            ReasoningEffortConfig::Low,
            ReasoningEffortConfig::Medium,
            ReasoningEffortConfig::High,
        ];

        assert_eq!(
            next_reasoning_effort(
                &choices,
                Some(ReasoningEffortConfig::Low),
                ReasoningShortcutDirection::Lower,
            ),
            None
        );
        assert_eq!(
            next_reasoning_effort(
                &choices,
                Some(ReasoningEffortConfig::High),
                ReasoningShortcutDirection::Raise,
            ),
            None
        );
    }

    #[test]
    fn next_reasoning_effort_single_option_is_noop() {
        let choices = vec![ReasoningEffortConfig::High];

        assert_eq!(
            next_reasoning_effort(
                &choices,
                Some(ReasoningEffortConfig::High),
                ReasoningShortcutDirection::Raise,
            ),
            None
        );
        assert_eq!(
            next_reasoning_effort(
                &choices,
                Some(ReasoningEffortConfig::High),
                ReasoningShortcutDirection::Lower,
            ),
            None
        );
    }
}
