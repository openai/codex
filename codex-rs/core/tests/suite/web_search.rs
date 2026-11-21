#![allow(clippy::unwrap_used)]

use codex_config::test_support::CloudConfigBundleFixture;
use codex_core::ConfigRefreshOutcome;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_login::auth::BedrockApiKeyAuth;
use codex_model_provider_info::AMAZON_BEDROCK_GPT_5_5_MODEL_ID;
use codex_model_provider_info::AMAZON_BEDROCK_PROVIDER_ID;
use codex_model_provider_info::AMAZON_BEDROCK_RUNTIME_PROVIDER_ID;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::models::PermissionProfile;
use codex_protocol::openai_models::WebSearchToolType;
use codex_utils_absolute_path::AbsolutePathBuf;
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

    for (case, config_toml, model_catalog) in [
        ("cached by default", "", ModelCatalog::BuiltIn),
        (
            "unsupported explicit live search falls back to cached",
            "web_search = \"live\"",
            ModelCatalog::BuiltIn,
        ),
        (
            "unsupported explicit indexed search falls back to cached",
            "web_search = \"indexed\"",
            ModelCatalog::BuiltIn,
        ),
        (
            "unsupported implicit indexed search falls back to cached",
            "[features]\nweb_search_indexed_default = true",
            ModelCatalog::BuiltIn,
        ),
        ("configured model catalog", "", ModelCatalog::Configured),
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
            .with_pre_build_hook(move |codex_home| {
                std::fs::write(codex_home.join("config.toml"), config_toml)
                    .expect("write web search configuration");
            });
        builder = match model_catalog {
            ModelCatalog::BuiltIn => builder.with_model(AMAZON_BEDROCK_GPT_5_5_MODEL_ID),
            ModelCatalog::Configured => builder.with_model_info_override("gpt-5.5", |model_info| {
                model_info.web_search_tool_type = WebSearchToolType::TextAndImage;
            }),
        };
        builder = builder.with_config(move |config| {
            let base_url = config.model_provider.base_url.clone();
            config.model_provider_id = AMAZON_BEDROCK_PROVIDER_ID.to_string();
            config.model_provider =
                ModelProviderInfo::create_amazon_bedrock_provider(/*aws*/ None);
            config.model_provider.base_url = base_url;
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
            .with_model(AMAZON_BEDROCK_GPT_5_5_MODEL_ID)
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
async fn default_web_search_rollout_respects_permissions_and_survives_refresh() -> anyhow::Result<()>
{
    skip_if_no_network!(Ok(()));

    for (enabled, restricted_tool) in [
        (
            false,
            json!({"type": "web_search", "external_web_access": false}),
        ),
        (
            true,
            json!({
                "type": "web_search",
                "external_web_access": true,
                "indexed_web_access": true,
            }),
        ),
    ] {
        let server = start_mock_server().await;
        let responses = responses::mount_sse_sequence(
            &server,
            vec![
                responses::sse_completed("restricted"),
                responses::sse_completed("full-access"),
                responses::sse_completed("after-refresh"),
            ],
        )
        .await;
        let home = Arc::new(tempfile::TempDir::new()?);
        let config_path = AbsolutePathBuf::from_absolute_path(home.path().join("config.toml"))?;
        std::fs::write(
            &config_path,
            format!("[features]\nweb_search_indexed_default = {enabled}\n"),
        )?;
        let test = test_codex()
            .with_home(home)
            .with_model("gpt-5.4")
            .with_cloud_config_bundle(
                CloudConfigBundleFixture::loader_with_enterprise_requirement(
                    r#"allowed_web_search_modes = ["cached", "indexed", "live"]"#,
                ),
            )
            .build_with_auto_env(&server)
            .await?;

        test.submit_turn_with_permission_profile("restricted", PermissionProfile::read_only())
            .await?;
        test.submit_turn_with_permission_profile("full access", PermissionProfile::Disabled)
            .await?;

        let current_config = test.codex.config().await;
        let mut refreshed_config = current_config.as_ref().clone();
        refreshed_config
            .features
            .set_enabled(Feature::WebSearchIndexedDefault, !enabled)?;
        refreshed_config.web_search_mode.set(if enabled {
            WebSearchMode::Cached
        } else {
            WebSearchMode::Indexed
        })?;
        refreshed_config.config_layer_stack =
            refreshed_config.config_layer_stack.with_user_config(
                &config_path,
                toml::from_str(&format!(
                    "[features]\nweb_search_indexed_default = {}\n",
                    !enabled
                ))?,
            )?;
        assert_eq!(
            test.codex
                .refresh_runtime_config(current_config, refreshed_config)
                .await,
            ConfigRefreshOutcome::Published,
        );
        test.submit_turn_with_permission_profile("after refresh", PermissionProfile::read_only())
            .await?;

        let requests = responses.requests();
        let tools: Vec<_> = requests
            .iter()
            .map(|request| find_web_search_tool(&request.body_json()).clone())
            .collect();
        assert_eq!(
            tools,
            vec![
                restricted_tool.clone(),
                json!({"type": "web_search", "external_web_access": true}),
                restricted_tool,
            ]
        );
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn web_search_rollout_preserves_explicit_modes_and_full_access_semantics()
-> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    for (mode, expected) in [
        ("disabled", None),
        (
            "cached",
            Some(json!({"type": "web_search", "external_web_access": false})),
        ),
        (
            "live",
            Some(json!({"type": "web_search", "external_web_access": true})),
        ),
        (
            "indexed",
            Some(json!({
                "type": "web_search",
                "external_web_access": true,
                "indexed_web_access": true,
            })),
        ),
    ] {
        let server = start_mock_server().await;
        let response = responses::mount_sse_sequence(
            &server,
            vec![
                responses::sse_completed("explicit-restricted"),
                responses::sse_completed("explicit-full-access"),
            ],
        )
        .await;
        let home = Arc::new(tempfile::TempDir::new()?);
        std::fs::write(
            home.path().join("config.toml"),
            format!("web_search = \"{mode}\"\n[features]\nweb_search_indexed_default = true\n",),
        )?;
        let test = test_codex()
            .with_home(home)
            .with_model("gpt-5.4")
            .build_with_auto_env(&server)
            .await?;
        test.submit_turn_with_permission_profile("restricted", PermissionProfile::read_only())
            .await?;
        test.submit_turn_with_permission_profile("full access", PermissionProfile::Disabled)
            .await?;
        let tools: Vec<_> = response
            .requests()
            .iter()
            .map(|request| {
                request.body_json()["tools"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|tool| tool["type"] == "web_search")
                    .cloned()
            })
            .collect();
        let full_access_expected = if mode == "cached" {
            Some(json!({"type": "web_search", "external_web_access": true}))
        } else {
            expected.clone()
        };
        assert_eq!(
            tools,
            vec![expected, full_access_expected],
            "web_search={mode}"
        );
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn default_web_search_rollout_respects_managed_constraints() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    for (enabled, requested_search_mode, requirements, effective_profile, expect_indexed) in [
        (
            true,
            None,
            "allowed_web_search_modes = [\"cached\", \"indexed\"]",
            PermissionProfile::Disabled,
            true,
        ),
        (
            true,
            None,
            "allowed_web_search_modes = [\"cached\", \"indexed\"]\n[features]\nweb_search_indexed_default = false",
            PermissionProfile::Disabled,
            false,
        ),
        (
            true,
            None,
            "allowed_web_search_modes = [\"cached\"]",
            PermissionProfile::Disabled,
            false,
        ),
        (
            false,
            None,
            "allowed_web_search_modes = [\"indexed\", \"live\"]",
            PermissionProfile::Disabled,
            true,
        ),
        (
            true,
            Some("live"),
            "allowed_web_search_modes = [\"cached\", \"indexed\"]",
            PermissionProfile::Disabled,
            false,
        ),
        (
            true,
            None,
            "allowed_sandbox_modes = [\"read-only\"]",
            PermissionProfile::read_only(),
            true,
        ),
    ] {
        let server = start_mock_server().await;
        let response =
            responses::mount_sse_once(&server, responses::sse_completed("managed")).await;
        let home = Arc::new(tempfile::TempDir::new()?);
        let mut user_config = String::from("sandbox_mode = \"danger-full-access\"\n");
        if let Some(mode) = requested_search_mode {
            user_config.push_str(&format!("web_search = \"{mode}\"\n"));
        }
        user_config.push_str(&format!(
            "[features]\nweb_search_indexed_default = {enabled}\n"
        ));
        std::fs::write(home.path().join("config.toml"), user_config)?;
        let test = test_codex()
            .with_home(home)
            .with_model("gpt-5.4")
            .with_cloud_config_bundle(
                CloudConfigBundleFixture::loader_with_enterprise_requirement(requirements),
            )
            .build_with_auto_env(&server)
            .await?;
        assert_eq!(
            test.codex
                .config()
                .await
                .permissions
                .effective_permission_profile(),
            effective_profile,
        );
        test.submit_text_turn("managed").await?;

        let tool = if expect_indexed {
            json!({
                "type": "web_search",
                "external_web_access": true,
                "indexed_web_access": true,
            })
        } else {
            json!({"type": "web_search", "external_web_access": false})
        };
        assert_eq!(
            find_web_search_tool(&response.single_request().body_json()),
            &tool
        );
    }
    Ok(())
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
async fn indexed_web_search_mode_sets_indexed_access() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let resp_mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse_completed("before-refresh"),
            responses::sse_completed("after-refresh"),
        ],
    )
    .await;
    let home = Arc::new(tempfile::TempDir::new()?);
    let config_path = AbsolutePathBuf::from_absolute_path(home.path().join("config.toml"))?;
    std::fs::write(&config_path, "web_search = \"indexed\"\n")?;
    let test = test_codex()
        .with_home(home)
        .with_model("gpt-5.2")
        .build_with_auto_env(&server)
        .await?;

    test.submit_turn_with_permission_profile("before refresh", PermissionProfile::Disabled)
        .await?;

    // Runtime refreshes replace the user layer, but web search stays session-static.
    let current_config = test.codex.config().await;
    let mut refreshed_config = current_config.as_ref().clone();
    refreshed_config.web_search_mode_was_explicit = false;
    refreshed_config
        .web_search_mode
        .set(WebSearchMode::Cached)?;
    refreshed_config.config_layer_stack = refreshed_config
        .config_layer_stack
        .with_user_config(&config_path, toml::Value::Table(Default::default()))?;
    assert_eq!(
        test.codex
            .refresh_runtime_config(current_config, refreshed_config)
            .await,
        ConfigRefreshOutcome::Published,
    );

    test.submit_turn_with_permission_profile("after refresh", PermissionProfile::Disabled)
        .await?;

    let requests = resp_mock.requests();
    assert_eq!(requests.len(), 2);
    for request in requests {
        assert_eq!(
            find_web_search_tool(&request.body_json()),
            &json!({
                "type": "web_search",
                "external_web_access": true,
                "indexed_web_access": true,
            }),
        );
    }
    Ok(())
}
