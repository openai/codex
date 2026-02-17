use codex_protocol::config_types::Personality;
use codex_protocol::models::ContentItem;
use codex_protocol::models::DeveloperInstructions;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::protocol::TurnContextItem;

fn build_environment_update_item(
    previous: Option<&TurnContextItem>,
    next: &TurnContextItem,
) -> Option<ResponseItem> {
    let prev = previous?;
    if prev.cwd == next.cwd && prev.network == next.network {
        return None;
    }

    let cwd = (prev.cwd != next.cwd).then_some(&next.cwd);
    let network = if prev.network != next.network {
        next.network.as_ref()
    } else {
        prev.network.as_ref()
    };
    let shell = if next.shell.is_empty() {
        "unknown"
    } else {
        next.shell.as_str()
    };
    let mut lines = vec!["<environment_context>".to_string()];
    if let Some(cwd) = cwd {
        lines.push(format!("  <cwd>{}</cwd>", cwd.to_string_lossy()));
    }
    lines.push(format!("  <shell>{shell}</shell>"));
    if let Some(network) = network {
        lines.push("  <network enabled=\"true\">".to_string());
        for allowed in &network.allowed_domains {
            lines.push(format!("    <allowed>{allowed}</allowed>"));
        }
        for denied in &network.denied_domains {
            lines.push(format!("    <denied>{denied}</denied>"));
        }
        lines.push("  </network>".to_string());
    }
    lines.push("</environment_context>".to_string());

    Some(ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: lines.join("\n"),
        }],
        end_turn: None,
        phase: None,
    })
}

fn build_permissions_update_item(
    previous: Option<&TurnContextItem>,
    next: &TurnContextItem,
) -> Option<ResponseItem> {
    let prev = previous?;
    if prev.sandbox_policy == next.sandbox_policy && prev.approval_policy == next.approval_policy {
        return None;
    }
    if next.permissions_instructions.is_empty() {
        return None;
    }

    Some(DeveloperInstructions::new(next.permissions_instructions.clone()).into())
}

fn build_collaboration_mode_update_item(
    previous: Option<&TurnContextItem>,
    next: &TurnContextItem,
) -> Option<ResponseItem> {
    let prev = previous?;
    if prev.collaboration_mode != next.collaboration_mode {
        // If the next mode has empty developer instructions, this returns None and we emit no
        // update, so prior collaboration instructions remain in the prompt history.
        DeveloperInstructions::from_collaboration_mode(next.collaboration_mode.as_ref()?)
            .map(Into::into)
    } else {
        None
    }
}

fn build_personality_update_item(
    previous: Option<&TurnContextItem>,
    next: &TurnContextItem,
) -> Option<ResponseItem> {
    let previous = previous?;
    if next.model != previous.model {
        return None;
    }

    if next.personality != previous.personality && !next.personality_spec.is_empty() {
        Some(DeveloperInstructions::personality_spec_message(next.personality_spec.clone()).into())
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
    previous: Option<&TurnContextItem>,
    next: &TurnContextItem,
) -> Option<ResponseItem> {
    let previous = previous?;
    if previous.model == next.model {
        return None;
    }

    if next.model_instructions.is_empty() {
        return None;
    }

    Some(DeveloperInstructions::model_switch_message(next.model_instructions.clone()).into())
}

pub(crate) fn build_settings_update_items(
    previous: Option<&TurnContextItem>,
    next: &TurnContextItem,
) -> Vec<ResponseItem> {
    let mut update_items = Vec::new();

    if let Some(env_item) = build_environment_update_item(previous, next) {
        update_items.push(env_item);
    }
    if let Some(permissions_item) = build_permissions_update_item(previous, next) {
        update_items.push(permissions_item);
    }
    if let Some(collaboration_mode_item) = build_collaboration_mode_update_item(previous, next) {
        update_items.push(collaboration_mode_item);
    }
    if let Some(model_instructions_item) = build_model_instructions_update_item(previous, next) {
        update_items.push(model_instructions_item);
    }
    if let Some(personality_item) = build_personality_update_item(previous, next) {
        update_items.push(personality_item);
    }

    update_items
}
