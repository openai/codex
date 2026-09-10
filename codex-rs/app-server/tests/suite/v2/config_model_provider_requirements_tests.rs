//! Verifies provider requirements through the public configuration RPCs.

use anyhow::Result;
use app_test_support::TestAppServer;
use codex_app_server_protocol::ConfigReadParams;
use codex_app_server_protocol::ConfigReadResponse;
use codex_app_server_protocol::ConfigRequirementsReadResponse;
use codex_app_server_protocol::ConfigValueWriteParams;
use codex_app_server_protocol::ConfigWriteResponse;
use codex_app_server_protocol::MergeStrategy;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_config::config_toml::ConfigToml;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn provider_requirements_are_effective_and_read_only() -> Result<()> {
    let home = TempDir::new()?;
    let requirements = r#"
model_provider = "gateway"
[model_providers.gateway]
name = "Managed gateway"
base_url = "https://gateway.example.test/v1"
requires_openai_auth = true
"#;
    std::fs::write(home.path().join("requirements.toml"), requirements)?;
    std::fs::write(
        home.path().join("config.toml"),
        r#"
model_provider = "openai"
[model_providers.gateway]
name = "Local override"
base_url = "https://local.example.test"
env_key = "LOCAL_KEY"
[model_providers.gateway.http_headers]
X-Local = "no"
"#,
    )?;
    let expected: ConfigToml = toml::from_str(requirements)?;
    let expected_providers = json!(expected.model_providers);
    let mut server = TestAppServer::builder()
        .with_codex_home(home.path())
        .build()
        .await?;
    timeout(Duration::from_secs(/*secs*/ 60), server.initialize()).await??;

    let id = server.send_config_requirements_read_request().await?;
    let response: ConfigRequirementsReadResponse =
        timeout(Duration::from_secs(/*secs*/ 60), server.read_response(id)).await??;
    let requirements = response
        .requirements
        .expect("provider requirements are visible");
    assert_eq!(
        (
            requirements.model_provider,
            json!(requirements.model_providers)
        ),
        (Some("gateway".to_string()), expected_providers.clone())
    );

    let id = server
        .send_config_read_request(ConfigReadParams {
            include_layers: false,
            cwd: None,
        })
        .await?;
    let response: ConfigReadResponse =
        timeout(Duration::from_secs(/*secs*/ 60), server.read_response(id)).await??;
    assert_eq!(
        (
            response.config.model_provider.as_deref(),
            response.config.additional.get("model_providers")
        ),
        (Some("gateway"), Some(&expected_providers))
    );
    assert!(
        !response
            .origins
            .keys()
            .any(|key| key == "model_provider" || key.starts_with("model_providers.gateway."))
    );

    let id = server
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model_provider: Some("openai".to_string()),
            ephemeral: Some(true),
            ..Default::default()
        })
        .await?;
    let started: ThreadStartResponse =
        timeout(Duration::from_secs(/*secs*/ 60), server.read_response(id)).await??;
    assert_eq!(started.model_provider, "gateway");

    for (key, value) in [
        ("model_provider", json!("openai")),
        (
            "model_providers.gateway.base_url",
            json!("https://other.example.test"),
        ),
        ("model_providers", json!({})),
    ] {
        let id = server
            .send_config_value_write_request(ConfigValueWriteParams {
                file_path: None,
                key_path: key.into(),
                value,
                merge_strategy: MergeStrategy::Replace,
                expected_version: None,
            })
            .await?;
        let error = timeout(
            Duration::from_secs(/*secs*/ 60),
            server.read_stream_until_error_message(RequestId::Integer(id)),
        )
        .await??;
        assert_eq!(
            error.error.data,
            Some(json!({"config_write_error_code": "configRequirementReadonly"}))
        );
    }

    let id = server
        .send_config_value_write_request(ConfigValueWriteParams {
            file_path: None,
            key_path: "model_providers.other".into(),
            value: json!({"name": "Other provider"}),
            merge_strategy: MergeStrategy::Replace,
            expected_version: None,
        })
        .await?;
    let _: ConfigWriteResponse =
        timeout(Duration::from_secs(/*secs*/ 60), server.read_response(id)).await??;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dotted_managed_provider_id_hides_exact_origins() -> Result<()> {
    let home = TempDir::new()?;
    std::fs::write(
        home.path().join("requirements.toml"),
        "[model_providers.\"corp.gateway\"]\nname = 'Managed gateway'\nbase_url = 'https://managed.example.test'\n",
    )?;
    std::fs::write(
        home.path().join("config.toml"),
        "[model_providers.\"corp.gateway\"]\nname = 'Local gateway'\nbase_url = 'https://local.example.test'\n[model_providers.corp]\nname = 'Other provider'\n",
    )?;
    let mut server = TestAppServer::builder()
        .with_codex_home(home.path())
        .build_initialized_with_timeout(Duration::from_secs(/*secs*/ 60))
        .await?;
    let id = server
        .send_config_read_request(ConfigReadParams {
            include_layers: false,
            cwd: None,
        })
        .await?;
    let response: ConfigReadResponse =
        timeout(Duration::from_secs(/*secs*/ 60), server.read_response(id)).await??;
    assert_eq!(
        response.config.additional["model_providers"]["corp.gateway"]["name"],
        json!("Managed gateway")
    );
    assert!(
        !response
            .origins
            .contains_key("model_providers.corp.gateway.name")
    );
    assert!(response.origins.contains_key("model_providers.corp.name"));
    Ok(())
}
