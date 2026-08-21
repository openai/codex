use anyhow::Result;
use anyhow::ensure;
use app_test_support::TestAppServer;
use codex_app_server_protocol::BrowserUseRequirements;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ConfigRequirementsReadResponse;
use codex_app_server_protocol::InAppBrowserRequirements;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::RequestId;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::collections::BTreeMap;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

const READ_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 60);
const ALLOW: &str = "[in_app_browser]\nallow_external_browser_settings_import = true";
const DENY: &str = "[in_app_browser]\nallow_external_browser_settings_import = false";

async fn start_stable_server(codex_home: &TempDir) -> Result<TestAppServer> {
    let mut server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build()
        .await?;
    let initialized = timeout(
        READ_TIMEOUT,
        server.initialize_with_capabilities(
            ClientInfo {
                name: "browser_import_policy_test".to_string(),
                title: None,
                version: "0.1.0".to_string(),
            },
            /*capabilities*/ None,
        ),
    )
    .await??;
    ensure!(matches!(initialized, JSONRPCMessage::Response(_)));
    Ok(server)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn config_requirements_read_preserves_import_policy() -> Result<()> {
    for (contents, expected_table) in [
        ("", None),
        ("[in_app_browser]", None),
        ("allow_remote_control = true", None),
        ("allow_remote_control = true\n[in_app_browser]", Some(None)),
        (ALLOW, Some(Some(true))),
        (DENY, Some(Some(false))),
    ] {
        let codex_home = TempDir::new()?;
        std::fs::write(codex_home.path().join("requirements.toml"), contents)?;
        // An unrecognized ordinary user setting cannot override managed policy.
        std::fs::write(codex_home.path().join("config.toml"), ALLOW)?;
        let mut server = start_stable_server(&codex_home).await?;
        let request_id = server.send_config_requirements_read_request().await?;
        let wire: Value = timeout(READ_TIMEOUT, server.read_response(request_id)).await??;
        let expected_wire = expected_table
            .map(|value| json!({ "allowExternalBrowserSettingsImport": value }))
            .unwrap_or(Value::Null);
        if let Some(requirements) = wire["requirements"].as_object() {
            assert_eq!(requirements.get("inAppBrowser"), Some(&expected_wire));
        } else {
            assert_eq!(wire, json!({ "requirements": null }));
        }
        let response: ConfigRequirementsReadResponse = serde_json::from_value(wire)?;
        let actual = response
            .requirements
            .and_then(|requirements| requirements.in_app_browser);
        assert_eq!(
            actual,
            expected_table.map(|value| InAppBrowserRequirements {
                allow_external_browser_settings_import: value,
            }),
        );
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn import_policy_is_separate_from_browser_feature_and_agent_policy() -> Result<()> {
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("requirements.toml"),
        format!(
            "[features]\nin_app_browser = true\nbrowser_use = false\n\
             [browser_use]\ndisable_auto_review = true\n{DENY}"
        ),
    )?;
    std::fs::write(codex_home.path().join("config.toml"), ALLOW)?;
    let mut server = start_stable_server(&codex_home).await?;
    let request_id = server.send_config_requirements_read_request().await?;
    let response: ConfigRequirementsReadResponse =
        timeout(READ_TIMEOUT, server.read_response(request_id)).await??;
    let requirements = response.requirements.expect("managed requirements");
    assert_eq!(
        (
            requirements.in_app_browser,
            requirements.browser_use,
            requirements.feature_requirements,
        ),
        (
            Some(InAppBrowserRequirements {
                allow_external_browser_settings_import: Some(false),
            }),
            Some(BrowserUseRequirements {
                allow_history_access: None,
                disable_auto_review: Some(true),
                allow_global_persistent_approval: None,
                default_origin_policy: None,
                origins: None,
            }),
            Some(BTreeMap::from([
                ("browser_use".to_string(), false),
                ("in_app_browser".to_string(), true),
            ])),
        ),
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn config_requirements_read_rejects_invalid_import_policy() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut server = start_stable_server(&codex_home).await?;
    std::fs::write(
        codex_home.path().join("requirements.toml"),
        "[in_app_browser]\nallow_external_browser_settings_import = \"false\"",
    )?;
    let request_id = server.send_config_requirements_read_request().await?;
    let error = timeout(
        READ_TIMEOUT,
        server.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    ensure!(
        error
            .error
            .message
            .contains("allow_external_browser_settings_import"),
        "{}",
        error.error.message,
    );
    Ok(())
}
