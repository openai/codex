use super::super::PreviousSectionState;
use super::super::test_support::render_section_cases;
use super::*;
use crate::context::world_state::WorldState;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::models::ResponseItem;

#[test]
fn snapshots() {
    use PreviousSectionState::Absent;
    use PreviousSectionState::Known;
    use PreviousSectionState::Unknown;

    let default = collaboration_mode_state(ModeKind::Default, "pair with the user");
    let old_default = collaboration_mode_state(ModeKind::Default, "old instructions");
    let new_default = collaboration_mode_state(ModeKind::Default, "new instructions");
    let plan = collaboration_mode_state(ModeKind::Plan, "make a plan");

    insta::assert_snapshot!(render_section_cases(&[
        (Absent, Absent),
        (Absent, Known(&default)),
        (Known(&default), Known(&default)),
        (Known(&old_default), Known(&new_default)),
        (Known(&default), Known(&plan)),
        (Unknown, Known(&default)),
    ]));
}

#[test]
fn persisted_instructions_are_restored_only_when_missing_from_history() {
    let state = collaboration_mode_state(ModeKind::Default, "pair with the user");
    let retained: ResponseItem = ContextualUserFragment::into(CollaborationModeInstructions {
        instructions: state.instructions.clone(),
    });
    let mut world_state = WorldState::default();
    world_state.add_section(state);
    let snapshot = world_state.snapshot();

    assert!(
        world_state
            .render_history_diff(/*previous*/ None, std::slice::from_ref(&retained))
            .is_empty()
    );
    assert_eq!(
        world_state.render_history_diff(Some(&snapshot), &[]).len(),
        1,
    );
    assert!(
        world_state
            .render_history_diff(Some(&snapshot), &[retained])
            .is_empty()
    );
}

fn collaboration_mode_state(mode: ModeKind, instructions: &str) -> CollaborationModeState {
    CollaborationModeState::from_collaboration_mode(&CollaborationMode {
        mode,
        settings: Settings {
            model: "test-model".to_string(),
            reasoning_effort: None,
            developer_instructions: Some(instructions.to_string()),
        },
    })
    .expect("test collaboration mode should have instructions")
}
