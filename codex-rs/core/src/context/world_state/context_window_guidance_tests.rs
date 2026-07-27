use super::ContextWindowGuidanceState;
use crate::context::ContextWindowGuidance;
use crate::context::ContextualUserFragment;
use crate::context::world_state::WorldState;
use codex_protocol::models::ResponseItem;
use pretty_assertions::assert_eq;

#[test]
fn changed_guidance_is_rendered_once() {
    let mut original = WorldState::default();
    original.add_section(ContextWindowGuidanceState::new("original guidance"));
    let snapshot = original.snapshot();

    let mut unchanged = WorldState::default();
    unchanged.add_section(ContextWindowGuidanceState::new("original guidance"));
    assert!(unchanged.render_diff(&snapshot).is_empty());

    let mut refreshed = WorldState::default();
    refreshed.add_section(ContextWindowGuidanceState::new("refreshed guidance"));
    let fragments = refreshed.render_diff(&snapshot);

    assert_eq!(fragments.len(), 1);
    assert_eq!(
        fragments[0].render(),
        ContextWindowGuidance::new("refreshed guidance").render()
    );
}

#[test]
fn legacy_guidance_is_reconciled_once() {
    let mut state = WorldState::default();
    state.add_section(ContextWindowGuidanceState::new("current guidance"));
    let retained: ResponseItem =
        ContextualUserFragment::into(ContextWindowGuidance::new("previous guidance"));

    let fragments =
        state.render_history_diff(/*previous*/ None, std::slice::from_ref(&retained));
    assert_eq!(fragments.len(), 1);
    assert_eq!(
        fragments[0].render(),
        ContextWindowGuidance::new("current guidance").render()
    );

    let snapshot = state.snapshot();
    let reconciled: ResponseItem =
        ContextualUserFragment::into(ContextWindowGuidance::new("current guidance"));
    assert!(
        state
            .render_history_diff(Some(&snapshot), &[retained, reconciled])
            .is_empty()
    );
}
