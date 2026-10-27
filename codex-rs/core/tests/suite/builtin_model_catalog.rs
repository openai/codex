//! Built-in OpenAI catalog routing uses configured metadata without redirecting inference.
#![cfg(test)]

use anyhow::Result;
use codex_core::config::ConfigBuilder;
use codex_login::CodexAuth;
use codex_models_manager::bundled_models_response;
use core_test_support::responses;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use tempfile::TempDir;
use test_case::test_case;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[test_case(CodexAuth::from_api_key("catalog-routing-test-key"), "Bearer catalog-routing-test-key"; "api_key")]
#[test_case(CodexAuth::create_dummy_chatgpt_auth_for_testing(), "Bearer Access Token"; "chatgpt")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn builtin_openai_catalog_override_preserves_inference(
    auth: CodexAuth,
    authorization: &str,
) -> Result<()> {
    let inference = responses::start_mock_server().await;
    let catalog_server = MockServer::start().await;
    let mut catalog = bundled_models_response()?;
    catalog.models[0].display_name = "Explicit built-in catalog".into();
    Mock::given(method("GET"))
        .and(path("/catalog"))
        .and(header("authorization", authorization))
        .respond_with(ResponseTemplate::new(200).set_body_json(&catalog))
        .mount(&catalog_server)
        .await;
    let home = Arc::new(TempDir::new()?);
    std::fs::write(
        home.path().join("config.toml"),
        format!(
            r#"model_provider = "openai"
openai_base_url = "{}/v1"
[features]
api_key_model_discovery = true
[model_providers.openai]
model_catalog_url = "https://chatgpt.com/backend-api/codex/models"
"#,
            inference.uri(),
        ),
    )?;
    let mut loaded = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .build()
        .await?;
    assert_eq!(
        loaded
            .model_provider
            .model_catalog_url
            .as_deref()
            .map(String::as_str),
        Some("https://chatgpt.com/backend-api/codex/models"),
    );
    // Config validation uses the production allowlist. Redirect only this test's
    // runtime catalog client to a local server to capture credential routing.
    loaded.model_provider.model_catalog_url =
        Some(format!("{}/catalog", catalog_server.uri()).into());
    assert_eq!(loaded.model_provider_id, "openai");
    assert_eq!(loaded.model_provider.name, "OpenAI");
    let test = test_codex()
        .with_home(home)
        .with_auth(auth)
        .with_config(move |config| {
            config.model_provider = loaded.model_provider.clone();
            config.model_provider.supports_websockets = false;
            config.features = loaded.features;
        })
        .build_with_auto_env(&inference)
        .await?;
    let response = responses::mount_sse_once(&inference, responses::sse_completed("done")).await;
    test.submit_turn("hello").await?;
    assert_eq!(
        response.single_request().header("authorization").as_deref(),
        Some(authorization),
    );
    assert_eq!(
        test.thread_manager
            .get_models_manager()
            .get_remote_models()
            .await,
        catalog.models,
    );
    assert!(
        inference
            .received_requests()
            .await
            .unwrap()
            .iter()
            .all(|request| request.url.path() != "/v1/models")
    );
    assert_eq!(catalog_server.received_requests().await.unwrap().len(), 1);
    Ok(())
}
