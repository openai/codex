//! Covers effort selection and persistent-context transitions.

use super::*;
use crate::context::world_state::WorldState;
use pretty_assertions::assert_eq;

#[test]
fn persistent_instructions_follow_effort_and_catalog_updates_without_duplicates() {
    let mut history = Vec::new();
    let mut previous = None;
    let persistent = Some(ReasoningEffort::Persistent);
    let medium = Some(ReasoningEffort::Medium);
    let replacement = format!("{REPLACEMENT_NOTICE}\n\nupdated instructions");

    for (effort, instructions, expected) in [
        (None, "", None),
        (persistent.clone(), "instructions", Some("instructions")),
        (persistent.clone(), "instructions", None),
        (
            persistent.clone(),
            "updated instructions",
            Some(replacement.as_str()),
        ),
        (persistent.clone(), "", Some(REMOVAL_NOTICE)),
        (persistent.clone(), "", None),
        (persistent, "instructions", Some("instructions")),
        (medium.clone(), "", Some(REMOVAL_NOTICE)),
        (medium, "", None),
    ] {
        let mut world_state = WorldState::default();
        world_state.add_section(PersistentModeState::new(
            effort.as_ref(),
            instructions,
            /*send_user_message_async_available*/ false,
        ));
        let updates = world_state
            .render_history_diff(previous.as_ref(), &history)
            .into_iter()
            .map(ContextualUserFragment::into_boxed_response_item)
            .collect::<Vec<_>>();
        assert_eq!(
            updates,
            expected
                .map(|instructions| {
                    ContextualUserFragment::into(PersistentModeState {
                        instructions: instructions.to_string(),
                    })
                })
                .into_iter()
                .collect::<Vec<_>>()
        );
        history.extend(updates);
        previous = Some(world_state.snapshot());
    }
}

#[test]
fn retained_persistent_instructions_are_replaced_or_retired_without_a_snapshot() {
    let retained = ContextualUserFragment::into(PersistentModeState {
        instructions: "previous instructions".to_string(),
    });
    for (effort, expected) in [
        (
            ReasoningEffort::Persistent,
            format!("{REPLACEMENT_NOTICE}\n\ncurrent instructions"),
        ),
        (ReasoningEffort::Medium, REMOVAL_NOTICE.to_string()),
    ] {
        let mut world_state = WorldState::default();
        world_state.add_section(PersistentModeState::new(
            Some(&effort),
            "current instructions",
            /*send_user_message_async_available*/ false,
        ));
        assert_eq!(
            world_state
                .render_history_diff(/*previous*/ None, std::slice::from_ref(&retained))
                .into_iter()
                .map(ContextualUserFragment::into_boxed_response_item)
                .collect::<Vec<_>>(),
            vec![ContextualUserFragment::into(PersistentModeState {
                instructions: expected,
            })]
        );
    }
}
