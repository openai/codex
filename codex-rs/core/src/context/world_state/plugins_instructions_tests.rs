use super::*;
use crate::context::ContextualUserFragment;
use crate::context::world_state::PreviousSectionState;
use crate::context::world_state::test_support::render_section_cases;
use codex_protocol::models::ResponseItem;
use pretty_assertions::assert_eq;

#[test]
fn snapshots() {
    use PreviousSectionState::Absent;
    use PreviousSectionState::Known;
    use PreviousSectionState::Unknown;

    let unavailable = PluginsInstructionsState::new(/*available*/ false);
    let available = PluginsInstructionsState::new(/*available*/ true);

    insta::assert_snapshot!(render_section_cases(&[
        (Absent, Absent),
        (Absent, Known(&unavailable)),
        (Absent, Known(&available)),
        (Known(&unavailable), Known(&available)),
        (Known(&available), Known(&available)),
        (Known(&available), Known(&unavailable)),
        (Unknown, Known(&unavailable)),
        (Unknown, Known(&available)),
    ]));
}

#[test]
fn legacy_guidance_is_not_injected_again() {
    let mut world_state = super::super::WorldState::default();
    world_state.add_section(PluginsInstructionsState::new(/*available*/ true));
    let legacy: ResponseItem = ContextualUserFragment::into(AvailablePluginsInstructions);

    assert!(
        world_state
            .render_history_diff(/*previous*/ None, &[legacy])
            .is_empty()
    );
}

#[test]
fn persisted_guidance_is_restored_only_when_missing_from_history() {
    let mut world_state = super::super::WorldState::default();
    world_state.add_section(PluginsInstructionsState::new(/*available*/ true));
    let snapshot = world_state.snapshot();
    let retained: ResponseItem = ContextualUserFragment::into(AvailablePluginsInstructions);

    assert_eq!(
        world_state.render_history_diff(Some(&snapshot), &[]).len(),
        1
    );
    assert!(
        world_state
            .render_history_diff(Some(&snapshot), &[retained])
            .is_empty()
    );
}
