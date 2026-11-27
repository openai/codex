use super::*;
use pretty_assertions::assert_eq;

#[test]
fn next_tick_tracks_the_displayed_clock_through_pauses_and_phase_overrides() {
    let mut timer = StatusTimer::default();
    timer.pause_at(timer.last_resume_at + Duration::from_millis(250));
    let now = timer.last_resume_at + Duration::from_secs(10);
    assert_eq!(timer.next_tick_delay(now), None);

    timer.display_started_at = Some(now - Duration::from_millis(800));
    assert_eq!(timer.next_tick_delay(now), Some(Duration::from_millis(200)));

    timer.display_started_at = None;
    timer.resume_at(now);
    assert_eq!(timer.next_tick_delay(now), Some(Duration::from_millis(750)));
}

#[test]
fn pauses_and_resumes_without_counting_wait_time() {
    let baseline = Instant::now();
    let mut timer = StatusTimer {
        last_resume_at: baseline,
        ..StatusTimer::default()
    };
    timer.pause_at(baseline + Duration::from_secs(5));
    timer.pause_at(baseline + Duration::from_secs(8));
    assert_eq!(
        timer.elapsed_at(baseline + Duration::from_secs(10)),
        Duration::from_secs(5)
    );
    timer.resume_at(baseline + Duration::from_secs(10));
    timer.resume_at(baseline + Duration::from_secs(12));
    assert_eq!(
        timer.elapsed_at(baseline + Duration::from_secs(13)),
        Duration::from_secs(8)
    );
    timer.reset(Duration::from_secs(24));
    assert_eq!(
        timer.elapsed_at(timer.last_resume_at),
        Duration::from_secs(24)
    );
    timer.pause_at(timer.last_resume_at);
    timer.reset(Duration::ZERO);
    assert_eq!(
        timer.elapsed_at(timer.last_resume_at + Duration::from_secs(/*secs*/ 120)),
        Duration::ZERO
    );
}
