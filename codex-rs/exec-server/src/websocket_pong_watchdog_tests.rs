use std::time::Duration;

use pretty_assertions::assert_eq;
use tokio::time::Instant;

use super::WebSocketPongWatchdog;

#[test]
fn repeated_ping_does_not_extend_deadline() {
    let started_at = Instant::now();
    let timeout = Duration::from_secs(2);
    let mut watchdog = WebSocketPongWatchdog::new(timeout);

    watchdog.ping_sent(started_at);
    watchdog.ping_sent(started_at + Duration::from_secs(1));
    assert_eq!(
        watchdog.write_deadline(started_at + Duration::from_secs(1)),
        started_at + timeout
    );
    assert_eq!(watchdog.deadline(), Some(started_at + timeout));
}

#[test]
fn pong_starts_a_fresh_deadline() {
    let started_at = Instant::now();
    let timeout = Duration::from_secs(2);
    let mut watchdog = WebSocketPongWatchdog::new(timeout);

    watchdog.ping_sent(started_at);
    watchdog.received_pong();
    assert_eq!(watchdog.deadline(), None);
    watchdog.ping_sent(started_at + timeout);
    assert_eq!(watchdog.deadline(), Some(started_at + timeout + timeout));
}
