use super::*;

use pretty_assertions::assert_eq;

#[test]
fn retry_backoff_grows_exponentially_and_caps_at_five_minutes() {
    let lower_bounds = [24, 48, 96, 192, 240, 240];
    let upper_bounds = [36, 72, 144, 288, 300, 300];

    for (index, (lower_seconds, upper_seconds)) in
        lower_bounds.into_iter().zip(upper_bounds).enumerate()
    {
        let consecutive_failures = u32::try_from(index + 1).expect("failure count fits in u32");
        for jitter_sample in [0, 1, u64::MAX / 2, u64::MAX] {
            let delay = jittered_exponential_backoff(consecutive_failures, jitter_sample);
            assert!(delay >= Duration::from_secs(lower_seconds));
            assert!(delay <= Duration::from_secs(upper_seconds));
        }
    }
}

#[test]
fn retry_backoff_jitter_is_stable_for_a_sample() {
    let delay =
        jittered_exponential_backoff(/*consecutive_failures*/ 2, /*jitter_sample*/ 42);

    assert_eq!(delay, Duration::from_millis(48_042));
}
