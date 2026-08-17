use std::fs;
use std::path::Path;

use pretty_assertions::assert_eq;

use super::CheckStatus;
use super::desktop::inspect_desktop_session;

const CURRENT_SESSION: &str = "12345678-1234-1234-1234-123456789abc";
const CURRENT_PROCESS: u32 = 12345;

#[test]
fn current_desktop_session_reports_the_latest_local_handshake() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    write_log(
        directory.path(),
        concat!(
            "2026-08-07T12:00:00.000Z error [AppServerConnection] initialize_handshake_result outcome=failure transportKind=stdio\n",
            "2026-08-07T12:01:00.000Z info [AppServerConnection] Starting app-server connection hostId=local transport=websocket\n",
            "2026-08-07T12:02:00.000Z info [AppServerConnection] initialize_handshake_result outcome=success transportKind=websocket\n",
            "2026-08-07T12:03:00.000Z info [AppServerConnection] app_server_connection.state_changed hostId=remote transport=websocket\n",
            "2026-08-07T12:04:00.000Z error [AppServerConnection] initialize_handshake_result outcome=failure transportKind=websocket\n",
        ),
    );

    let (running, check) = inspect_desktop_session(directory.path(), |pid| pid == CURRENT_PROCESS);

    assert!(running);
    assert_eq!(check.id, "desktop.app_server.handshake");
    assert_eq!(check.status, CheckStatus::Ok);
    assert_eq!(
        check.summary,
        "the desktop app-server initialized successfully"
    );
}

#[test]
fn failed_handshake_does_not_expose_sensitive_log_fields() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    write_log(
        directory.path(),
        "2026-08-07T12:00:00.000Z error [AppServerConnection] initialize_handshake_result errorMessage=\"desktop-secret outcome=success\" outcome=failure transportKind=stdio\n",
    );

    let (running, check) = inspect_desktop_session(directory.path(), |_| true);

    assert!(running);
    assert_eq!(check.status, CheckStatus::Fail);
    assert_eq!(check.summary, "the desktop app-server failed to initialize");
    assert!(
        !serde_json::to_string(&check)
            .unwrap()
            .contains("desktop-secret")
    );
}

#[test]
fn handshake_failure_in_the_middle_of_a_log_segment_is_detected() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let padding = "x".repeat(96 * 1024);
    write_log(
        directory.path(),
        &format!(
            "{padding}\n2026-08-07T12:00:00.000Z error [AppServerConnection] initialize_handshake_result outcome=failure transportKind=stdio\n{padding}"
        ),
    );

    let (running, check) = inspect_desktop_session(directory.path(), |_| true);

    assert!(running);
    assert_eq!(check.status, CheckStatus::Fail);
}

#[test]
fn stopped_desktop_does_not_report_a_failure() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    write_log(
        directory.path(),
        "2026-08-07T12:00:00.000Z error [AppServerConnection] initialize_handshake_result outcome=failure transportKind=stdio\n",
    );

    let (running, check) = inspect_desktop_session(directory.path(), |_| false);

    assert!(!running);
    assert_eq!(check.status, CheckStatus::Ok);
    assert_eq!(check.summary, "the desktop application is not running");
}

fn write_log(directory: &Path, contents: &str) {
    fs::write(
        directory.join(format!(
            "codex-desktop-{CURRENT_SESSION}-{CURRENT_PROCESS}-t0-i0-120000-0.log"
        )),
        contents,
    )
    .expect("desktop log fixture should be created");
}
