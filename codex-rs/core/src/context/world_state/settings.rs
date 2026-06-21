use super::WorldStateSection;
use crate::context::CollaborationModeInstructions;
use crate::context::ContextualUserFragment;
use crate::context::ModelSwitchInstructions;
use crate::context::MultiAgentModeInstructions;
use crate::context::PermissionsInstructions;
use crate::context::PersonalitySpecInstructions;
use crate::context::RealtimeEndInstructions;
use crate::context::RealtimeStartInstructions;
use crate::context::RealtimeStartWithInstructions;
use crate::session::PreviousTurnSettings;
use crate::session::multi_agents;
use crate::session::turn_context::TurnContext;
use codex_execpolicy::Policy;
use codex_features::Feature;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::MultiAgentMode;
use codex_protocol::config_types::Personality;
use codex_protocol::models::ContentItem;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::TurnContextItem;

#[derive(Debug, PartialEq)]
struct PermissionValues {
    permission_profile: PermissionProfile,
    approval_policy: AskForApproval,
}

#[derive(Debug, Default)]
pub(crate) struct SettingsState {
    model: Option<String>,
    model_instructions: String,
    permissions: Option<PermissionValues>,
    permissions_rendered: Option<String>,
    // The outer option tracks whether a baseline exists; the inner option is the effective mode.
    collaboration_mode: Option<Option<CollaborationMode>>,
    collaboration_mode_enabled: bool,
    multi_agent_mode: Option<Option<MultiAgentMode>>,
    realtime_active: Option<bool>,
    realtime_active_fallback: Option<bool>,
    realtime_start_instructions: Option<String>,
    personality_enabled: bool,
    personality_model: Option<String>,
    personality: Option<Personality>,
    personality_spec: Option<String>,
}

impl SettingsState {
    pub(crate) fn from_turn_context(
        turn_context: &TurnContext,
        exec_policy: &Policy,
        personality_enabled: bool,
    ) -> Self {
        let permissions_rendered =
            turn_context
                .config
                .include_permissions_instructions
                .then(|| {
                    PermissionsInstructions::from_permission_profile(
                        &turn_context.permission_profile,
                        turn_context.approval_policy.value(),
                        turn_context.config.approvals_reviewer,
                        exec_policy,
                        #[allow(deprecated)]
                        &turn_context.cwd,
                        turn_context
                            .config
                            .features
                            .enabled(Feature::ExecPermissionApprovals),
                        turn_context
                            .config
                            .features
                            .enabled(Feature::RequestPermissionsTool),
                    )
                    .render()
                });
        let model = turn_context.model_info.slug.clone();
        let personality = turn_context.personality;
        Self {
            model: Some(model.clone()),
            model_instructions: turn_context.model_info.get_model_instructions(personality),
            permissions: Some(PermissionValues {
                permission_profile: turn_context.permission_profile(),
                approval_policy: turn_context.approval_policy.value(),
            }),
            permissions_rendered,
            collaboration_mode: Some(Some(turn_context.collaboration_mode.clone())),
            collaboration_mode_enabled: turn_context.config.include_collaboration_mode_instructions,
            multi_agent_mode: Some(multi_agents::effective_multi_agent_mode(
                turn_context.multi_agent_version,
                &turn_context.config.multi_agent_v2,
                &turn_context.session_source,
                turn_context.multi_agent_mode,
                turn_context
                    .config
                    .features
                    .enabled(Feature::MultiAgentMode),
            )),
            realtime_active: Some(turn_context.realtime_active),
            realtime_active_fallback: None,
            realtime_start_instructions: turn_context
                .config
                .experimental_realtime_start_instructions
                .clone(),
            personality_enabled,
            personality_model: Some(model),
            personality,
            personality_spec: personality.and_then(|personality| {
                Self::personality_message(&turn_context.model_info, personality)
            }),
        }
    }

    pub(crate) fn from_turn_context_item(
        turn_context_item: &TurnContextItem,
        previous_turn_settings: Option<&PreviousTurnSettings>,
    ) -> Self {
        Self {
            model: previous_turn_settings.map(|settings| settings.model.clone()),
            model_instructions: String::new(),
            permissions: Some(PermissionValues {
                permission_profile: turn_context_item.permission_profile(),
                approval_policy: turn_context_item.approval_policy,
            }),
            permissions_rendered: None,
            collaboration_mode: Some(turn_context_item.collaboration_mode.clone()),
            collaboration_mode_enabled: false,
            multi_agent_mode: Some(turn_context_item.multi_agent_mode),
            realtime_active: turn_context_item.realtime_active,
            realtime_active_fallback: previous_turn_settings
                .and_then(|settings| settings.realtime_active),
            realtime_start_instructions: None,
            personality_enabled: false,
            personality_model: Some(turn_context_item.model.clone()),
            personality: turn_context_item.personality,
            personality_spec: None,
        }
    }

    pub(crate) fn model_update(
        previous_turn_settings: Option<&PreviousTurnSettings>,
        turn_context: &TurnContext,
    ) -> Option<String> {
        render_model_update(
            previous_turn_settings.map(|settings| settings.model.as_str()),
            Some(turn_context.model_info.slug.as_str()),
            &turn_context
                .model_info
                .get_model_instructions(turn_context.personality),
        )
    }

    pub(crate) fn realtime_update(
        previous: Option<&TurnContextItem>,
        previous_turn_settings: Option<&PreviousTurnSettings>,
        turn_context: &TurnContext,
    ) -> Option<String> {
        render_realtime_update(
            previous.and_then(|item| item.realtime_active),
            previous_turn_settings.and_then(|settings| settings.realtime_active),
            turn_context.realtime_active,
            turn_context
                .config
                .experimental_realtime_start_instructions
                .as_deref(),
        )
    }

    pub(crate) fn personality_message(
        model_info: &ModelInfo,
        personality: Personality,
    ) -> Option<String> {
        model_info
            .model_messages
            .as_ref()
            .and_then(|spec| spec.get_personality_message(Some(personality)))
            .filter(|message| !message.is_empty())
    }

    fn model_diff(&self, previous: &Self) -> Option<String> {
        render_model_update(
            previous.model.as_deref(),
            self.model.as_deref(),
            &self.model_instructions,
        )
    }

    fn permissions_diff(&self, previous: &Self) -> Option<String> {
        let rendered = self.permissions_rendered.as_ref()?;
        (self.permissions != previous.permissions).then(|| rendered.clone())
    }

    fn collaboration_mode_diff(&self, previous: &Self) -> Option<String> {
        if !self.collaboration_mode_enabled
            || self.collaboration_mode == previous.collaboration_mode
        {
            return None;
        }
        let collaboration_mode = self.collaboration_mode.as_ref()?.as_ref()?;
        CollaborationModeInstructions::from_collaboration_mode(collaboration_mode)
            .map(|instructions| instructions.render())
    }

    fn multi_agent_mode_diff(&self, previous: &Self) -> Option<String> {
        let previous_mode = previous.multi_agent_mode.as_ref()?;
        let current_mode = self.multi_agent_mode.as_ref()?;
        if current_mode == previous_mode {
            return None;
        }
        match current_mode {
            Some(mode) => Some(MultiAgentModeInstructions::new(*mode).render()),
            None if *previous_mode == Some(MultiAgentMode::Proactive) => {
                Some(MultiAgentModeInstructions::new(MultiAgentMode::ExplicitRequestOnly).render())
            }
            None => None,
        }
    }

    fn realtime_diff(&self, previous: &Self) -> Option<String> {
        render_realtime_update(
            previous.realtime_active,
            previous.realtime_active_fallback,
            self.realtime_active.unwrap_or(false),
            self.realtime_start_instructions.as_deref(),
        )
    }

    fn personality_diff(&self, previous: &Self) -> Option<String> {
        if !self.personality_enabled
            || self.personality_model != previous.personality_model
            || self.personality == previous.personality
        {
            return None;
        }
        self.personality?;
        self.personality_spec
            .as_ref()
            .map(|spec| PersonalitySpecInstructions::new(spec).render())
    }
}

impl WorldStateSection for SettingsState {
    fn render_diff(&self, previous: &Self) -> Option<ResponseItem> {
        // Keep model-switch instructions first so the new model sees its guidance before other
        // settings updates, while retaining one developer message for the complete diff.
        let content = [
            self.model_diff(previous),
            self.permissions_diff(previous),
            self.collaboration_mode_diff(previous),
            self.multi_agent_mode_diff(previous),
            self.realtime_diff(previous),
            self.personality_diff(previous),
        ]
        .into_iter()
        .flatten()
        .map(|text| ContentItem::InputText { text })
        .collect::<Vec<_>>();
        (!content.is_empty()).then(|| ResponseItem::Message {
            id: None,
            role: "developer".to_string(),
            content,
            phase: None,
            metadata: None,
        })
    }
}

fn render_model_update(
    previous_model: Option<&str>,
    current_model: Option<&str>,
    model_instructions: &str,
) -> Option<String> {
    let previous_model = previous_model?;
    if current_model == Some(previous_model) || model_instructions.is_empty() {
        return None;
    }
    Some(ModelSwitchInstructions::new(model_instructions).render())
}

fn render_realtime_update(
    previous_active: Option<bool>,
    previous_active_fallback: Option<bool>,
    active: bool,
    start_instructions: Option<&str>,
) -> Option<String> {
    match (previous_active, active) {
        (Some(true), false) => Some(RealtimeEndInstructions::new("inactive").render()),
        (Some(false), true) | (None, true) => {
            Some(if let Some(instructions) = start_instructions {
                RealtimeStartWithInstructions::new(instructions).render()
            } else {
                RealtimeStartInstructions.render()
            })
        }
        (Some(true), true) | (Some(false), false) => None,
        (None, false) => previous_active_fallback
            .filter(|realtime_active| *realtime_active)
            .map(|_| RealtimeEndInstructions::new("inactive").render()),
    }
}
