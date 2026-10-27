use super::*;
use crate::context::world_state::test_support::FragmentSectionTestExt as _;
use codex_execpolicy::Decision;
use codex_protocol_test_support::ContentItemTestExt;
use pretty_assertions::assert_eq;

#[test_case::test_case(None, "Approved command prefix saved:"; "bundled")]
#[test_case::test_case(Some("Approved command prefix saved:"), "Approved command prefix saved:"; "catalog_with_legacy_heading")]
#[test_case::test_case(Some("Newly approved:"), "Approved command prefix saved:\nNewly approved:"; "catalog")]
fn renders_only_newly_approved_prefixes(heading: Option<&str>, expected: &str) {
    use PreviousSectionState::Known;

    let mut exec_policy = Policy::empty();
    exec_policy
        .add_prefix_rule(&["git".to_string(), "pull".to_string()], Decision::Allow)
        .expect("test prefix should be valid");
    let with_existing_prefix =
        CompactPermissionsState::new(&exec_policy, /*saved_prefix_heading*/ None);
    exec_policy
        .add_prefix_rule(&["cargo".to_string(), "test".to_string()], Decision::Allow)
        .expect("test prefix should be valid");
    let with_new_prefix = CompactPermissionsState::new(&exec_policy, heading);
    let existing_snapshot = with_existing_prefix
        .render_fragment_diff(PreviousSectionState::Absent)
        .0
        .unwrap();
    let current_snapshot = with_new_prefix
        .render_fragment_diff(PreviousSectionState::Absent)
        .0
        .unwrap();

    assert_eq!(
        with_new_prefix
            .render_fragment_diff(Known(&existing_snapshot))
            .1
            .map(|fragment| fragment.render().expect_text()),
        Some(format!("{expected}\n- [\"cargo\", \"test\"]"))
    );
    assert!(
        with_new_prefix
            .render_fragment_diff(Known(&current_snapshot))
            .1
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
    let state = CompactPermissionsState::new(&exec_policy, /*saved_prefix_heading*/ None);

    assert_eq!(
        state
            .render_fragment_diff(Unknown)
            .1
            .map(|fragment| fragment.render().expect_text()),
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
        CompactPermissionsState::new(&exec_policy, /*saved_prefix_heading*/ None)
            .render_fragment_diff(Absent)
            .1
            .is_none()
    );
}
