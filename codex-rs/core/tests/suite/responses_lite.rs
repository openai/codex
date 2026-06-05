use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use codex_core::config::Config;
use codex_extension_api::ExtensionRegistry;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_features::Feature;
use codex_image_generation_extension::install as install_image_generation_extension;
use codex_login::CodexAuth;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::openai_models::InputModality;
use codex_web_search_extension::install as install_web_search_extension;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::Value;

const RESPONSES_LITE_HEADER: &str = "x-openai-internal-codex-responses-lite";

fn responses_extensions(auth: &CodexAuth) -> Arc<ExtensionRegistry<Config>> {
    let auth_manager = codex_core::test_support::auth_manager_from_auth(auth.clone());
    let mut extension_builder = ExtensionRegistryBuilder::<Config>::new();
    install_web_search_extension(&mut extension_builder, Arc::clone(&auth_manager));
    install_image_generation_extension(&mut extension_builder, auth_manager);
    Arc::new(extension_builder.build())
}

fn configure_responses_tools(config: &mut Config) {
    assert!(config.web_search_mode.set(WebSearchMode::Live).is_ok());
    assert!(
        config
            .features
            .disable(Feature::StandaloneWebSearch)
            .is_ok()
    );
    assert!(config.features.enable(Feature::ImageGeneration).is_ok());
    assert!(config.features.disable(Feature::ImageGenExt).is_ok());
}

fn configure_image_capable_model(model_info: &mut codex_protocol::openai_models::ModelInfo) {
    model_info.input_modalities = vec![InputModality::Text, InputModality::Image];
}

fn has_hosted_tool(tools: &[Value], tool_type: &str) -> bool {
    tools
        .iter()
        .any(|tool| tool.get("type").and_then(Value::as_str) == Some(tool_type))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_lite_uses_standalone_web_search_and_image_generation() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;

    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let extensions = responses_extensions(&auth);

    let mut builder = test_codex()
        .with_auth(auth)
        .with_extensions(extensions)
        .with_model_info_override("gpt-5.4", |model_info| {
            model_info.use_responses_lite = true;
            configure_image_capable_model(model_info);
        })
        .with_config(configure_responses_tools);
    let test = builder.build(&server).await?;

    test.submit_turn("Use standalone tools").await?;

    let request = response_mock.single_request();
    assert_eq!(
        request.header(RESPONSES_LITE_HEADER).as_deref(),
        Some("true")
    );
    request
        .tool_by_name("web", "run")
        .context("Responses Lite should expose standalone web search")?;
    request
        .tool_by_name("image_gen", "imagegen")
        .context("Responses Lite should expose standalone image generation")?;

    let body = request.body_json();
    let tools = body["tools"]
        .as_array()
        .context("Responses request tools should be an array")?;
    assert!(!has_hosted_tool(tools, "web_search"));
    assert!(!has_hosted_tool(tools, "image_generation"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_lite_omits_hosted_tools_without_standalone_extensions() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;

    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_model_info_override("gpt-5.4", |model_info| {
            model_info.use_responses_lite = true;
            configure_image_capable_model(model_info);
        })
        .with_config(configure_responses_tools);
    let test = builder.build(&server).await?;

    test.submit_turn("Do not use hosted tools").await?;

    let body = response_mock.single_request().body_json();
    let tools = body["tools"]
        .as_array()
        .context("Responses request tools should be an array")?;
    assert!(!has_hosted_tool(tools, "web_search"));
    assert!(!has_hosted_tool(tools, "image_generation"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn non_lite_uses_hosted_tools_when_standalone_features_are_disabled() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;

    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let extensions = responses_extensions(&auth);
    let mut builder = test_codex()
        .with_auth(auth)
        .with_extensions(extensions)
        .with_model_info_override("gpt-5.4", configure_image_capable_model)
        .with_config(configure_responses_tools);
    let test = builder.build(&server).await?;

    test.submit_turn("Use hosted tools").await?;

    let request = response_mock.single_request();
    assert_eq!(request.header(RESPONSES_LITE_HEADER), None);
    assert!(request.tool_by_name("web", "run").is_none());
    assert!(request.tool_by_name("image_gen", "imagegen").is_none());
    let body = request.body_json();
    let tools = body["tools"]
        .as_array()
        .context("Responses request tools should be an array")?;
    assert!(has_hosted_tool(tools, "web_search"));
    assert!(has_hosted_tool(tools, "image_generation"));

    Ok(())
}
