use anyhow::Result;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn app_server_capabilities_reports_websocket_auth_hash_support() -> Result<()> {
    let output = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
        .args(["app-server", "capabilities"])
        .output()?;

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let capabilities: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        capabilities,
        json!({
            "websocketAuth": {
                "capabilityTokenSha256": true,
            },
        })
    );
    Ok(())
}
