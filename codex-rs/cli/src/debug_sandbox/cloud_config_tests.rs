use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::write_chatgpt_auth;
use codex_config::ConfigRequirementsToml;
use codex_config::LoaderOverrides;
use codex_config::types::AuthCredentialsStoreMode;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::super::DebugSandboxConfigOptions;
use super::super::ManagedRequirementsMode;
use super::super::load_debug_sandbox_config_with_codex_home;
use super::bootstrap_cloud_config_bundle;

const CLOUD_MANAGED_PERMISSION_PROFILE_REQUIREMENTS: &str = r#"
default_permissions = "managed-cloud"

[allowed_permission_profiles]
managed-cloud = true

[permissions.managed-cloud]
extends = ":workspace"

[permissions.managed-cloud.network]
enabled = true
"#;

#[tokio::test]
async fn debug_sandbox_bootstraps_cloud_managed_permission_profile_from_backend() -> Result<()> {
    let server = MockServer::start().await;
    let expected_requirements = json!([{
        "id": "req-managed-cloud",
        "name": "Managed permissions",
        "contents": CLOUD_MANAGED_PERMISSION_PROFILE_REQUIREMENTS,
    }]);
    Mock::given(method("GET"))
        .and(path("/backend-api/wham/config/bundle"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "workspace-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "requirements_toml": {
                "enterprise_managed": expected_requirements.clone(),
            },
        })))
        .expect(1)
        .mount(&server)
        .await;

    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            "cli_auth_credentials_store = \"file\"\nchatgpt_base_url = \"{}/backend-api\"\n",
            server.uri(),
        ),
    )?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("workspace-123")
            .chatgpt_account_id("workspace-123")
            .chatgpt_user_id("user-123")
            .plan_type("enterprise"),
        AuthCredentialsStoreMode::File,
    )?;

    let options = DebugSandboxConfigOptions {
        sandbox_state: Default::default(),
        permissions_profile: Some("managed-cloud".to_string()),
        cwd: Some(codex_home.path().to_path_buf()),
        managed_requirements_mode: ManagedRequirementsMode::Include,
        loader_overrides: LoaderOverrides::without_managed_config_for_tests(),
    };
    let cloud_config_bundle = bootstrap_cloud_config_bundle(
        &[],
        &options,
        || AbsolutePathBuf::from_absolute_path(codex_home.path()),
        /*strict_config*/ false,
    )
    .await?;
    let config = load_debug_sandbox_config_with_codex_home(
        Vec::new(),
        /*codex_linux_sandbox_exe*/ None,
        options,
        Some(codex_home.path().to_path_buf()),
        cloud_config_bundle,
        /*strict_config*/ false,
    )
    .await?;

    assert_eq!(
        config
            .permissions
            .active_permission_profile()
            .map(|profile| profile.id),
        Some("managed-cloud".to_string()),
    );
    assert_eq!(
        config.permissions.network_sandbox_policy(),
        NetworkSandboxPolicy::Enabled,
    );
    assert_eq!(
        config.config_layer_stack.requirements_toml(),
        &toml::from_str::<ConfigRequirementsToml>(CLOUD_MANAGED_PERMISSION_PROFILE_REQUIREMENTS,)?,
    );

    let cache: Value = serde_json::from_slice(&std::fs::read(
        codex_home.path().join("cloud-config-bundle-cache.json"),
    )?)?;
    assert_eq!(
        json!({
            "chatgpt_user_id": cache["signed_payload"]["chatgpt_user_id"],
            "account_id": cache["signed_payload"]["account_id"],
            "requirements_toml": cache["signed_payload"]["bundle"]["requirements_toml"],
        }),
        json!({
            "chatgpt_user_id": "user-123",
            "account_id": "workspace-123",
            "requirements_toml": {
                "enterprise_managed": expected_requirements,
            },
        }),
    );
    server.verify().await;

    Ok(())
}
