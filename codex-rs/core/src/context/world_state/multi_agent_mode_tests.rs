use super::super::test_support::render_section_cases;
use super::*;
use crate::context::MultiAgentRoleInstructions;
use crate::context::world_state::WorldState;
use codex_protocol::models::ResponseItem;
use codex_utils_output_truncation::approx_token_count;

fn state(mode: Option<MultiAgentMode>) -> MultiAgentModeState {
    MultiAgentModeState::new(mode)
}

#[test]
fn snapshots() {
    use PreviousSectionState::Absent;
    use PreviousSectionState::Known;
    use PreviousSectionState::Unknown;

    let inactive = state(/*mode*/ None);
    let explicit = state(Some(MultiAgentMode::ExplicitRequestOnly));
    let proactive = state(Some(MultiAgentMode::Proactive));
    let custom = state(Some(MultiAgentMode::Custom(
        "use a custom policy".to_string(),
    )));
    let empty = state(Some(MultiAgentMode::Custom(String::new())));

    insta::assert_snapshot!(render_section_cases(&[
        (Absent, Absent),
        (Absent, Known(&inactive)),
        (Absent, Known(&explicit)),
        (Known(&explicit), Known(&explicit)),
        (Known(&explicit), Known(&proactive)),
        (Known(&proactive), Known(&inactive)),
        (Known(&explicit), Known(&inactive)),
        (Known(&explicit), Known(&custom)),
        (Known(&custom), Known(&empty)),
        (Unknown, Known(&explicit)),
        (Unknown, Known(&inactive)),
    ]));
}

#[test]
fn persisted_mode_is_restored_only_when_missing_from_history() {
    let state = state(Some(MultiAgentMode::ExplicitRequestOnly));
    let retained: ResponseItem = ContextualUserFragment::into(
        MultiAgentModeInstructions::from_mode(MultiAgentMode::ExplicitRequestOnly)
            .expect("explicit mode should render"),
    );
    let mut world_state = WorldState::default();
    world_state.add_section(state);
    let snapshot = world_state.snapshot();

    assert_eq!(
        world_state
            .render_history_diff(/*previous*/ None, std::slice::from_ref(&retained))
            .len(),
        1,
    );
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

/// Active mode instructions must follow a newly migrated multi-agent usage hint.
#[test]
fn unchanged_mode_is_reemitted_after_usage_hint_migration() {
    let previous = state(Some(MultiAgentMode::Proactive));
    let current = MultiAgentModeState::new(Some(MultiAgentMode::Proactive)).with_usage_hint(
        &MultiAgentUsageHintState::new(MultiAgentRoleInstructions::unmarked(
            "Current usage instructions.",
        )),
    );

    let instructions = current
        .render_diff(PreviousSectionState::Known(&previous))
        .expect("unchanged mode should follow migrated usage instructions");

    assert_eq!(
        instructions.render(),
        MultiAgentModeInstructions::from_mode(MultiAgentMode::Proactive)
            .expect("proactive mode should render")
            .render()
    );
}

#[test]
fn catalog_role_updates_remain_separate_from_active_mode() {
    let previous_hint =
        MultiAgentUsageHintState::new(MultiAgentRoleInstructions::catalog("Previous role."));
    let previous_mode =
        MultiAgentModeState::new(Some(MultiAgentMode::Proactive)).with_usage_hint(&previous_hint);
    let mut previous = WorldState::default();
    previous.add_section(previous_hint);
    previous.add_section(previous_mode);

    let current_role = MultiAgentRoleInstructions::catalog("Current role.");
    let current_hint = MultiAgentUsageHintState::new(current_role.clone());
    let current_mode =
        MultiAgentModeState::new(Some(MultiAgentMode::Proactive)).with_usage_hint(&current_hint);
    let mut current = WorldState::default();
    current.add_section(current_hint);
    current.add_section(current_mode);

    let updates = crate::context_manager::updates::merge_contextual_fragments(
        current.render_diff(&previous.snapshot()),
    );
    let expected_mode = MultiAgentModeInstructions::from_mode(MultiAgentMode::Proactive)
        .expect("proactive mode should render");
    assert_eq!(
        updates,
        vec![
            ContextualUserFragment::into(current_role),
            ContextualUserFragment::into(expected_mode),
        ],
    );
}

#[test]
fn custom_mode_is_bounded_before_snapshot_and_rendering() {
    let state = state(Some(MultiAgentMode::Custom("custom mode ".repeat(1_000))));
    let Some(MultiAgentMode::Custom(snapshot_mode)) = state.snapshot().mode else {
        panic!("expected custom multi-agent mode")
    };
    assert!(approx_token_count(&snapshot_mode) < 1_000);

    let rendered = state
        .render_diff(PreviousSectionState::Absent)
        .expect("custom mode should render")
        .render();
    assert!(approx_token_count(&rendered) < 1_000);
}
