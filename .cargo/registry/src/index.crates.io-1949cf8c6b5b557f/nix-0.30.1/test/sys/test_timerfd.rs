use nix::sys::time::{TimeSpec, TimeValLike};
use nix::sys::timerfd::{
    ClockId, Expiration, TimerFd, TimerFlags, TimerSetTimeFlags,
};
use std::time::Instant;

#[test]
pub fn test_timerfd_oneshot() {
    let timer =
        TimerFd::new(ClockId::CLOCK_MONOTONIC, TimerFlags::empty()).unwrap();

    let before = Instant::now();

    timer
        .set(
            Expiration::OneShot(TimeSpec::seconds(1)),
            TimerSetTimeFlags::empty(),
        )
        .unwrap();

    timer.wait().unwrap();

    let millis = before.elapsed().as_millis();
    assert!(millis > 900);
}

#[test]
pub fn test_timerfd_interval() {
    let timer =
        TimerFd::new(ClockId::CLOCK_MONOTONIC, TimerFlags::empty()).unwrap();

    let before = Instant::now();
    timer
        .set(
            Expiration::IntervalDelayed(
                TimeSpec::seconds(1),
                TimeSpec::seconds(2),
            ),
            TimerSetTimeFlags::empty(),
        )
        .unwrap();

    timer.wait().unwrap();

    let start_delay = before.elapsed().as_millis();
    assert!(start_delay > 900);

    timer.wait().unwrap();

    let interval_delay = before.elapsed().as_millis();
    assert!(interval_delay > 2900);
}

#[test]
pub fn test_timerfd_unset() {
    let timer =
        TimerFd::new(ClockId::CLOCK_MONOTONIC, TimerFlags::empty()).unwrap();

    timer
        .set(
            Expiration::OneShot(TimeSpec::seconds(1)),
            TimerSetTimeFlags::empty(),
        )
        .unwrap();

    timer.unset().unwrap();

    assert!(timer.get().unwrap().is_none());
}
