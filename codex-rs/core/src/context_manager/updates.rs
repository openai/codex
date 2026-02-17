use crate::codex::TurnContext;
use crate::environment_context::EnvironmentContext;
use crate::shell::Shell;
use codex_execpolicy::Policy;
use codex_protocol::config_types::Personality;
use codex_protocol::models::DeveloperInstructions;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ModelInfo;

fn build_environment_update_item(
    previous: Option<&TurnContext>,
    next: &TurnContext,
    shell: &Shell,
) -> Option<ResponseItem> {
    let prev = previous?;
    let prev_context = EnvironmentContext::from_turn_context(prev, shell);
    let next_context = EnvironmentContext::from_turn_context(next, shell);
    if prev_context.equals_except_shell(&next_context) {
        return None;
    }

    Some(ResponseItem::from(EnvironmentContext::diff(
        prev, next, shell,
    )))
}

fn build_permissions_update_item(
    previous: Option<&TurnContext>,
    next: &TurnContext,
    exec_policy: &Policy,
) -> Option<ResponseItem> {
    let prev = previous?;
    if prev.sandbox_policy == next.sandbox_policy && prev.approval_policy == next.approval_policy {
        return None;
    }

    Some(
        DeveloperInstructions::from_policy(
            &next.sandbox_policy,
            next.approval_policy,
            exec_policy,
            &next.cwd,
        )
        .into(),
    )
}

fn build_collaboration_mode_update_item(
    previous: Option<&TurnContext>,
    next: &TurnContext,
) -> Option<ResponseItem> {
    let prev = previous?;
    if prev.collaboration_mode != next.collaboration_mode {
        // If the next mode has empty developer instructions, this returns None and we emit no
        // update, so prior collaboration instructions remain in the prompt history.
        Some(DeveloperInstructions::from_collaboration_mode(&next.collaboration_mode)?.into())
    } else {
        None
    }
}

fn build_personality_update_item(
    previous: Option<&TurnContext>,
    next: &TurnContext,
    personality_feature_enabled: bool,
) -> Option<ResponseItem> {
    if !personality_feature_enabled {
        return None;
    }
    let previous = previous?;
    if next.model_info.slug != previous.model_info.slug {
        return None;
    }

    if let Some(personality) = next.personality
        && next.personality != previous.personality
    {
        let model_info = &next.model_info;
        let personality_message = personality_message_for(model_info, personality);
        personality_message
            .map(|message| DeveloperInstructions::personality_spec_message(message).into())
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
    previous: Option<&TurnContext>,
    resumed_model: Option<&str>,
    next: &TurnContext,
) -> Option<ResponseItem> {
    let previous_model =
        resumed_model.or_else(|| previous.map(|prev| prev.model_info.slug.as_str()))?;
    if previous_model == next.model_info.slug {
        return None;
    }

    let model_instructions = next.model_info.get_model_instructions(next.personality);
    if model_instructions.is_empty() {
        return None;
    }

    Some(DeveloperInstructions::model_switch_message(model_instructions).into())
}

pub(crate) fn build_settings_update_items(
    previous: Option<&TurnContext>,
    resumed_model: Option<&str>,
    next: &TurnContext,
    shell: &Shell,
    exec_policy: &Policy,
    personality_feature_enabled: bool,
) -> Vec<ResponseItem> {
    let mut update_items = Vec::new();

    if let Some(env_item) = build_environment_update_item(previous, next, shell) {
        update_items.push(env_item);
    }
    if let Some(permissions_item) = build_permissions_update_item(previous, next, exec_policy) {
        update_items.push(permissions_item);
    }
    if let Some(collaboration_mode_item) = build_collaboration_mode_update_item(previous, next) {
        update_items.push(collaboration_mode_item);
    }
    if let Some(model_instructions_item) =
        build_model_instructions_update_item(previous, resumed_model, next)
    {
        update_items.push(model_instructions_item);
    }
    if let Some(personality_item) =
        build_personality_update_item(previous, next, personality_feature_enabled)
    {
        update_items.push(personality_item);
    }

    update_items
}
