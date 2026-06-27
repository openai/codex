use super::*;
use crate::context::ContextualUserFragment;
use codex_protocol::models::ResponseItem;
use pretty_assertions::assert_eq;

fn render(
    state: PluginsInstructionsState,
    previous: PreviousSectionState<'_, bool>,
) -> Vec<String> {
    state
        .render_diff(previous)
        .into_iter()
        .map(|fragment| fragment.render())
        .collect()
}

#[test]
fn renders_only_when_plugins_become_available() {
    let unavailable = PluginsInstructionsState::new(/*available*/ false);
    let available = PluginsInstructionsState::new(/*available*/ true);
    let false_snapshot = false;
    let true_snapshot = true;

    assert_eq!(
        render(unavailable, PreviousSectionState::Absent),
        Vec::<String>::new()
    );
    assert_eq!(render(available, PreviousSectionState::Absent).len(), 1);
    assert_eq!(
        render(available, PreviousSectionState::Known(&false_snapshot)).len(),
        1
    );
    assert_eq!(
        render(available, PreviousSectionState::Known(&true_snapshot)),
        Vec::<String>::new()
    );
    assert_eq!(
        render(unavailable, PreviousSectionState::Known(&true_snapshot)),
        Vec::<String>::new()
    );
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
