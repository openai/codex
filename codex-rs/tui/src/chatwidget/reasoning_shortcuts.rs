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
//! active model. Unsupported efforts anchor to the model default, or the first
//! advertised effort when the default is absent, before stepping through the
//! advertised order. Raising never silently crosses into Max or Ultra; those
//! efforts require the explicit advanced-reasoning picker.

use codex_protocol::config_types::ModeKind;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use crossterm::event::KeyEvent;

use super::ChatWidget;
use crate::app_event::AppEvent;
use crate::key_hint::KeyBindingListExt;

/// Direction requested by a reasoning-level shortcut.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ReasoningShortcutDirection {
    Lower,
    Raise,
}

impl ReasoningShortcutDirection {
    fn bound_message(self, effort: &ReasoningEffortConfig) -> String {
        let label = ChatWidget::reasoning_effort_sentence_label(effort);
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
        let direction = if self
            .chat_keymap
            .decrease_reasoning_effort
            .is_pressed(key_event)
        {
            ReasoningShortcutDirection::Lower
        } else if self
            .chat_keymap
            .increase_reasoning_effort
            .is_pressed(key_event)
        {
            ReasoningShortcutDirection::Raise
        } else {
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
        let configured_effort = self
            .effective_reasoning_effort()
            .unwrap_or_else(|| preset.default_reasoning_effort.clone());
        let current_effort = if choices.contains(&configured_effort) {
            configured_effort
        } else if choices.contains(&preset.default_reasoning_effort) {
            preset.default_reasoning_effort
        } else {
            choices
                .first()
                .cloned()
                .unwrap_or(preset.default_reasoning_effort)
        };
        let Some(next_effort) =
            next_reasoning_effort(&choices, Some(current_effort.clone()), direction)
        else {
            self.add_info_message(direction.bound_message(&current_effort), /*hint*/ None);
            return true;
        };

        if direction == ReasoningShortcutDirection::Raise
            && Self::is_advanced_reasoning_effort(&next_effort)
        {
            let advanced_label = choices
                .iter()
                .filter(|effort| Self::is_advanced_reasoning_effort(effort))
                .map(Self::reasoning_effort_label)
                .collect::<Vec<_>>()
                .join(" and ");
            let verb = if advanced_label.contains(" and ") {
                "are"
            } else {
                "is"
            };
            let model_path = if current_model.starts_with("codex-auto-") {
                current_model
            } else {
                format!("All models → {current_model}")
            };
            self.add_info_message(
                format!(
                    "{advanced_label} {verb} available under /model → {model_path} → More reasoning…"
                ),
                /*hint*/ None,
            );
            return true;
        }

        if self.collaboration_modes_enabled() && self.active_mode_kind() == ModeKind::Plan {
            let warning = self.ultra_reasoning_concurrency_warning(&next_effort);
            self.app_event_tx
                .send(AppEvent::UpdatePlanModeReasoningEffort(Some(next_effort)));
            if let Some(warning) = warning {
                self.app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
                    crate::history_cell::new_warning_event(warning),
                )));
            }
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
    let (mut choices, mut advanced_choices): (Vec<_>, Vec<_>) = preset
        .supported_reasoning_efforts
        .iter()
        .map(|option| option.effort.clone())
        .partition(|effort| !ChatWidget::is_advanced_reasoning_effort(effort));
    advanced_choices.sort_by_key(|effort| matches!(effort, ReasoningEffortConfig::Ultra));
    choices.extend(advanced_choices);
    if choices.is_empty() {
        choices.push(preset.default_reasoning_effort.clone());
    }
    choices
}

fn next_reasoning_effort(
    choices: &[ReasoningEffortConfig],
    current_effort: Option<ReasoningEffortConfig>,
    direction: ReasoningShortcutDirection,
) -> Option<ReasoningEffortConfig> {
    let current_effort = current_effort?;
    if let Some(current_index) = choices.iter().position(|choice| choice == &current_effort) {
        return match direction {
            ReasoningShortcutDirection::Lower => current_index
                .checked_sub(1)
                .and_then(|index| choices.get(index))
                .cloned(),
            ReasoningShortcutDirection::Raise => choices.get(current_index + 1).cloned(),
        };
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

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
    fn next_reasoning_effort_does_not_infer_position_for_unsupported_current() {
        let choices = vec![ReasoningEffortConfig::Low, ReasoningEffortConfig::High];

        assert_eq!(
            (
                next_reasoning_effort(
                    &choices,
                    Some(ReasoningEffortConfig::Medium),
                    ReasoningShortcutDirection::Raise,
                ),
                next_reasoning_effort(
                    &choices,
                    Some(ReasoningEffortConfig::Medium),
                    ReasoningShortcutDirection::Lower,
                ),
            ),
            (None, None)
        );
    }

    #[test]
    fn next_reasoning_effort_uses_advertised_order_for_custom_levels() {
        let custom_effort = ReasoningEffortConfig::Custom("future".to_string());
        let choices = vec![
            ReasoningEffortConfig::High,
            ReasoningEffortConfig::Low,
            custom_effort.clone(),
        ];

        assert_eq!(
            (
                next_reasoning_effort(
                    &choices,
                    Some(ReasoningEffortConfig::High),
                    ReasoningShortcutDirection::Raise,
                ),
                next_reasoning_effort(
                    &choices,
                    Some(custom_effort),
                    ReasoningShortcutDirection::Lower,
                ),
            ),
            (
                Some(ReasoningEffortConfig::Low),
                Some(ReasoningEffortConfig::Low),
            )
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
