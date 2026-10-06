use anyhow::Result;
use app_test_support::TestAppServer;
use codex_app_server_protocol::ClientInfo;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn config_requirements_read_preserves_webmcp_policy() -> Result<()> {
    for (contents, expected_browser_use) in [
        ("", None),
        ("[browser_use]", None),
        (
            "[browser_use]\nallow_history_access = true",
            Some((Some(true), None)),
        ),
        (
            "[browser_use]\nallow_webmcp = true",
            Some((None, Some(true))),
        ),
        (
            "[browser_use]\nallow_webmcp = false",
            Some((None, Some(false))),
        ),
    ] {
        let codex_home = TempDir::new()?;
        std::fs::write(codex_home.path().join("requirements.toml"), contents)?;
        std::fs::write(
            codex_home.path().join("config.toml"),
            "[browser_use]\nallow_webmcp = true",
        )?;
        let mut server = TestAppServer::builder()
            .with_codex_home(codex_home.path())
            .build()
            .await?;
        let read_timeout = Duration::from_secs(/*secs*/ 60);
        timeout(
            read_timeout,
            server.initialize_with_capabilities(
                ClientInfo {
                    name: "webmcp_policy_test".to_string(),
                    title: None,
                    version: "0.1.0".to_string(),
                },
                /*capabilities*/ None,
            ),
        )
        .await??;
        let request_id = server.send_config_requirements_read_request().await?;
        let wire: Value = timeout(read_timeout, server.read_response(request_id)).await??;
        if let Some((allow_history_access, allow_webmcp)) = expected_browser_use {
            assert_eq!(
                wire["requirements"]["browserUse"],
                json!({
                    "allowWebmcp": allow_webmcp,
                    "allowHistoryAccess": allow_history_access,
                    "extension": null,
                    "disableAutoReview": null,
                    "allowGlobalPersistentApproval": null,
                    "defaultOriginPolicy": null,
                    "origins": null,
                }),
                "{contents}",
            );
        } else {
            assert_eq!(
                wire,
                json!({ "requirements": null, "supportsIndependentSpeedModes": true }),
                "{contents}"
            );
        }
    }
    Ok(())
}
