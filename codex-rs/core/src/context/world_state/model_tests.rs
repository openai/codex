use super::*;
use crate::context::world_state::test_support::FragmentSectionTestExt as _;
use pretty_assertions::assert_eq;

#[test]
fn model_change_renders_when_persisted_or_inferred_from_previous_turn() {
    let state = ModelInstructionsState::new("gpt-new", Some("gpt-old"), "instructions".into());
    let previous = ModelInstructionsSnapshot::ModelOnly("gpt-old".to_string());
    let previous_incremental = serde_json::from_value(serde_json::json!({
        "model": "gpt-new",
        "instructions": WorldStateHash::from_fragment(&BaseInstructionsFragment("instructions".into())),
    }))
    .unwrap();

    for previous in [
        PreviousSectionState::Known(&previous),
        PreviousSectionState::Known(&previous_incremental),
        PreviousSectionState::Unknown,
        PreviousSectionState::Absent,
    ] {
        assert_eq!(
            state
                .render_fragment_diff(previous)
                .1
                .expect("model change should render")
                .markers(),
            ModelSwitchInstructions::type_markers()
        );
    }
}

#[test]
fn unchanged_model_does_not_render() {
    let state = ModelInstructionsState::new("gpt-test", Some("gpt-test"), "instructions".into());
    let previous = ModelInstructionsSnapshot::ModelOnly("gpt-test".to_string());

    let (snapshot, fragment) = state.render_fragment_diff(PreviousSectionState::Known(&previous));
    assert_eq!(
        serde_json::to_value(snapshot).unwrap(),
        serde_json::json!("gpt-test")
    );
    assert!(fragment.is_none());
    assert!(
        state
            .render_fragment_diff(PreviousSectionState::Absent)
            .1
            .is_none()
    );
}

#[test]
fn legacy_model_snapshot_keeps_its_window_until_model_change_or_compaction() {
    let legacy: ModelInstructionsSnapshot = serde_json::from_str("\"gpt-old\"").unwrap();
    let same_model = ModelInstructionsState::for_incremental_tools(
        "gpt-old",
        "new prompt".into(),
        /*suppress_model_change_announcements*/ false,
    );
    let (snapshot, updates) = same_model.render_diff(PreviousSectionState::Known(&legacy));
    assert!(snapshot.is_none());
    assert!(updates.is_empty());

    let new_model = ModelInstructionsState::for_incremental_tools(
        "gpt-new",
        "new prompt".into(),
        /*suppress_model_change_announcements*/ false,
    );
    let (snapshot, updates) = new_model.render_diff(PreviousSectionState::Known(&legacy));
    assert!(snapshot.is_some());
    assert_eq!(updates.len(), 1);

    for previous in [PreviousSectionState::Absent, PreviousSectionState::Unknown] {
        let (snapshot, updates) = same_model.render_diff(previous);
        assert_eq!(
            crate::context_manager::updates::merge_world_state_updates(updates),
            vec![ContextualUserFragment::into(BaseInstructionsFragment(
                "new prompt".into()
            ))]
        );
        let snapshot = snapshot.unwrap();
        assert!(
            same_model
                .render_diff(PreviousSectionState::Known(&snapshot))
                .1
                .is_empty()
        );
    }
}

#[test]
fn suppressed_model_changes_advance_snapshot_without_repeated_updates() {
    let initial = ModelInstructionsState::for_incremental_tools(
        "gpt-old",
        "instructions".into(),
        /*suppress_model_change_announcements*/ true,
    );
    let (snapshot, _) = initial.render_diff(PreviousSectionState::Absent);
    let previous = snapshot.unwrap();
    let switched = ModelInstructionsState::for_incremental_tools(
        "gpt-new",
        "instructions".into(),
        /*suppress_model_change_announcements*/ true,
    );
    let (snapshot, updates) = switched.render_diff(PreviousSectionState::Known(&previous));
    assert!(updates.is_empty());
    let snapshot = snapshot.unwrap();
    assert_eq!(snapshot, switched.snapshot);
    let (unchanged, updates) = switched.render_diff(PreviousSectionState::Known(&snapshot));
    assert!(unchanged.is_none());
    assert!(updates.is_empty());
}
