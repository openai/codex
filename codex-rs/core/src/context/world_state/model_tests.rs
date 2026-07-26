use super::*;
use pretty_assertions::assert_eq;

#[test]
fn model_change_renders_when_persisted_or_inferred_from_previous_turn() {
    let state = ModelInstructionsState::new("gpt-new", Some("gpt-old"), "instructions".into());
    let previous = "gpt-old".to_string();

    for previous in [
        PreviousSectionState::Known(&previous),
        PreviousSectionState::Unknown,
        PreviousSectionState::Absent,
    ] {
        assert_eq!(
            state
                .render_diff(previous)
                .expect("model change should render")
                .markers(),
            ModelSwitchInstructions::type_markers()
        );
    }
}

#[test]
fn unchanged_model_does_not_render() {
    let state = ModelInstructionsState::new("gpt-test", Some("gpt-test"), "instructions".into());
    let previous = "gpt-test".to_string();

    assert!(
        state
            .render_diff(PreviousSectionState::Known(&previous))
            .is_none()
    );
    assert!(state.render_diff(PreviousSectionState::Absent).is_none());
}
