use super::super::test_support::render_section_cases;
use super::*;

fn state(active: bool, start_instructions: Option<&str>) -> RealtimeState {
    RealtimeState::new(active, start_instructions)
}

#[test]
fn snapshots() {
    use PreviousSectionState::Absent;
    use PreviousSectionState::Known;
    use PreviousSectionState::Unknown;

    let inactive = state(/*active*/ false, /*start_instructions*/ None);
    let active = state(/*active*/ true, /*start_instructions*/ None);
    let custom_active = state(/*active*/ true, Some("custom realtime instructions"));
    let changed_custom_active = state(
        /*active*/ true,
        Some("changed custom realtime instructions"),
    );

    insta::assert_snapshot!(render_section_cases(&[
        (Absent, Absent),
        (Absent, Known(&inactive)),
        (Absent, Known(&active)),
        (Known(&inactive), Known(&active)),
        (Known(&inactive), Known(&custom_active)),
        (Known(&active), Known(&active)),
        (Known(&custom_active), Known(&changed_custom_active)),
        (Known(&active), Known(&inactive)),
        (Unknown, Known(&active)),
        (Unknown, Known(&inactive)),
    ]));
}

#[test]
fn retained_fragment_matcher_only_matches_starts() {
    let start = RealtimeStartWithInstructions::new("custom instructions").render();
    let end = RealtimeEndInstructions::new("inactive").render();

    assert!(RealtimeState::matches_legacy_fragment("developer", &start));
    assert!(!RealtimeState::matches_legacy_fragment("developer", &end));
}
