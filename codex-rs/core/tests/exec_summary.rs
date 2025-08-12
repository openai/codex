#![expect(clippy::unwrap_used)]

use std::time::Duration;

use codex_core::protocol::{ExecCommandEndEvent, ExecCommandSummary};

#[test]
fn exec_end_event_summary_serialization_roundtrip_some() {
    let event = ExecCommandEndEvent {
        call_id: "call-123".into(),
        stdout: "out".into(),
        stderr: "err".into(),
        exit_code: 1,
        duration: Duration::from_secs(1),
        summary: Some(ExecCommandSummary {
            cwd: std::path::PathBuf::from("/tmp"),
            stderr_tail: "e".into(),
            stdout_tail: "o".into(),
            stdout_truncated_after_lines: Some(10),
            stderr_truncated_after_lines: None,
        }),
    };

    let json = serde_json::to_string(&event).unwrap();
    let de: ExecCommandEndEvent = serde_json::from_str(&json).unwrap();

    assert_eq!(de.call_id, event.call_id);
    assert_eq!(de.exit_code, 1);
    assert!(de.summary.is_some());
    let s = de.summary.unwrap();
    assert_eq!(s.cwd, std::path::PathBuf::from("/tmp"));
    assert_eq!(s.stdout_truncated_after_lines, Some(10));
    assert_eq!(s.stderr_truncated_after_lines, None);
}

#[test]
fn exec_end_event_summary_serialization_roundtrip_none() {
    let event = ExecCommandEndEvent {
        call_id: "call-456".into(),
        stdout: String::new(),
        stderr: String::new(),
        exit_code: 0,
        duration: Duration::from_millis(0),
        summary: None,
    };

    let json = serde_json::to_string(&event).unwrap();
    let de: ExecCommandEndEvent = serde_json::from_str(&json).unwrap();

    assert_eq!(de.call_id, event.call_id);
    assert!(de.summary.is_none());
}
