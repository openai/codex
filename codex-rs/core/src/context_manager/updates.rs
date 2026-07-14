use crate::context::ApprovalPromptContext;
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
use crate::session::turn_context::TurnContext;
use codex_execpolicy::Policy;
use codex_features::Feature;
use codex_protocol::config_types::MultiAgentMode;
use codex_protocol::config_types::Personality;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::protocol::TurnContextItem;

fn build_permissions_update_item(
    previous: Option<&TurnContextItem>,
    next: &TurnContext,
    exec_policy: &Policy,
) -> Option<String> {
    if !next.config.include_permissions_instructions {
        return None;
    }

    let prev = previous?;
    if prev.permission_profile() == next.permission_profile()
        && prev.approval_policy == next.approval_policy.value()
        && prev.model == next.model_info.slug
    {
        return None;
    }

    Some(
        PermissionsInstructions::from_permission_profile(
            &next.permission_profile,
            next.approval_policy.value(),
            ApprovalPromptContext::new(
                next.config.approvals_reviewer,
                next.model_info
                    .model_messages
                    .as_ref()
                    .and_then(|messages| messages.approvals.as_ref()),
                next.model_info
                    .model_messages
                    .as_ref()
                    .and_then(|messages| messages.permissions.as_ref()),
            ),
            exec_policy,
            #[allow(deprecated)]
            &next.cwd,
            next.config
                .features
                .enabled(Feature::ExecPermissionApprovals),
            next.config
                .features
                .enabled(Feature::RequestPermissionsTool),
        )
        .render(),
    )
}

fn build_collaboration_mode_update_item(
    previous: Option<&TurnContextItem>,
    next: &TurnContext,
) -> Option<String> {
    if !next.config.include_collaboration_mode_instructions {
        return None;
    }

    let prev = previous?;
    let collaboration_mode = next.collaboration_mode();
    if prev.collaboration_mode.as_ref() != Some(&collaboration_mode) {
        // If the next mode has empty developer instructions, this returns None and we emit no
        // update, so prior collaboration instructions remain in the prompt history.
        Some(CollaborationModeInstructions::from_collaboration_mode(&collaboration_mode)?.render())
    } else {
        None
    }
}

fn build_multi_agent_mode_update_item(
    previous: Option<&TurnContextItem>,
    next: &TurnContext,
) -> Option<String> {
    let effective_multi_agent_mode = crate::session::multi_agents::effective_multi_agent_mode(next);
    let previous = previous?;
    if previous.multi_agent_mode == effective_multi_agent_mode {
        return None;
    }

    match effective_multi_agent_mode {
        Some(multi_agent_mode) => Some(MultiAgentModeInstructions::new(multi_agent_mode).render()),
        None if previous.multi_agent_mode == Some(MultiAgentMode::Proactive) => {
            Some(MultiAgentModeInstructions::new(MultiAgentMode::ExplicitRequestOnly).render())
        }
        None => None,
    }
}

pub(crate) fn build_realtime_update_item(
    previous: Option<&TurnContextItem>,
    previous_turn_settings: Option<&PreviousTurnSettings>,
    next: &TurnContext,
) -> Option<String> {
    match (
        previous.and_then(|item| item.realtime_active),
        next.realtime_active,
    ) {
        (Some(true), false) => Some(RealtimeEndInstructions::new("inactive").render()),
        (Some(false), true) | (None, true) => Some(
            if let Some(instructions) = next
                .config
                .experimental_realtime_start_instructions
                .as_deref()
            {
                RealtimeStartWithInstructions::new(instructions).render()
            } else {
                RealtimeStartInstructions.render()
            },
        ),
        (Some(true), true) | (Some(false), false) => None,
        (None, false) => previous_turn_settings
            .and_then(|settings| settings.realtime_active)
            .filter(|realtime_active| *realtime_active)
            .map(|_| RealtimeEndInstructions::new("inactive").render()),
    }
}

pub(crate) fn build_initial_realtime_item(
    previous: Option<&TurnContextItem>,
    previous_turn_settings: Option<&PreviousTurnSettings>,
    next: &TurnContext,
) -> Option<String> {
    build_realtime_update_item(previous, previous_turn_settings, next)
}

fn build_personality_update_item(
    previous: Option<&TurnContextItem>,
    next: &TurnContext,
    personality_feature_enabled: bool,
) -> Option<String> {
    if !personality_feature_enabled {
        return None;
    }
    let previous = previous?;
    if next.model_info.slug != previous.model {
        return None;
    }

    if let Some(personality) = next.personality
        && next.personality != previous.personality
    {
        let model_info = &next.model_info;
        let personality_message = personality_message_for(model_info, personality);
        personality_message.map(|message| PersonalitySpecInstructions::new(message).render())
    } else {
        None
    }
}

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
    next: &TurnContext,
) -> Option<String> {
    let previous_turn_settings = previous_turn_settings?;
    if previous_turn_settings.model == next.model_info.slug {
        return None;
    }

    let model_instructions = next.model_info.get_model_instructions(next.personality);
    if model_instructions.is_empty() {
        return None;
    }

    Some(ModelSwitchInstructions::new(model_instructions).render())
}

pub(crate) fn build_developer_update_item(text_sections: Vec<String>) -> Option<ResponseItem> {
    build_text_message("developer", text_sections)
}

pub(crate) fn build_contextual_user_message(text_sections: Vec<String>) -> Option<ResponseItem> {
    build_text_message("user", text_sections)
}

pub(crate) fn merge_contextual_fragments(
    fragments: Vec<Box<dyn ContextualUserFragment>>,
) -> Vec<ResponseItem> {
    let mut messages: Vec<(&str, Vec<String>)> = Vec::with_capacity(fragments.len());
    for fragment in fragments {
        let role = fragment.role();
        let text = fragment.render();
        match messages.last_mut() {
            Some((previous_role, text_sections)) if *previous_role == role => {
                text_sections.push(text);
            }
            _ => messages.push((role, vec![text])),
        }
    }
    messages
        .into_iter()
        .filter_map(|(role, text_sections)| build_text_message(role, text_sections))
        .collect()
}

fn build_text_message(role: &str, text_sections: Vec<String>) -> Option<ResponseItem> {
    if text_sections.is_empty() {
        return None;
    }

    let content = text_sections
        .into_iter()
        .map(|text| ContentItem::InputText { text })
        .collect();

    Some(ResponseItem::Message {
        id: None,
        role: role.to_string(),
        content,
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    })
}

pub(crate) fn build_settings_update_items(
    previous: Option<&TurnContextItem>,
    previous_turn_settings: Option<&PreviousTurnSettings>,
    next: &TurnContext,
    exec_policy: &Policy,
    personality_feature_enabled: bool,
) -> Vec<ResponseItem> {
    // TODO(ccunningham): build_settings_update_items still does not cover every
    // model-visible item emitted by build_initial_context. Persist the remaining
    // inputs or add explicit replay events so fork/resume can diff everything
    // deterministically.
    let developer_update_sections = [
        // Keep model-switch instructions first so model-specific guidance is read before
        // any other context diffs on this turn.
        build_model_instructions_update_item(previous_turn_settings, next),
        build_permissions_update_item(previous, next, exec_policy),
        build_collaboration_mode_update_item(previous, next),
        build_multi_agent_mode_update_item(previous, next),
        build_realtime_update_item(previous, previous_turn_settings, next),
        build_personality_update_item(previous, next, personality_feature_enabled),
    ]
    .into_iter()
    .flatten()
    .collect();

    build_developer_update_item(developer_update_sections)
        .into_iter()
        .collect()
}
