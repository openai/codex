use std::process::Command;

use anyhow::Context;
use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::write_chatgpt_auth;
use codex_config::ConfigLoadOptions;
use codex_config::types::AuthCredentialsStoreMode;
use codex_core::config::load_config_toml_with_layer_stack;
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

const CLOUD_MANAGED_PERMISSION_PROFILE_REQUIREMENTS: &str = r#"
default_permissions = "managed-cloud"

[allowed_permission_profiles]
managed-cloud = true

[permissions.managed-cloud]
extends = ":workspace"

[permissions.managed-cloud.network]
enabled = true
"#;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sandbox_fetches_and_enforces_cloud_managed_permission_profile() -> Result<()> {
    let server = MockServer::start().await;
    let chatgpt_base_url = format!("{}/backend-api", server.uri());
    let expected_requirements = json!([{
        "id": "req-managed-cloud",
        "name": "Managed permissions",
        "contents": CLOUD_MANAGED_PERMISSION_PROFILE_REQUIREMENTS,
    }]);

    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            "cli_auth_credentials_store = \"file\"\nchatgpt_base_url = \"{chatgpt_base_url}\"\n",
        ),
    )?;
    let bootstrap_config = load_config_toml_with_layer_stack(
        codex_home.path(),
        Some(&AbsolutePathBuf::from_absolute_path(codex_home.path())?),
        vec![
            (
                "cli_auth_credentials_store".to_string(),
                toml::Value::String("file".to_string()),
            ),
            (
                "chatgpt_base_url".to_string(),
                toml::Value::String(chatgpt_base_url.clone()),
            ),
        ],
        ConfigLoadOptions::default(),
    )
    .await?;
    if bootstrap_config.config_toml.cli_auth_credentials_store
        != Some(AuthCredentialsStoreMode::File)
        || bootstrap_config.config_toml.chatgpt_base_url.as_deref()
            != Some(chatgpt_base_url.as_str())
    {
        eprintln!(
            "skipping cloud-managed sandbox subprocess: host-managed authentication or backend routing prevents isolated mock credentials"
        );
        return Ok(());
    }

    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("workspace-123")
            .chatgpt_account_id("workspace-123")
            .chatgpt_user_id("user-123")
            .plan_type("enterprise"),
        AuthCredentialsStoreMode::File,
    )?;
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

    let codex = codex_utils_cargo_bin::cargo_bin("codex")?;
    let chatgpt_base_url_override = format!("chatgpt_base_url=\"{chatgpt_base_url}\"");
    let output = Command::new(&codex)
        .current_dir(codex_home.path())
        .env("CODEX_HOME", codex_home.path())
        .env("NO_PROXY", "127.0.0.1,localhost")
        .env("no_proxy", "127.0.0.1,localhost")
        .env_remove("CODEX_ACCESS_TOKEN")
        .env_remove("OPENAI_API_KEY")
        .args(["-c", "cli_auth_credentials_store=\"file\""])
        .args(["-c", chatgpt_base_url_override.as_str()])
        .args([
            "sandbox",
            "-P",
            "managed-cloud",
            "--include-managed-config",
            "--",
        ])
        .arg(&codex)
        .arg("--version")
        .output()?;
    let cloud_bundle_request_paths: Vec<_> = server
        .received_requests()
        .await
        .context("failed to read mock cloud configuration requests")?
        .into_iter()
        .map(|request| request.url.path().to_string())
        .collect();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let nested_macos_sandbox_unavailable = cfg!(target_os = "macos")
        && output.status.code() == Some(71)
        && stderr.contains("sandbox-exec: sandbox_apply: Operation not permitted");

    assert!(
        output.status.success() || nested_macos_sandbox_unavailable,
        "cloud-managed sandbox profile was not enforced: status={:?}; stdout={}; stderr={}; cloud bundle requests={cloud_bundle_request_paths:?}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        stderr,
    );
    if !nested_macos_sandbox_unavailable {
        assert!(
            String::from_utf8(output.stdout)?.starts_with("codex"),
            "expected the sandboxed Codex version command to run",
        );
    }

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
