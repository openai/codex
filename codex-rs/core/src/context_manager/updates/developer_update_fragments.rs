//! Developer-envelope model-visible fragments used by steady-state settings
//! updates (`build_settings_update_items`).
//!
//! This module owns the turn-context diffing logic for developer-role context
//! updates (permissions, collaboration mode, realtime, personality, and model
//! switch guidance).

use crate::codex::PreviousTurnSettings;
use crate::codex::TurnContext;
use crate::features::Feature;
use crate::model_visible_context::DeveloperContextRole;
use crate::model_visible_context::ModelVisibleContextFragment;
use crate::model_visible_context::TurnContextDiffFragment;
use crate::model_visible_context::TurnContextDiffParams;
use codex_protocol::config_types::Personality;
use codex_protocol::models::developer_collaboration_mode_text;
use codex_protocol::models::developer_model_switch_text;
use codex_protocol::models::developer_permissions_text;
use codex_protocol::models::developer_personality_spec_text;
use codex_protocol::models::developer_realtime_end_text;
use codex_protocol::models::developer_realtime_start_text;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::protocol::TurnContextItem;

// ---------------------------------------------------------------------------
// Settings-update fragments for the developer envelope
// ---------------------------------------------------------------------------
//
// Contextual-user settings-update fragments are assembled in `updates.rs`
// (currently `EnvironmentContext::diff_from_turn_context_item(...)`) and the
// corresponding fragment types live in their own modules (for example
// `environment_context.rs` and `instructions/contextual_user_fragments.rs`).

struct PermissionsUpdateFragment {
    text: String,
}

impl ModelVisibleContextFragment for PermissionsUpdateFragment {
    type Role = DeveloperContextRole;

    fn render_text(&self) -> String {
        self.text.clone()
    }
}

impl TurnContextDiffFragment for PermissionsUpdateFragment {
    fn diff_from_turn_context_item(
        previous: &TurnContextItem,
        turn_context: &TurnContext,
        params: &TurnContextDiffParams<'_>,
    ) -> Option<Self> {
        if previous.sandbox_policy == *turn_context.sandbox_policy.get()
            && previous.approval_policy == turn_context.approval_policy.value()
        {
            return None;
        }

        Some(Self {
            text: developer_permissions_text(
                turn_context.sandbox_policy.get(),
                turn_context.approval_policy.value(),
                params.exec_policy,
                &turn_context.cwd,
                turn_context.features.enabled(Feature::RequestPermissions),
            ),
        })
    }
}

struct CustomDeveloperInstructionsUpdateFragment {
    text: String,
}

impl ModelVisibleContextFragment for CustomDeveloperInstructionsUpdateFragment {
    type Role = DeveloperContextRole;

    fn render_text(&self) -> String {
        self.text.clone()
    }
}

impl TurnContextDiffFragment for CustomDeveloperInstructionsUpdateFragment {
    fn diff_from_turn_context_item(
        previous: &TurnContextItem,
        turn_context: &TurnContext,
        _params: &TurnContextDiffParams<'_>,
    ) -> Option<Self> {
        if previous.developer_instructions == turn_context.developer_instructions {
            return None;
        }

        turn_context
            .developer_instructions
            .as_ref()
            .map(|text| Self { text: text.clone() })
    }
}

struct CollaborationModeUpdateFragment {
    text: String,
}

impl ModelVisibleContextFragment for CollaborationModeUpdateFragment {
    type Role = DeveloperContextRole;

    fn render_text(&self) -> String {
        self.text.clone()
    }
}

impl TurnContextDiffFragment for CollaborationModeUpdateFragment {
    fn diff_from_turn_context_item(
        previous: &TurnContextItem,
        turn_context: &TurnContext,
        _params: &TurnContextDiffParams<'_>,
    ) -> Option<Self> {
        if previous.collaboration_mode.as_ref() != Some(&turn_context.collaboration_mode) {
            // If the next mode has empty developer instructions, this returns None and we emit no
            // update, so prior collaboration instructions remain in the prompt history.
            Some(Self {
                text: developer_collaboration_mode_text(&turn_context.collaboration_mode)?,
            })
        } else {
            None
        }
    }
}

struct RealtimeUpdateFragment {
    text: String,
}

impl ModelVisibleContextFragment for RealtimeUpdateFragment {
    type Role = DeveloperContextRole;

    fn render_text(&self) -> String {
        self.text.clone()
    }
}

impl TurnContextDiffFragment for RealtimeUpdateFragment {
    fn from_turn_context(
        turn_context: &TurnContext,
        params: &TurnContextDiffParams<'_>,
    ) -> Option<Self> {
        if turn_context.realtime_active {
            return Some(Self {
                text: developer_realtime_start_text(),
            });
        }

        params
            .previous_turn_settings
            .and_then(|settings| settings.realtime_active)
            .filter(|realtime_active| *realtime_active)
            .map(|_| Self {
                text: developer_realtime_end_text("inactive"),
            })
    }

    fn diff_from_turn_context_item(
        previous: &TurnContextItem,
        turn_context: &TurnContext,
        params: &TurnContextDiffParams<'_>,
    ) -> Option<Self> {
        match (previous.realtime_active, turn_context.realtime_active) {
            (Some(true), false) => Some(Self {
                text: developer_realtime_end_text("inactive"),
            }),
            (Some(false), true) | (None, true) => Some(Self {
                text: developer_realtime_start_text(),
            }),
            (Some(true), true) | (Some(false), false) => None,
            (None, false) => params
                .previous_turn_settings
                .and_then(|settings| settings.realtime_active)
                .filter(|realtime_active| *realtime_active)
                .map(|_| Self {
                    text: developer_realtime_end_text("inactive"),
                }),
        }
    }
}

struct PersonalityUpdateFragment {
    text: String,
}

impl ModelVisibleContextFragment for PersonalityUpdateFragment {
    type Role = DeveloperContextRole;

    fn render_text(&self) -> String {
        self.text.clone()
    }
}

impl TurnContextDiffFragment for PersonalityUpdateFragment {
    fn from_turn_context(
        turn_context: &TurnContext,
        params: &TurnContextDiffParams<'_>,
    ) -> Option<Self> {
        if !params.personality_feature_enabled {
            return None;
        }

        let personality = turn_context.personality?;
        let personality_message = personality_message_for(&turn_context.model_info, personality)?;
        Some(Self {
            text: developer_personality_spec_text(personality_message),
        })
    }

    fn diff_from_turn_context_item(
        previous: &TurnContextItem,
        turn_context: &TurnContext,
        params: &TurnContextDiffParams<'_>,
    ) -> Option<Self> {
        if !params.personality_feature_enabled {
            return None;
        }
        if turn_context.model_info.slug != previous.model {
            return None;
        }

        if let Some(personality) = turn_context.personality
            && turn_context.personality != previous.personality
        {
            let model_info = &turn_context.model_info;
            let personality_message = personality_message_for(model_info, personality)?;
            Some(Self {
                text: developer_personality_spec_text(personality_message),
            })
        } else {
            None
        }
    }
}

struct ModelInstructionsUpdateFragment {
    text: String,
}

impl ModelVisibleContextFragment for ModelInstructionsUpdateFragment {
    type Role = DeveloperContextRole;

    fn render_text(&self) -> String {
        self.text.clone()
    }
}

impl TurnContextDiffFragment for ModelInstructionsUpdateFragment {
    fn from_turn_context(
        turn_context: &TurnContext,
        params: &TurnContextDiffParams<'_>,
    ) -> Option<Self> {
        let previous_turn_settings = params.previous_turn_settings?;
        if previous_turn_settings.model == turn_context.model_info.slug {
            return None;
        }

        let model_instructions = turn_context
            .model_info
            .get_model_instructions(turn_context.personality);
        if model_instructions.is_empty() {
            return None;
        }

        Some(Self {
            text: developer_model_switch_text(model_instructions),
        })
    }

    fn diff_from_turn_context_item(
        previous: &TurnContextItem,
        turn_context: &TurnContext,
        _params: &TurnContextDiffParams<'_>,
    ) -> Option<Self> {
        if previous.model == turn_context.model_info.slug {
            return None;
        }

        let model_instructions = turn_context
            .model_info
            .get_model_instructions(turn_context.personality);
        if model_instructions.is_empty() {
            return None;
        }

        Some(Self {
            text: developer_model_switch_text(model_instructions),
        })
    }
}

// ---------------------------------------------------------------------------
// Fragment list assembly
// ---------------------------------------------------------------------------

pub(super) fn build_developer_update_texts(
    previous: Option<&TurnContextItem>,
    next: &TurnContext,
    params: &TurnContextDiffParams<'_>,
) -> Vec<String> {
    [
        // Keep model-switch instructions first so model-specific guidance is read before
        // any other context diffs on this turn.
        ModelInstructionsUpdateFragment::from_turn_context(next, params)
            .map(|fragment| fragment.text),
        previous
            .and_then(|previous| {
                PermissionsUpdateFragment::diff_from_turn_context_item(previous, next, params)
            })
            .map(|fragment| fragment.text),
        previous
            .and_then(|previous| {
                CustomDeveloperInstructionsUpdateFragment::diff_from_turn_context_item(
                    previous, next, params,
                )
            })
            .map(|fragment| fragment.text),
        previous
            .and_then(|previous| {
                CollaborationModeUpdateFragment::diff_from_turn_context_item(previous, next, params)
            })
            .map(|fragment| fragment.text),
        match previous {
            Some(previous) => {
                RealtimeUpdateFragment::diff_from_turn_context_item(previous, next, params)
            }
            None => RealtimeUpdateFragment::from_turn_context(next, params),
        }
        .map(|fragment| fragment.text),
        match previous {
            Some(previous) => {
                PersonalityUpdateFragment::diff_from_turn_context_item(previous, next, params)
            }
            None => PersonalityUpdateFragment::from_turn_context(next, params),
        }
        .map(|fragment| fragment.text),
    ]
    .into_iter()
    .flatten()
    .collect()
}

// ---------------------------------------------------------------------------
// Shared helper exports used outside this module
// ---------------------------------------------------------------------------

pub(crate) fn personality_message_for(
    model_info: &ModelInfo,
    personality: Personality,
) -> Option<String> {
    model_info
        .model_messages
        .as_ref()
        .and_then(|spec| spec.get_personality_message(Some(personality)))
        .filter(|message| !message.is_empty())
}

pub(crate) fn build_model_instructions_update_item(
    previous_turn_settings: Option<&PreviousTurnSettings>,
    turn_context: &TurnContext,
) -> Option<String> {
    let previous_turn_settings = previous_turn_settings?;
    if previous_turn_settings.model == turn_context.model_info.slug {
        return None;
    }

    let model_instructions = turn_context
        .model_info
        .get_model_instructions(turn_context.personality);
    if model_instructions.is_empty() {
        return None;
    }

    Some(developer_model_switch_text(model_instructions))
}

pub(crate) fn build_realtime_update_item(
    previous: Option<&TurnContextItem>,
    previous_turn_settings: Option<&PreviousTurnSettings>,
    turn_context: &TurnContext,
) -> Option<String> {
    match (
        previous.and_then(|item| item.realtime_active),
        turn_context.realtime_active,
    ) {
        (Some(true), false) => Some(developer_realtime_end_text("inactive")),
        (Some(false), true) | (None, true) => Some(developer_realtime_start_text()),
        (Some(true), true) | (Some(false), false) => None,
        (None, false) => previous_turn_settings
            .and_then(|settings| settings.realtime_active)
            .filter(|realtime_active| *realtime_active)
            .map(|_| developer_realtime_end_text("inactive")),
    }
}
