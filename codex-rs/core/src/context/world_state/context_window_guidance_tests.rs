use super::ContextWindowGuidanceState;
use super::REMOVAL_NOTICE;
use crate::context::ContextWindowGuidance;
use crate::context::ContextualUserFragment;
use crate::context::world_state::WorldState;
use crate::context::world_state::WorldStateSnapshot;
use codex_protocol::models::ResponseItem;
use pretty_assertions::assert_eq;
use serde_json::json;
use test_case::test_case;

#[test_case(None, None, None; "absent guidance stays silent")]
#[test_case(None, Some("guidance"), Some("guidance"); "initial guidance")]
#[test_case(Some("guidance"), Some("guidance"), None; "unchanged guidance")]
#[test_case(
    Some("original guidance"),
    Some("refreshed guidance"),
    Some("This context-window guidance replaces all previously provided context-window guidance.\n\nrefreshed guidance");
    "changed guidance"
)]
#[test_case(Some("guidance"), None, Some(REMOVAL_NOTICE); "removed guidance")]
#[test_case(Some("guidance"), Some("  "), Some(REMOVAL_NOTICE); "blank guidance")]
fn guidance_transitions_render_once(
    previous: Option<&str>,
    current: Option<&str>,
    expected: Option<&str>,
) {
    let mut original = WorldState::default();
    original.add_section(ContextWindowGuidanceState::new(previous));

    let mut refreshed = WorldState::default();
    refreshed.add_section(ContextWindowGuidanceState::new(current));
    let mut history: Vec<ResponseItem> = previous
        .map(|message| ContextualUserFragment::into(ContextWindowGuidance::new(message)))
        .into_iter()
        .collect();
    let (next_snapshot, fragments) =
        refreshed.render_history_fragment_diff(Some(&original.render_full().0), &history);
    assert_eq!(
        fragments
            .iter()
            .map(|fragment| fragment.render())
            .collect::<Vec<_>>(),
        expected
            .map(|message| ContextWindowGuidance::new(message).render())
            .into_iter()
            .collect::<Vec<_>>()
    );

    // Empty guidance must survive persistence as a known state, not a deleted section.
    let snapshot: WorldStateSnapshot =
        serde_json::from_value(serde_json::to_value(next_snapshot).unwrap()).unwrap();
    history.extend(
        fragments
            .into_iter()
            .map(ContextualUserFragment::into_boxed_response_item),
    );
    let (repeated_snapshot, repeated_fragments) =
        refreshed.render_history_fragment_diff(Some(&snapshot), &history);
    assert_eq!(repeated_snapshot, snapshot);
    assert!(repeated_fragments.is_empty());
}

#[test_case(
    false,
    Some("current guidance"),
    Some("This context-window guidance replaces all previously provided context-window guidance.\n\ncurrent guidance");
    "legacy history is replaced"
)]
#[test_case(false, None, Some(REMOVAL_NOTICE); "legacy history is cleared")]
#[test_case(true, None, Some(REMOVAL_NOTICE); "legacy string snapshot is cleared")]
#[test_case(true, Some("previous guidance"), None; "existing string snapshot is unchanged")]
fn legacy_guidance_is_reconciled_once(
    has_snapshot: bool,
    current: Option<&str>,
    expected: Option<&str>,
) {
    let mut state = WorldState::default();
    state.add_section(ContextWindowGuidanceState::new(current));
    let snapshot: Option<WorldStateSnapshot> = has_snapshot.then(|| {
        serde_json::from_value(json!({"context_window_guidance": "previous guidance"})).unwrap()
    });
    let retained: ResponseItem =
        ContextualUserFragment::into(ContextWindowGuidance::new("previous guidance"));
    let fragments = state
        .render_history_fragment_diff(snapshot.as_ref(), std::slice::from_ref(&retained))
        .1;
    assert_eq!(
        fragments
            .iter()
            .map(|fragment| fragment.render())
            .collect::<Vec<_>>(),
        expected
            .map(|message| ContextWindowGuidance::new(message).render())
            .into_iter()
            .collect::<Vec<_>>()
    );

    let mut history = vec![retained];
    if let Some(message) = expected {
        history.push(ContextualUserFragment::into(ContextWindowGuidance::new(
            message,
        )));
    }
    assert!(
        state
            .render_history_fragment_diff(Some(&state.render_full().0), &history)
            .1
            .is_empty()
    );
}
