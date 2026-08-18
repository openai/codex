use super::*;
use pretty_assertions::assert_eq;

#[test]
fn reconnect_delay_backs_off_and_caps() {
    assert_eq!(
        reconnect_delay(/*rapid_disconnects*/ 1),
        Duration::from_millis(200)
    );
    assert_eq!(
        reconnect_delay(/*rapid_disconnects*/ 2),
        Duration::from_millis(400)
    );
    assert_eq!(
        reconnect_delay(/*rapid_disconnects*/ 3),
        Duration::from_millis(800)
    );
    assert_eq!(
        reconnect_delay(/*rapid_disconnects*/ 10),
        RECONNECT_MAX_DELAY
    );
}

#[test]
fn recognizes_terminal_sideband_statuses() {
    for status in [StatusCode::NOT_FOUND, StatusCode::GONE] {
        assert!(webrtc_sideband_session_ended(&ApiError::Api {
            status,
            message: "session ended".to_string(),
        }));
    }
    assert!(!webrtc_sideband_session_ended(&ApiError::Api {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: "retry".to_string(),
    }));
}
