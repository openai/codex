//! Reconnect delays vary across samples while retaining a nonzero floor and the original cap.

use std::time::Duration;

use pretty_assertions::assert_eq;

use super::ReconnectBackoff;
use super::STABLE_CONNECTION_DURATION;
use super::reconnect_delay;

#[test]
fn reconnect_jitter_stays_within_each_backoff_window() {
    for seconds in [1, 2, 4, 8, 16, 30] {
        let backoff = Duration::from_secs(seconds);
        let half_ms = seconds * 500;
        assert_eq!(reconnect_delay(backoff, /*random_sample*/ 0), backoff / 2);
        assert_eq!(reconnect_delay(backoff, half_ms), backoff);
        for sample in [1, 1234, 987654, u64::MAX] {
            let delay = reconnect_delay(backoff, sample);
            assert!((backoff / 2..=backoff).contains(&delay));
        }
        assert_ne!(
            reconnect_delay(backoff, /*random_sample*/ 1),
            reconnect_delay(backoff, /*random_sample*/ 2)
        );
    }
}

#[test]
fn flapping_connections_retain_backoff_until_a_stable_session() {
    let mut backoff = ReconnectBackoff::new();
    for seconds in [1, 2, 4, 8, 16, 30, 30] {
        backoff.connection_closed(STABLE_CONNECTION_DURATION - Duration::from_millis(1));
        assert_eq!(
            backoff.next_delay(/*random_sample*/ 0),
            Duration::from_millis(seconds * 500)
        );
    }
    backoff.connection_closed(STABLE_CONNECTION_DURATION);
    assert_eq!(
        backoff.next_delay(/*random_sample*/ 0),
        Duration::from_millis(500)
    );
    assert_eq!(
        backoff.next_delay(/*random_sample*/ 0),
        Duration::from_secs(1)
    );
}
