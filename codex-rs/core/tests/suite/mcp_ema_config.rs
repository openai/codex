//! Exercises EMA activation and registration ownership through real configuration layers.

use std::sync::Arc;

use anyhow::Result;
use codex_config::LoaderOverrides;
use codex_config::McpServerAuth;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_core::ConfigRefreshOutcome;
use codex_core::config::ConfigBuilder;
use codex_core::config::set_project_trust_level;
use codex_login::CodexAuth;
use codex_protocol::config_types::TrustLevel;
use codex_protocol::protocol::EventMsg;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use tempfile::tempdir;
use test_case::test_case;
use wiremock::MockServer;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn refresh_ema_denial_clears_authority_for_configured_servers() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let endpoint = MockServer::start().await;
    let responses_server = responses::start_mock_server().await;
    let home = Arc::new(tempdir()?);
    let url = format!("{}/mcp", endpoint.uri());
    let policy = |revision| {
        format!(
            "[features]\nuse_xaa = true\napps = false\nsecret_auth_storage = false\n\
             [mcp_enterprise_managed_auth.idp]\nissuer = '{}/idp-{revision}'\nclient_id = 'client-{revision}'\n\
             [mcp_servers.enterprise]\nurl = '{url}'\nauth = 'ema_auth'\noauth_resource = 'resource-{revision}'\n",
            endpoint.uri(),
        )
    };
    let fixture = test_codex()
        .with_home(Arc::clone(&home))
        .with_auth(CodexAuth::from_api_key("test-api-key"))
        .with_cloud_config_bundle(CloudConfigBundleFixture::loader_with_enterprise_config(
            policy(1),
        ))
        .build_with_auto_env(&responses_server)
        .await?;
    let (before, _) = fixture.codex.current_mcp_config_and_runtime_context().await;
    let current_config = fixture.codex.config().await;
    let refreshed = ConfigBuilder::default()
        .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
        .codex_home(home.path().to_path_buf())
        .fallback_cwd(Some(fixture.config.cwd.to_path_buf()))
        .cloud_config_bundle(
            CloudConfigBundleFixture::enterprise_config(policy(2))
                .add_enterprise_requirement("[features]\nuse_xaa = false\n")
                .into_loader(),
        )
        .build()
        .await?;
    assert_eq!(
        fixture
            .codex
            .refresh_mcp_config(current_config, refreshed)
            .await,
        ConfigRefreshOutcome::Published
    );
    let (after, _) = fixture.codex.current_mcp_config_and_runtime_context().await;
    let old_servers = codex_mcp::configured_mcp_servers(&before);
    let new_servers = codex_mcp::configured_mcp_servers(&after);
    let old = old_servers["enterprise"]
        .ema_registration()
        .expect("original registration");
    assert!(old_servers["enterprise"].enabled);
    assert!(
        !new_servers["enterprise"].enabled,
        "managed XAA opt-out must apply"
    );
    assert_eq!(old.resource(), Some("resource-1"));
    assert!(new_servers["enterprise"].ema_registration().is_none());
    fixture.codex.shutdown_and_wait().await?;
    assert!(endpoint.received_requests().await.unwrap().is_empty());
    Ok(())
}

#[derive(Clone, Copy)]
enum ActivationScenario {
    FeatureDisabled,
    MissingIdp,
    OperatorEnabled,
}

#[test_case(ActivationScenario::FeatureDisabled; "feature disabled")]
#[test_case(ActivationScenario::MissingIdp; "missing IdP")]
#[test_case(ActivationScenario::OperatorEnabled; "operator registration is eligible for startup")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn enterprise_activation_respects_configuration(scenario: ActivationScenario) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = MockServer::start().await;
    let responses_server = responses::start_mock_server().await;
    let home = Arc::new(tempdir()?);
    let resource = format!("{}/mcp", server.uri());
    let issuer = format!("{}/idp", server.uri());
    let xaa_enabled = !matches!(scenario, ActivationScenario::FeatureDisabled);
    let mut managed_config = format!(
        "mcp_oauth_credentials_store = \"file\"\n[features]\nuse_xaa = {xaa_enabled}\nsecret_auth_storage = false\napps = false\n"
    );
    if !matches!(scenario, ActivationScenario::MissingIdp) {
        managed_config.push_str(&format!(
            "[mcp_enterprise_managed_auth.idp]\nissuer = {issuer:?}\nclient_id = \"idp-client\"\n"
        ));
    }
    managed_config.push_str(&format!(
        "[mcp_servers.enterprise]\nurl = {resource:?}\nauth = \"ema_auth\"\n[mcp_servers.enterprise.oauth]\nclient_id = \"mcp-client\"\n"
    ));
    let fixture = test_codex()
        .with_home(home)
        .with_auth(CodexAuth::from_api_key("test-api-key"))
        .with_cloud_config_bundle(CloudConfigBundleFixture::loader_with_enterprise_config(
            managed_config,
        ))
        .build_with_auto_env(&responses_server)
        .await?;
    let (runtime_config, _) = fixture.codex.current_mcp_config_and_runtime_context().await;
    let expected_enabled = match scenario {
        ActivationScenario::FeatureDisabled | ActivationScenario::MissingIdp => Some(false),
        ActivationScenario::OperatorEnabled => Some(true),
    };
    assert_eq!(
        codex_mcp::configured_mcp_servers(&runtime_config)
            .get("enterprise")
            .map(|server| server.enabled),
        expected_enabled,
    );
    let startup = wait_for_event_match(&fixture.codex, |event| match event {
        EventMsg::McpStartupComplete(summary) => Some(summary.clone()),
        _ => None,
    })
    .await;
    // Eligible registration reaches startup, which fails closed without usable EMA auth.
    // Ineligible declarations must never start, even when their URLs are reachable.
    let expected_failed = if expected_enabled == Some(true) {
        vec!["enterprise".to_string()]
    } else {
        Vec::new()
    };
    assert_eq!(
        (
            startup.ready,
            startup
                .failed
                .into_iter()
                .map(|failure| failure.server)
                .collect::<Vec<_>>(),
            startup.cancelled,
        ),
        (Vec::<String>::new(), expected_failed, Vec::<String>::new()),
    );
    fixture.codex.shutdown_and_wait().await?;
    assert!(
        server
            .received_requests()
            .await
            .expect("recorded requests")
            .is_empty()
    );
    Ok(())
}

#[test_case("oauth"; "OAuth")]
#[test_case("chatgpt"; "ChatGPT")]
#[tokio::test]
async fn trusted_project_cannot_downgrade_enterprise_auth(project_auth: &str) -> Result<()> {
    let home = tempdir()?;
    let workspace = tempdir()?;
    std::fs::create_dir_all(workspace.path().join(".git"))?;
    std::fs::create_dir_all(workspace.path().join(".codex"))?;
    set_project_trust_level(home.path(), workspace.path(), TrustLevel::Trusted)?;
    let managed_config = r#"
[features]
use_xaa = true
[mcp_enterprise_managed_auth.idp]
issuer = "https://idp.example"
client_id = "idp-client"
[mcp_servers.enterprise]
url = "https://resource.example/mcp"
auth = "ema_auth"
scopes = ["files.read"]
oauth_resource = "https://resource.example/mcp"
[mcp_servers.enterprise.oauth]
client_id = "mcp-client"
"#;
    let builder = ConfigBuilder::default()
        .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
        .codex_home(home.path().to_path_buf())
        .fallback_cwd(Some(workspace.path().to_path_buf()))
        .cloud_config_bundle(
            CloudConfigBundleFixture::enterprise_config(managed_config.to_string())
                .add_enterprise_requirement(
                    "[mcp_servers.enterprise.identity]\nurl = \"https://resource.example/mcp\"\n",
                )
                .into_loader(),
        );
    assert_eq!(
        builder.clone().build().await?.mcp_servers.get()["enterprise"].auth,
        McpServerAuth::EmaAuth,
    );
    std::fs::write(
        workspace.path().join(".codex/config.toml"),
        format!("[mcp_servers.enterprise]\nauth = {project_auth:?}\n"),
    )?;
    let error = builder
        .build()
        .await
        .expect_err("a trusted project must not downgrade EMA before MCP startup");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    let message = format!("{error:#}");
    assert!(
        message.ends_with("one non-project config layer"),
        "{message}"
    );
    Ok(())
}
