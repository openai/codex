use super::*;
use crate::context::world_state::WorldState;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn renders_full_state_and_omits_unchanged_state() {
    let loaded = LoadedAgentsMd::from_text_for_testing("use the project formatter");
    let mut state = WorldState::default();
    state.add_section(AgentsMdState::new(Some(&loaded)));

    assert_eq!(
        vec![user_message(
            "# AGENTS.md instructions\n\n<INSTRUCTIONS>\nuse the project formatter\n</INSTRUCTIONS>",
        )],
        render_fragments(state.render_full()),
    );
    assert_eq!(
        Vec::<ResponseItem>::new(),
        render_fragments(state.render_diff(&state.snapshot()))
    );
    assert_eq!(
        state.snapshot().into_value(),
        json!({"agents_md": {"text": "use the project formatter"}}),
    );
}

#[test]
fn changed_and_removed_state_supersedes_previous_instructions() {
    let previous_loaded = LoadedAgentsMd::from_text_for_testing("old instructions");
    let mut previous = WorldState::default();
    previous.add_section(AgentsMdState::new(Some(&previous_loaded)));

    let current_loaded = LoadedAgentsMd::from_text_for_testing("new instructions");
    let mut current = WorldState::default();
    current.add_section(AgentsMdState::new(Some(&current_loaded)));
    assert_eq!(
        vec![user_message(
            "# AGENTS.md instructions\n\n<INSTRUCTIONS>\nThese AGENTS.md instructions replace all previously provided AGENTS.md instructions.\n\nnew instructions\n</INSTRUCTIONS>",
        )],
        render_fragments(current.render_diff(&previous.snapshot())),
    );

    let mut removed = WorldState::default();
    removed.add_section(AgentsMdState::default());
    assert_eq!(
        vec![user_message(
            "# AGENTS.md instructions\n\n<INSTRUCTIONS>\nThe previously provided AGENTS.md instructions no longer apply.\n</INSTRUCTIONS>",
        )],
        render_fragments(removed.render_diff(&current.snapshot())),
    );
}

#[test]
fn unknown_previous_state_is_explicitly_superseded() {
    let loaded = LoadedAgentsMd::from_text_for_testing("current instructions");
    let current = AgentsMdState::new(Some(&loaded));
    assert_eq!(
        vec![user_message(
            "# AGENTS.md instructions\n\n<INSTRUCTIONS>\nThese AGENTS.md instructions replace all previously provided AGENTS.md instructions.\n\ncurrent instructions\n</INSTRUCTIONS>",
        )],
        render_fragments(vec![
            WorldStateSection::render_diff(&current, PreviousSectionState::Unknown)
                .expect("unknown state should be replaced"),
        ]),
    );

    assert_eq!(
        vec![user_message(
            "# AGENTS.md instructions\n\n<INSTRUCTIONS>\nThe previously provided AGENTS.md instructions no longer apply.\n</INSTRUCTIONS>",
        )],
        render_fragments(vec![
            WorldStateSection::render_diff(
                &AgentsMdState::default(),
                PreviousSectionState::Unknown,
            )
            .expect("unknown state should be removed"),
        ]),
    );
}

fn render_fragments(fragments: Vec<Box<dyn ContextualUserFragment>>) -> Vec<ResponseItem> {
    fragments
        .into_iter()
        .map(ContextualUserFragment::into_boxed_response_item)
        .collect()
}

fn user_message(text: &str) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: text.to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    }
}
