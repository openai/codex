use super::*;
use codex_execpolicy::Decision;
use pretty_assertions::assert_eq;

#[test]
fn renders_only_newly_approved_prefixes() {
    use PreviousSectionState::Known;

    let mut exec_policy = Policy::empty();
    exec_policy
        .add_prefix_rule(&["git".to_string(), "pull".to_string()], Decision::Allow)
        .expect("test prefix should be valid");
    let with_existing_prefix = CompactPermissionsState::new(&exec_policy);
    exec_policy
        .add_prefix_rule(&["cargo".to_string(), "test".to_string()], Decision::Allow)
        .expect("test prefix should be valid");
    let with_new_prefix = CompactPermissionsState::new(&exec_policy);
    let existing_snapshot = with_existing_prefix.snapshot();
    let current_snapshot = with_new_prefix.snapshot();

    assert_eq!(
        with_new_prefix
            .render_diff(Known(&existing_snapshot))
            .map(|fragment| fragment.render()),
        Some("Approved command prefix saved:\n- [\"cargo\", \"test\"]".to_string())
    );
    assert!(
        with_new_prefix
            .render_diff(Known(&current_snapshot))
            .is_none()
    );
}

#[test]
fn does_not_duplicate_a_retained_legacy_update() {
    use PreviousSectionState::Unknown;

    let mut exec_policy = Policy::empty();
    exec_policy
        .add_prefix_rule(&["touch".to_string()], Decision::Allow)
        .expect("test prefix should be valid");
    let state = CompactPermissionsState::new(&exec_policy);

    assert_eq!(
        state.render_diff(Unknown).map(|fragment| fragment.render()),
        None
    );
}

#[test]
fn does_not_render_existing_prefixes_without_a_previous_snapshot() {
    use PreviousSectionState::Absent;

    let mut exec_policy = Policy::empty();
    exec_policy
        .add_prefix_rule(&["touch".to_string()], Decision::Allow)
        .expect("test prefix should be valid");

    assert!(
        CompactPermissionsState::new(&exec_policy)
            .render_diff(Absent)
            .is_none()
    );
}
