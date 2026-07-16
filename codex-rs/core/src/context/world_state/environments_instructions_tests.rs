use super::*;
use crate::context::ContextualUserFragment;
use crate::context::world_state::test_support::render_section_cases;
use codex_protocol::models::ResponseItem;
use pretty_assertions::assert_eq;

#[test]
fn snapshots() {
    use PreviousSectionState::Absent;
    use PreviousSectionState::Known;
    use PreviousSectionState::Unknown;

    let disabled = EnvironmentsInstructionsState::new(/*enabled*/ false);
    let enabled = EnvironmentsInstructionsState::new(/*enabled*/ true);

    insta::assert_snapshot!(render_section_cases(&[
        (Absent, Absent),
        (Absent, Known(&disabled)),
        (Absent, Known(&enabled)),
        (Known(&disabled), Known(&enabled)),
        (Known(&enabled), Known(&enabled)),
        (Known(&enabled), Known(&disabled)),
        (Unknown, Known(&disabled)),
        (Unknown, Known(&enabled)),
    ]));
}

#[test]
fn legacy_guidance_is_not_injected_again() {
    let mut world_state = super::super::WorldState::default();
    world_state.add_section(EnvironmentsInstructionsState::new(/*enabled*/ true));
    let legacy: ResponseItem = ContextualUserFragment::into(EnvironmentsInstructions);

    assert!(
        world_state
            .render_history_diff(/*previous*/ None, &[legacy])
            .is_empty()
    );
}

#[test]
fn persisted_guidance_is_restored_only_when_missing_from_history() {
    let mut world_state = super::super::WorldState::default();
    world_state.add_section(EnvironmentsInstructionsState::new(/*enabled*/ true));
    let snapshot = world_state.snapshot();
    let retained: ResponseItem = ContextualUserFragment::into(EnvironmentsInstructions);

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
