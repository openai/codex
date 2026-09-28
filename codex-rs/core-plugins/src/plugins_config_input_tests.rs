//! Verify connection reuse without retaining request metadata or crossing config lifetimes.

use super::PluginsConfigInput;
use crate::remote::fetch_recommended_plugins;
use crate::test_support::test_http_client_factory;
use codex_config::ConfigLayerStack;
use codex_login::CodexAuth;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[test]
fn repeated_service_configs_and_early_clones_share_one_lazy_pool() {
    let input = PluginsConfigInput::new(
        ConfigLayerStack::default(),
        "openai".to_string(),
        /*plugins_enabled*/ true,
        /*remote_plugin_enabled*/ true,
        "https://chatgpt.com/backend-api".to_string(),
        test_http_client_factory(),
        /*product_sku*/ None,
    );
    let clone = input.clone();
    assert!(input.remote_http_clients.get().is_none());
    let first = input.remote_plugin_service_config();
    let reuse: Vec<_> = (0..220)
        .map(|_| {
            Arc::ptr_eq(
                &first.http_clients,
                &clone.remote_plugin_service_config().http_clients,
            )
        })
        .collect();
    let new_input = PluginsConfigInput::new(
        input.config_layer_stack.clone(),
        input.model_provider_id.clone(),
        input.plugins_enabled,
        input.remote_plugin_enabled,
        input.chatgpt_base_url.clone(),
        input.http_client_factory.clone(),
        input.product_sku,
    );
    assert_eq!(
        (
            reuse,
            Arc::ptr_eq(
                &first.http_clients,
                &new_input.remote_plugin_service_config().http_clients,
            ),
        ),
        (vec![true; 220], false)
    );
}

#[tokio::test]
async fn reused_pool_uses_current_endpoint_product_and_authentication() {
    let first_server = MockServer::start().await;
    let second_server = MockServer::start().await;
    for (server, token, account, product_sku) in [
        (&first_server, "header.e30.first", "first-account", "codex"),
        (
            &second_server,
            "header.e30.second",
            "second-account",
            "updated",
        ),
    ] {
        Mock::given(method("GET"))
            .and(path("/backend-api/ps/plugins/suggested/codex"))
            .and(header("authorization", format!("Bearer {token}")))
            .and(header("chatgpt-account-id", account))
            .and(header("oai-product-sku", product_sku))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "enabled": true,
                "plugins": [],
            })))
            .expect(1)
            .mount(server)
            .await;
    }
    let mut input = PluginsConfigInput::new(
        ConfigLayerStack::default(),
        "openai".to_string(),
        /*plugins_enabled*/ true,
        /*remote_plugin_enabled*/ true,
        format!("{}/backend-api", first_server.uri()),
        test_http_client_factory(),
        /*product_sku*/ None,
    );
    let first = input.remote_plugin_service_config();
    let first_auth = CodexAuth::from_external_chatgpt_tokens(
        "header.e30.first",
        "first-account",
        /*chatgpt_plan_type*/ None,
    )
    .unwrap();
    let first_result = fetch_recommended_plugins(&first, Some(&first_auth))
        .await
        .unwrap();

    input.chatgpt_base_url = format!("{}/backend-api", second_server.uri());
    input.product_sku = Some("updated".to_string());
    let second = input.remote_plugin_service_config();
    let second_auth = CodexAuth::from_external_chatgpt_tokens(
        "header.e30.second",
        "second-account",
        /*chatgpt_plan_type*/ None,
    )
    .unwrap();
    let second_result = fetch_recommended_plugins(&second, Some(&second_auth))
        .await
        .unwrap();
    assert_eq!(
        (
            first_result,
            Arc::ptr_eq(&first.http_clients, &second.http_clients),
        ),
        (second_result, true)
    );
}
