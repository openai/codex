//! Standalone web search must resolve the configured route for every redirect destination.

use anyhow::Result;
use codex_core::config::Config;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_features::Feature;
use codex_http_client::cache_system_proxy_route_for_test;
use codex_login::CodexAuth;
use codex_models_manager::bundled_models_response;
use codex_protocol::config_types::WebSearchMode;
use codex_web_search_extension::install as install_web_search_extension;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use http::HeaderMap;
use http::StatusCode;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

const TEST_NAME: &str =
    "suite::web_search_system_proxy::standalone_web_search_resolves_redirect_routes";
const TEST_SUBPROCESS_ENV_VAR: &str = "CODEX_WEB_SEARCH_SYSTEM_PROXY_TEST_SUBPROCESS";
const API_BASE_URL: &str = "http://web-search-proxy.invalid/v1";
const REDIRECT_URL: &str = "http://web-search-redirect.invalid/v1/alpha/search";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn standalone_web_search_resolves_redirect_routes() -> Result<()> {
    skip_if_no_network!(Ok(()));

    if std::env::var_os(TEST_SUBPROCESS_ENV_VAR).is_none() {
        // Isolate the proxy cache and remove environment proxies so only the configured
        // system proxy route can reach the fake API origin.
        let mut command = Command::new(std::env::current_exe()?);
        command.arg("--exact").arg(TEST_NAME);
        for key in codex_network_proxy::PROXY_ENV_KEYS {
            command.env_remove(key);
        }
        let output = timeout(
            Duration::from_secs(120),
            command
                .kill_on_drop(true)
                .env(TEST_SUBPROCESS_ENV_VAR, "1")
                .output(),
        )
        .await??;
        assert!(
            output.status.success(),
            "subprocess test `{TEST_NAME}` failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        return Ok(());
    }

    let proxy = responses::start_mock_server().await;
    let redirect_proxy = responses::start_mock_server().await;
    Mock::given(method("POST"))
        .and(path("/v1/alpha/search"))
        .and(header("host", "web-search-proxy.invalid"))
        .respond_with(
            ResponseTemplate::new(StatusCode::TEMPORARY_REDIRECT.as_u16())
                .insert_header("location", REDIRECT_URL),
        )
        .expect(/*r*/ 1)
        .mount(&proxy)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/alpha/search"))
        .and(header("host", "web-search-redirect.invalid"))
        .respond_with(
            ResponseTemplate::new(StatusCode::OK.as_u16()).set_body_json(json!({
                "output": "Search result through system proxy",
            })),
        )
        .expect(/*r*/ 1)
        .mount(&redirect_proxy)
        .await;
    responses::mount_sse_once(
        &proxy,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_function_call_with_namespace(
                "web-run-1",
                "web",
                "run",
                &json!({"search_query": [{"q": "standalone web search"}]}).to_string(),
            ),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    let follow_up_mock = responses::mount_sse_once(
        &proxy,
        responses::sse(vec![
            responses::ev_assistant_message("msg-1", "done"),
            responses::ev_completed("resp-2"),
        ]),
    )
    .await;

    let auth = CodexAuth::from_api_key("dummy");
    let auth_manager = codex_core::test_support::auth_manager_from_auth(auth.clone());
    let mut extension_builder = ExtensionRegistryBuilder::<Config>::new();
    install_web_search_extension(&mut extension_builder, auth_manager);
    let mut builder = test_codex()
        .with_auth(auth)
        .with_extensions(Arc::new(extension_builder.build()))
        .with_config(|config| {
            config.model_catalog =
                Some(bundled_models_response().expect("bundled models.json should parse"));
            config.model_provider.base_url = Some(API_BASE_URL.to_string());
            for feature in [Feature::RespectSystemProxy, Feature::StandaloneWebSearch] {
                config
                    .features
                    .enable(feature)
                    .expect("test config should allow feature update");
            }
            config.respect_system_proxy = true;
            config
                .web_search_mode
                .set(WebSearchMode::Live)
                .expect("web search mode should be accepted");
        });
    let test = builder.build_with_auto_env(&proxy).await?;

    for endpoint in ["responses", "alpha/search"] {
        cache_system_proxy_route_for_test(&format!("{API_BASE_URL}/{endpoint}"), proxy.uri());
    }
    cache_system_proxy_route_for_test(REDIRECT_URL, redirect_proxy.uri());
    test.submit_turn("search the web through the system proxy")
        .await?;

    let requests = proxy
        .received_requests()
        .await
        .expect("initial proxy requests");
    let initial_search = requests
        .iter()
        .find(|request| request.url.path() == "/v1/alpha/search")
        .expect("initial proxy should receive search");
    let redirected_requests = redirect_proxy
        .received_requests()
        .await
        .expect("redirect proxy requests");
    let redirected_search = redirected_requests
        .first()
        .expect("redirect proxy should receive search");
    assert_eq!(initial_search.body, redirected_search.body);
    let default_headers = codex_login::default_client::default_headers();
    let observed_default_headers = [initial_search, redirected_search].map(|request| {
        default_headers
            .keys()
            .filter_map(|name| {
                request
                    .headers
                    .get(name)
                    .map(|value| (name.clone(), value.clone()))
            })
            .collect::<HeaderMap>()
    });
    assert_eq!(
        observed_default_headers,
        [default_headers.clone(), default_headers]
    );
    assert_eq!(
        follow_up_mock
            .single_request()
            .function_call_output_content_and_success("web-run-1"),
        Some((Some("Search result through system proxy".to_string()), None))
    );

    Ok(())
}
