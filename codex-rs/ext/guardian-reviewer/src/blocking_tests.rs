//! Checks overlapping approvals and turn boundaries with an explicit clock.

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn overlapping_reviews_span_turns_without_charging_idle_gaps() {
    let start = Instant::now();
    let mut interval = Interval::default();
    interval.start_turn(start);
    interval.enter(start);
    interval.enter(start + Duration::from_millis(10));
    interval.leave(start + Duration::from_millis(30));
    assert_eq!(interval.elapsed, Duration::ZERO);
    assert_eq!(
        interval.finish_turn(start + Duration::from_millis(40)),
        Duration::from_millis(40)
    );
    interval.start_turn(start + Duration::from_millis(60));
    interval.leave(start + Duration::from_millis(70));
    assert_eq!(
        interval.finish_turn(start + Duration::from_millis(80)),
        Duration::from_millis(10)
    );
    // A review that starts while idle contributes only after the next turn starts.
    interval.enter(start + Duration::from_millis(100));
    interval.start_turn(start + Duration::from_millis(200));
    interval.leave(start + Duration::from_millis(210));
    assert_eq!(
        interval.finish_turn(start + Duration::from_millis(220)),
        Duration::from_millis(10)
    );
    interval.start_turn(start + Duration::from_millis(230));
    interval.enter(start + Duration::from_millis(240));
    interval.leave(start + Duration::from_millis(250));
    interval.enter(start + Duration::from_millis(270));
    interval.leave(start + Duration::from_millis(280));
    assert_eq!(
        interval.finish_turn(start + Duration::from_millis(300)),
        Duration::from_millis(20)
    );
    interval.start_turn(start + Duration::from_millis(310));
    assert_eq!(
        interval.finish_turn(start + Duration::from_millis(320)),
        Duration::ZERO
    );
}

#[test]
fn late_turn_completion_preserves_both_samples_and_ignores_duplicate_stops() {
    let start = Instant::now();
    let mut state = State::default();
    state.start_turn("a", start);
    state.interval.enter(start + Duration::from_millis(10));
    state.start_turn("b", start + Duration::from_millis(30));
    assert_eq!(
        state.finish_turn("a", start + Duration::from_millis(50)),
        Some(Duration::from_millis(20))
    );
    assert_eq!(
        state.finish_turn("a", start + Duration::from_millis(60)),
        None
    );
    assert_eq!(
        state.finish_turn("b", start + Duration::from_millis(70)),
        Some(Duration::from_millis(40))
    );
}
