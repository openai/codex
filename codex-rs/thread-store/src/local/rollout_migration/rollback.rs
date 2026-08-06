//! Defines which persisted history entries count as rollbackable user turns.
//!
//! Legacy `ThreadRolledBack { num_turns }` does not mean “drop the last N JSONL records.” A turn
//! can include contextual user/developer messages, paired `ResponseItem` + `UserMessage` records,
//! inter-agent communication, or compaction replacement history.
//!
//! This module mirrors the legacy cold-resume rollback boundary rules for persisted rollout shapes.
//! It stays local to migration because core's live predicate also knows about dynamically
//! registered contextual fragments that thread-store should not depend on.

use codex_protocol::items::parse_hook_prompt_fragment;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::InterAgentCommunication;

/// Match the rollback boundaries used by legacy history reconstruction without
/// treating every persisted lifecycle as a user turn. This intentionally stays
/// local to the frozen migration adapter: the full core predicate also knows
/// about dynamically registered contextual fragments that thread-store cannot
/// depend on.
pub(super) fn counts_as_boundary(response: &ResponseItem) -> bool {
    if matches!(response, ResponseItem::AgentMessage { .. }) {
        return true;
    }
    let ResponseItem::Message { role, content, .. } = response else {
        return false;
    };
    (role == "user" && !is_known_contextual_user_message_content(content))
        || (role == "assistant" && InterAgentCommunication::is_message_content(content))
}

pub(super) fn is_pre_turn_context_update(response: &ResponseItem) -> bool {
    let ResponseItem::Message { role, content, .. } = response else {
        return false;
    };
    (role == "user" && is_known_contextual_user_message_content(content))
        || (role == "developer" && is_known_contextual_developer_message_content(content))
}

/// Apply the same response-history cut used by legacy cold resume to a persisted
/// compaction checkpoint. The migration adapter uses a frozen contextual-fragment
/// matcher because thread-store cannot depend on core's runtime fragment registry.
pub(super) fn drop_last_n_user_turns(history: &mut Vec<ResponseItem>, num_turns: u32) {
    if num_turns == 0 {
        return;
    }
    let user_positions = history
        .iter()
        .enumerate()
        .filter_map(|(index, item)| counts_as_boundary(item).then_some(index))
        .collect::<Vec<_>>();
    let Some(&first_turn_index) = user_positions.first() else {
        return;
    };
    let count = usize::try_from(num_turns).unwrap_or(usize::MAX);
    let mut cut_index = if count >= user_positions.len() {
        first_turn_index
    } else {
        user_positions[user_positions.len() - count]
    };
    while cut_index > first_turn_index && is_pre_turn_context_update(&history[cut_index - 1]) {
        cut_index -= 1;
    }
    history.truncate(cut_index);
}

fn is_known_contextual_user_message_content(content: &[ContentItem]) -> bool {
    content.iter().any(|item| {
        let ContentItem::InputText { text } = item else {
            return false;
        };
        is_known_contextual_user_text(text)
    })
}

fn is_known_contextual_developer_message_content(content: &[ContentItem]) -> bool {
    content.iter().any(|item| {
        let ContentItem::InputText { text } = item else {
            return false;
        };
        let text = text.trim_start();
        [
            "<permissions instructions>",
            "<model_switch>",
            "<apps_instructions>",
            "<collaboration_mode>",
            "<multi_agent_mode>",
            "<environments_instructions>",
            "<git_attribution>",
            "<plugins_instructions>",
            "<realtime_conversation>",
            "<skills_instructions>",
            "<tools>",
            "<personality_spec>",
            "<token_budget>",
            "<context_window>",
            "<context_window_guidance>",
            "<rollout_budget>",
        ]
        .iter()
        .any(|prefix| {
            text.get(..prefix.len())
                .is_some_and(|candidate| candidate.eq_ignore_ascii_case(prefix))
        })
    })
}

// Keep this frozen alongside the legacy migration adapter. Core's live predicate also knows
// about dynamically registered fragments, but legacy rollouts only need these persisted shapes.
fn is_known_contextual_user_text(text: &str) -> bool {
    let text = text.trim();
    parse_hook_prompt_fragment(text).is_some()
        || [
            ("# AGENTS.md instructions", "</INSTRUCTIONS>"),
            ("<environment_context>", "</environment_context>"),
            ("<skill>", "</skill>"),
            ("<user_shell_command>", "</user_shell_command>"),
            ("<turn_aborted>", "</turn_aborted>"),
            ("<subagent_notification>", "</subagent_notification>"),
            ("<recommended_plugins>", "</recommended_plugins>"),
            ("<goal_context>", "</goal_context>"),
        ]
        .iter()
        .any(|(start, end)| text.starts_with(start) && text.ends_with(end))
        || (text.starts_with("<external_")
            && text
                .split_once('>')
                .and_then(|(start, _)| start.strip_prefix("<external_"))
                .is_some_and(|key| text.ends_with(&format!("</external_{key}>"))))
        || (text.starts_with("<codex_internal_context")
            && text.ends_with("</codex_internal_context>"))
        || text.starts_with(
            "Warning: The maximum number of unified exec processes you can keep open is",
        )
        || (text.starts_with("Warning: apply_patch was requested via ")
            && text.ends_with("Use the apply_patch tool instead of exec_command."))
        || text.starts_with(
            "Warning: Your account was flagged for potentially high-risk cyber activity",
        )
}
