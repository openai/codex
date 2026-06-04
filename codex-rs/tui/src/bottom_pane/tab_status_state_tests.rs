use std::time::Duration;
use std::time::Instant;

use pretty_assertions::assert_eq;

use super::TabStatusState;
use crate::tab_status::TabStatus;

#[test]
fn activity_rolls_forward_and_resets_for_a_new_turn() {
    let mut state = TabStatusState::new();
    assert!(state.set_current_activity(Some(" Run cargo test ".to_string())));
    assert_eq!(state.current_activity(), Some("Run cargo test"));

    assert!(state.set_current_activity(/*activity*/ None));
    assert_eq!(state.current_activity(), None);
    assert_eq!(state.last_activity(), Some("Run cargo test"));

    state.reset_for_new_turn();
    assert_eq!(state.last_activity(), None);
}

#[test]
fn throttle_defers_detail_changes_but_not_status_changes() {
    let mut state = TabStatusState::new();
    let started_at = Instant::now();
    state.refresh((TabStatus::Working, Some("first".to_string())), started_at);

    assert_eq!(
        state.refresh_delay(
            TabStatus::Working,
            started_at + Duration::from_millis(/*millis*/ 100)
        ),
        Some(Duration::from_millis(/*millis*/ 150))
    );
    assert_eq!(
        state.refresh_delay(
            TabStatus::Waiting,
            started_at + Duration::from_millis(/*millis*/ 100)
        ),
        None
    );
}

#[test]
fn equal_refresh_rearms_the_detail_throttle() {
    let mut state = TabStatusState::new();
    let started_at = Instant::now();
    let unchanged_at = started_at + Duration::from_millis(/*millis*/ 300);
    let desired = (TabStatus::Working, Some("cargo test".to_string()));
    state.refresh(desired.clone(), started_at);
    state.refresh(desired, unchanged_at);

    assert_eq!(
        state.refresh_delay(
            TabStatus::Working,
            unchanged_at + Duration::from_millis(/*millis*/ 100)
        ),
        Some(Duration::from_millis(/*millis*/ 150))
    );
}
