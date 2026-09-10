//! Exercises managed provider routing and conflict diagnostics through real turns.

use anyhow::Result;
use codex_config::LoaderOverrides;
use codex_config::config_toml::ConfigToml;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_core::config::ConfigBuilder;
use codex_core::config::ConfigOverrides;
use codex_login::CodexAuth;
use codex_models_manager::bundled_models_response;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_models_once;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use tempfile::tempdir;
use test_case::test_case;
use wiremock::MockServer;

#[tokio::test]
async fn cloud_provider_auth_merges_before_parsing_and_resolves_cwd_from_codex_home() -> Result<()>
{
    let home = tempdir()?;
    std::fs::write(
        home.path().join("config.toml"),
        "[model_providers.gateway]\nname = 'Local'\nexperimental_bearer_token = 'local-token'",
    )?;
    // Cloud requirements arrive highest-priority first.
    let managed = CloudConfigBundleFixture::enterprise_requirement(
        "[model_providers.gateway.auth]\ntimeout_ms = 10000\ncwd = 'auth'",
    )
    .add_enterprise_requirement(
        r#"
model_provider = "gateway"
[model_providers.gateway]
name = "Managed gateway"
base_url = "https://gateway.example/v1"
[model_providers.gateway.auth]
command = "get-token"
args = ["--token"]
refresh_interval_ms = 12345
"#,
    )
    .into_loader();
    let config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
        .cloud_config_bundle(managed)
        .build()
        .await?;
    let cwd = toml::Value::String(home.path().join("auth").display().to_string());
    let expected: ConfigToml = toml::from_str(&format!(
        r#"
[model_providers.gateway]
name = "Managed gateway"
base_url = "https://gateway.example/v1"
[model_providers.gateway.auth]
command = "get-token"
args = ["--token"]
timeout_ms = 10000
refresh_interval_ms = 12345
cwd = {cwd}
"#
    ))?;
    assert_eq!(config.model_provider, expected.model_providers["gateway"]);
    Ok(())
}

#[test_case("model_provider = 'gateway'", "other"; "selection_and_definition")]
#[test_case("", "gateway"; "definition_only")]
#[test_case("model_provider = 'gateway'", "matching"; "matching_configuration")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn required_provider_routes_models_and_inference_with_chatgpt_auth(
    required_selection: &str,
    local_selection: &str,
) -> Result<()> {
    let gateway = MockServer::start().await;
    let local = MockServer::start().await;
    let gateway_url = format!("{}/v1", gateway.uri());
    let local_url = format!("{}/v1", local.uri());
    let local_config = format!(
        r#"
model_provider = "{local_selection}"
[model_providers.gateway]
name = "Local gateway"
base_url = "{local_url}"
env_key = "CODEX_TEST_LOCAL_GATEWAY_KEY_MUST_NOT_BE_USED"
experimental_bearer_token = "local-token"
[model_providers.gateway.http_headers]
X-Local = "local"
[model_providers.other]
name = "Other provider"
base_url = "{local_url}"
"#
    );
    let requirements = format!(
        r#"
{required_selection}
[model_providers.gateway]
name = "Managed gateway"
base_url = "{gateway_url}"
requires_openai_auth = true
[model_providers.gateway.http_headers]
X-Managed = "required"
"#
    );
    let local_config = if local_selection == "matching" {
        requirements.clone()
    } else {
        local_config
    };
    let managed = CloudConfigBundleFixture::loader_with_enterprise_requirement(requirements);
    let models = mount_models_once(&gateway, bundled_models_response()?).await;
    let responses = mount_sse_once(
        &gateway,
        sse(vec![
            ev_response_created("response-1"),
            ev_completed("response-1"),
        ]),
    )
    .await;
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let expected_authorization = format!("Bearer {}", auth.get_token()?);
    let test = test_codex()
        .with_auth(auth)
        .with_pre_build_hook(move |home| {
            std::fs::write(home.join("config.toml"), local_config).expect("write local config");
        })
        .with_cloud_config_bundle(managed.clone())
        .with_config(|config| {
            // The fixture normally replaces the selected provider with its mock provider.
            // Restore the provider entry selected by the real requirements-aware loader.
            config.model_provider = config.model_providers[&config.model_provider_id].clone();
        })
        .build_with_auto_env(&local)
        .await?;

    let warnings = test
        .config
        .startup_warnings
        .iter()
        .filter(|warning| warning.contains("`model_provider"))
        .cloned()
        .collect::<Vec<_>>();
    if local_selection == "other" {
        insta::assert_debug_snapshot!("provider_selection_override_warning", warnings);
    } else {
        assert_eq!(warnings, Vec::<String>::new());
    }
    if !required_selection.is_empty() {
        for cli_provider in ["openai", "gateway"] {
            let overridden = ConfigBuilder::default()
                .codex_home(test.config.codex_home.to_path_buf())
                .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
                .cloud_config_bundle(managed.clone())
                .cli_overrides(vec![(
                    "model_provider".into(),
                    toml::Value::String(cli_provider.into()),
                )])
                .harness_overrides(ConfigOverrides {
                    model_provider: Some("openai".into()),
                    ..Default::default()
                })
                .build()
                .await?;
            let warnings = overridden
                .startup_warnings
                .iter()
                .filter(|warning| warning.contains("`model_provider"))
                .cloned()
                .collect::<Vec<_>>();
            if cli_provider == "gateway" {
                assert_eq!(warnings, Vec::<String>::new());
            } else {
                insta::assert_debug_snapshot!("provider_selection_override_warning", warnings);
            }
            assert_eq!(&overridden.model_provider, &test.config.model_provider);
            let rebuilt = overridden
                .rebuild_preserving_session_layers(&overridden)
                .await?;
            assert_eq!(rebuilt.model_provider, test.config.model_provider);
        }
    }
    test.submit_turn("Use the managed gateway.").await?;

    let inference_request = responses.single_request();
    assert_eq!(models.single_request_path(), "/v1/models");
    assert_eq!(inference_request.path(), "/v1/responses");
    let header_names = ["authorization", "x-managed", "x-local"];
    let expected_headers = [
        Some(expected_authorization),
        Some("required".to_string()),
        None,
    ];
    assert_eq!(
        header_names.map(|name| inference_request.header(name)),
        expected_headers
    );
    for request in models.requests() {
        let actual = header_names.map(|name| {
            request
                .headers
                .get(name)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string)
        });
        assert_eq!(actual, expected_headers);
    }
    assert!(
        local
            .received_requests()
            .await
            .expect("recorded local requests")
            .is_empty()
    );
    Ok(())
}
