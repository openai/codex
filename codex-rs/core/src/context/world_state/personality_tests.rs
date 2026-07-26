use super::*;
use crate::context::world_state::WorldState;
use pretty_assertions::assert_eq;

fn state(
    model: &str,
    personality: Option<Personality>,
    previous: Option<(&str, Option<Personality>)>,
    personality_is_baked: bool,
) -> PersonalityState {
    PersonalityState::new(
        model,
        personality,
        previous.map(|(model, _)| model),
        previous.and_then(|(_, personality)| personality),
        personality.map(|personality| format!("instructions for {personality:?}")),
        personality_is_baked,
    )
}

#[test]
fn initial_personality_renders_only_when_missing_from_base_instructions() {
    let separate = state(
        "gpt-test",
        Some(Personality::Friendly),
        /*previous*/ None,
        /*personality_is_baked*/ false,
    );
    let baked = state(
        "gpt-test",
        Some(Personality::Friendly),
        /*previous*/ None,
        /*personality_is_baked*/ true,
    );

    assert_eq!(
        separate
            .render_diff(PreviousSectionState::Absent)
            .expect("separate personality should render")
            .markers(),
        PersonalitySpecInstructions::type_markers()
    );
    assert!(baked.render_diff(PreviousSectionState::Absent).is_none());
}

#[test]
fn personality_changes_render_without_repeating_model_changes() {
    let previous = PersonalitySnapshot {
        model: "gpt-test".to_string(),
        personality: Some(Personality::Friendly),
    };
    let changed = state(
        "gpt-test",
        Some(Personality::Pragmatic),
        Some(("gpt-test", Some(Personality::Friendly))),
        /*personality_is_baked*/ true,
    );
    let model_changed = state(
        "gpt-next",
        Some(Personality::Pragmatic),
        Some(("gpt-test", Some(Personality::Friendly))),
        /*personality_is_baked*/ true,
    );

    assert_eq!(
        changed
            .render_diff(PreviousSectionState::Known(&previous))
            .expect("changed personality should render")
            .markers(),
        PersonalitySpecInstructions::type_markers()
    );
    assert!(
        model_changed
            .render_diff(PreviousSectionState::Known(&previous))
            .is_none()
    );
    assert!(
        model_changed
            .render_diff(PreviousSectionState::Unknown)
            .is_none()
    );
    assert!(
        model_changed
            .render_diff(PreviousSectionState::Absent)
            .is_none()
    );
}

#[test]
fn persisted_personality_does_not_require_a_retained_update() {
    let state = state(
        "gpt-test",
        Some(Personality::Friendly),
        Some(("gpt-test", Some(Personality::Friendly))),
        /*personality_is_baked*/ false,
    );
    let mut world_state = WorldState::default();
    world_state.add_section(state);
    let snapshot = world_state.snapshot();

    assert!(
        world_state
            .render_history_diff(Some(&snapshot), &[])
            .is_empty()
    );
}
