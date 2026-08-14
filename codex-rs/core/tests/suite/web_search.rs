#![allow(clippy::unwrap_used)]

use codex_config::test_support::CloudConfigBundleFixture;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_login::auth::BedrockApiKeyAuth;
use codex_model_provider_info::AMAZON_BEDROCK_GPT_5_4_MODEL_ID;
use codex_model_provider_info::AMAZON_BEDROCK_PROVIDER_ID;
use codex_model_provider_info::AMAZON_BEDROCK_RUNTIME_PROVIDER_ID;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::models::PermissionProfile;
use codex_protocol::openai_models::WebSearchToolType;
use core_test_support::responses;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::sync::Arc;

fn find_web_search_tool(body: &Value) -> &Value {
    body["tools"]
        .as_array()
        .expect("request body should include tools array")
        .iter()
        .find(|tool| tool.get("type").and_then(Value::as_str) == Some("web_search"))
        .expect("tools should include a web_search tool")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn web_search_mode_cached_sets_external_web_access_false() {
    skip_if_no_network!();

    let server = start_mock_server().await;
    let sse = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_completed("resp-1"),
    ]);
    let resp_mock = responses::mount_sse_once(&server, sse).await;

    let mut builder = test_codex().with_model("gpt-5.4").with_config(|config| {
        config
            .web_search_mode
            .set(WebSearchMode::Cached)
            .expect("test web_search_mode should satisfy constraints");
    });
    let test = builder
        .build(&server)
        .await
        .expect("create test Codex conversation");

    test.submit_turn_with_permission_profile(
        "hello cached web search",
        PermissionProfile::read_only(),
    )
    .await
    .expect("submit turn");

    let body = resp_mock.single_request().body_json();
    let tool = find_web_search_tool(&body);
    assert_eq!(
        tool.get("external_web_access").and_then(Value::as_bool),
        Some(false),
        "web_search cached mode should force external_web_access=false"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn amazon_bedrock_web_search_uses_text_only_hosted_tools() {
    skip_if_no_network!();

    enum ModelCatalog {
        BuiltIn,
        Configured,
    }

    for (case, configured_web_search_mode, model_catalog) in [
        ("cached by default", None, ModelCatalog::BuiltIn),
        (
            "unsupported explicit live search falls back to cached",
            Some(WebSearchMode::Live),
            ModelCatalog::BuiltIn,
        ),
        (
            "unsupported explicit indexed search falls back to cached",
            Some(WebSearchMode::Indexed),
            ModelCatalog::BuiltIn,
        ),
        ("configured model catalog", None, ModelCatalog::Configured),
    ] {
        let server = start_mock_server().await;
        let resp_mock = responses::mount_sse_once(
            &server,
            responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_completed("resp-1"),
            ]),
        )
        .await;

        let auth = CodexAuth::BedrockApiKey(BedrockApiKeyAuth {
            api_key: "dummy".to_string(),
            region: "us-east-1".to_string(),
        });
        let mut builder = test_codex().with_auth(auth);
        builder = match model_catalog {
            ModelCatalog::BuiltIn => builder.with_model(AMAZON_BEDROCK_GPT_5_4_MODEL_ID),
            ModelCatalog::Configured => builder.with_model_info_override("gpt-5.4", |model_info| {
                model_info.web_search_tool_type = WebSearchToolType::TextAndImage;
            }),
        };
        builder = builder.with_config(move |config| {
            let base_url = config.model_provider.base_url.clone();
            config.model_provider_id = AMAZON_BEDROCK_PROVIDER_ID.to_string();
            config.model_provider =
                ModelProviderInfo::create_amazon_bedrock_provider(/*aws*/ None);
            config.model_provider.base_url = base_url;
            if let Some(mode) = configured_web_search_mode {
                config
                    .web_search_mode
                    .set(mode)
                    .expect("test web search mode should satisfy constraints");
            }
        });
        let test = builder
            .build_with_auto_env(&server)
            .await
            .expect("create test Bedrock conversation");

        test.submit_turn_with_permission_profile(
            "hello Bedrock web search",
            PermissionProfile::Disabled,
        )
        .await
        .expect("submit Bedrock turn");

        let body = resp_mock.single_request().body_json();
        assert_eq!(
            find_web_search_tool(&body),
            &json!({
                "type": "web_search",
                "external_web_access": false,
            }),
            "unexpected Bedrock search tool for {case}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn amazon_bedrock_runtime_preserves_cross_region_models_without_web_search() {
    skip_if_no_network!();

    for model in ["us.openai.gpt-5.6-sol", "global.openai.gpt-5.6-sol"] {
        let server = start_mock_server().await;
        let response = responses::mount_sse_once(
            &server,
            responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_completed("resp-1"),
            ]),
        )
        .await;
        let auth = CodexAuth::BedrockApiKey(BedrockApiKeyAuth {
            api_key: "dummy".to_string(),
            region: "us-east-1".to_string(),
        });
        let mut builder = test_codex()
            .with_auth(auth)
            .with_model(model)
            .with_config(|config| {
                let base_url = config.model_provider.base_url.clone();
                config.model_provider_id = AMAZON_BEDROCK_RUNTIME_PROVIDER_ID.to_string();
                config.model_provider =
                    ModelProviderInfo::create_amazon_bedrock_runtime_provider(/*aws*/ None);
                config.model_provider.base_url = base_url;
                config
                    .web_search_mode
                    .set(WebSearchMode::Cached)
                    .expect("test web search mode should satisfy constraints");
            });
        let test = builder
            .build_with_auto_env(&server)
            .await
            .expect("create test Bedrock Runtime conversation");

        test.submit_turn_with_permission_profile(
            "hello Bedrock Runtime",
            PermissionProfile::Disabled,
        )
        .await
        .expect("submit Bedrock Runtime turn");

        let body = response.single_request().body_json();
        let web_search_tool = body
            .get("tools")
            .and_then(Value::as_array)
            .and_then(|tools| {
                tools
                    .iter()
                    .find(|tool| tool.get("type").and_then(Value::as_str) == Some("web_search"))
            });
        assert_eq!(
            (body["model"].as_str(), web_search_tool),
            (Some(model), None)
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn amazon_bedrock_web_search_is_disabled_when_managed_requirements_prohibit_cached_search() {
    skip_if_no_network!();

    for (allowed_mode, requested_mode) in [
        ("live", WebSearchMode::Live),
        ("indexed", WebSearchMode::Indexed),
    ] {
        let server = start_mock_server().await;
        let resp_mock = responses::mount_sse_once(
            &server,
            responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_completed("resp-1"),
            ]),
        )
        .await;

        let auth = CodexAuth::BedrockApiKey(BedrockApiKeyAuth {
            api_key: "dummy".to_string(),
            region: "us-east-1".to_string(),
        });
        let mut builder = test_codex()
            .with_auth(auth)
            .with_model(AMAZON_BEDROCK_GPT_5_4_MODEL_ID)
            .with_cloud_config_bundle(
                CloudConfigBundleFixture::loader_with_enterprise_requirement(format!(
                    r#"allowed_web_search_modes = ["{allowed_mode}"]"#
                )),
            )
            .with_config(move |config| {
                assert_eq!(config.web_search_mode.value(), requested_mode);
                let base_url = config.model_provider.base_url.clone();
                config.model_provider_id = AMAZON_BEDROCK_PROVIDER_ID.to_string();
                config.model_provider =
                    ModelProviderInfo::create_amazon_bedrock_provider(/*aws*/ None);
                config.model_provider.base_url = base_url;
            });
        let test = builder
            .build_with_auto_env(&server)
            .await
            .expect("create managed Bedrock conversation");

        test.submit_turn_with_permission_profile(
            "hello managed Bedrock web search",
            PermissionProfile::Disabled,
        )
        .await
        .expect("submit managed Bedrock turn");

        let body = resp_mock.single_request().body_json();
        let web_search_tool = body
            .get("tools")
            .and_then(Value::as_array)
            .and_then(|tools| {
                tools
                    .iter()
                    .find(|tool| tool.get("type").and_then(Value::as_str) == Some("web_search"))
            });
        assert_eq!(
            web_search_tool, None,
            "Bedrock search should be disabled when managed requirements only allow {allowed_mode}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn web_search_mode_takes_precedence_over_legacy_flags() {
    skip_if_no_network!();

    let server = start_mock_server().await;
    let sse = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_completed("resp-1"),
    ]);
    let resp_mock = responses::mount_sse_once(&server, sse).await;

    let mut builder = test_codex().with_model("gpt-5.4").with_config(|config| {
        config
            .features
            .enable(Feature::WebSearchRequest)
            .expect("test config should allow feature update");
        config
            .web_search_mode
            .set(WebSearchMode::Cached)
            .expect("test web_search_mode should satisfy constraints");
    });
    let test = builder
        .build(&server)
        .await
        .expect("create test Codex conversation");

    test.submit_turn_with_permission_profile(
        "hello cached+live flags",
        PermissionProfile::read_only(),
    )
    .await
    .expect("submit turn");

    let body = resp_mock.single_request().body_json();
    let tool = find_web_search_tool(&body);
    assert_eq!(
        tool.get("external_web_access").and_then(Value::as_bool),
        Some(false),
        "web_search mode should win over legacy web_search_request"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn web_search_mode_defaults_to_cached_when_features_disabled() {
    skip_if_no_network!();

    let server = start_mock_server().await;
    let sse = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_completed("resp-1"),
    ]);
    let resp_mock = responses::mount_sse_once(&server, sse).await;

    let mut builder = test_codex().with_model("gpt-5.4").with_config(|config| {
        config
            .web_search_mode
            .set(WebSearchMode::Cached)
            .expect("test web_search_mode should satisfy constraints");
        config
            .features
            .disable(Feature::WebSearchCached)
            .expect("test config should allow feature update");
        config
            .features
            .disable(Feature::WebSearchRequest)
            .expect("test config should allow feature update");
    });
    let test = builder
        .build(&server)
        .await
        .expect("create test Codex conversation");

    test.submit_turn_with_permission_profile(
        "hello default cached web search",
        PermissionProfile::read_only(),
    )
    .await
    .expect("submit turn");

    let body = resp_mock.single_request().body_json();
    let tool = find_web_search_tool(&body);
    assert_eq!(
        tool.get("external_web_access").and_then(Value::as_bool),
        Some(false),
        "default web_search should be cached when unset"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn web_search_mode_updates_between_turns_with_permission_profile() {
    skip_if_no_network!();

    let server = start_mock_server().await;
    let resp_mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_completed("resp-1"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("resp-2"),
                responses::ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex().with_model("gpt-5.4").with_config(|config| {
        config
            .web_search_mode
            .set(WebSearchMode::Cached)
            .expect("test web_search_mode should satisfy constraints");
        config
            .features
            .disable(Feature::WebSearchCached)
            .expect("test config should allow feature update");
        config
            .features
            .disable(Feature::WebSearchRequest)
            .expect("test config should allow feature update");
    });
    let test = builder
        .build(&server)
        .await
        .expect("create test Codex conversation");

    test.submit_turn_with_permission_profile("hello cached", PermissionProfile::read_only())
        .await
        .expect("submit first turn");
    test.submit_turn_with_permission_profile("hello live", PermissionProfile::Disabled)
        .await
        .expect("submit second turn");

    let requests = resp_mock.requests();
    assert_eq!(requests.len(), 2, "expected two response requests");

    let first_body = requests[0].body_json();
    let first_tool = find_web_search_tool(&first_body);
    assert_eq!(
        first_tool
            .get("external_web_access")
            .and_then(Value::as_bool),
        Some(false),
        "read-only policy should default web_search to cached"
    );

    let second_body = requests[1].body_json();
    let second_tool = find_web_search_tool(&second_body);
    assert_eq!(
        second_tool
            .get("external_web_access")
            .and_then(Value::as_bool),
        Some(true),
        "danger-full-access policy should default web_search to live"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn web_search_tool_config_from_config_toml_is_forwarded_to_request() {
    skip_if_no_network!();

    let server = start_mock_server().await;
    let sse = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_completed("resp-1"),
    ]);
    let resp_mock = responses::mount_sse_once(&server, sse).await;

    let home = Arc::new(tempfile::TempDir::new().expect("create codex home"));
    std::fs::write(
        home.path().join("config.toml"),
        r#"web_search = "live"

[tools.web_search]
context_size = "high"
allowed_domains = ["example.com"]
location = { country = "US", city = "New York", timezone = "America/New_York" }
"#,
    )
    .expect("write config.toml");

    let mut builder = test_codex().with_model("gpt-5.2").with_home(home);
    let test = builder
        .build(&server)
        .await
        .expect("create test Codex conversation");

    test.submit_turn_with_permission_profile(
        "hello configured web search",
        PermissionProfile::Disabled,
    )
    .await
    .expect("submit turn");

    let body = resp_mock.single_request().body_json();
    let tool = find_web_search_tool(&body);
    assert_eq!(
        tool,
        &json!({
            "type": "web_search",
            "external_web_access": true,
            "search_context_size": "high",
            "filters": {
                "allowed_domains": ["example.com"],
            },
            "user_location": {
                "type": "approximate",
                "country": "US",
                "city": "New York",
                "timezone": "America/New_York",
            },
        })
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn indexed_web_search_mode_sets_indexed_access() {
    skip_if_no_network!();

    let server = start_mock_server().await;
    let sse = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_completed("resp-1"),
    ]);
    let resp_mock = responses::mount_sse_once(&server, sse).await;

    let home = Arc::new(tempfile::TempDir::new().expect("create codex home"));
    std::fs::write(home.path().join("config.toml"), r#"web_search = "indexed""#)
        .expect("write config.toml");

    let mut builder = test_codex().with_model("gpt-5.2").with_home(home);
    let test = builder
        .build(&server)
        .await
        .expect("create test Codex conversation");

    test.submit_turn_with_permission_profile(
        "hello indexed web search",
        PermissionProfile::Disabled,
    )
    .await
    .expect("submit turn");

    let body = resp_mock.single_request().body_json();
    let tool = find_web_search_tool(&body);
    assert_eq!(
        (
            tool.get("external_web_access").and_then(Value::as_bool),
            tool.get("indexed_web_access").and_then(Value::as_bool),
        ),
        (Some(true), Some(true))
    );
}
