use anyhow::Result;
use app_test_support::app_server_json_shutdown_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;

#[test]
fn standalone_app_server_emits_json_info_events() -> Result<()> {
    let codex_home = TempDir::new()?;
    let event = app_server_json_shutdown_event("codex-app-server", &[], codex_home.path())?;

    assert_eq!(
        event,
        json!({
            "level": "INFO",
            "fields": {
                "message": "processor task exited",
                "exit_reason": "last_connection_closed",
                "remaining_connection_count": 0,
                "shutdown_forced": false,
            },
            "target": "codex_app_server",
        })
    );

    Ok(())
}
