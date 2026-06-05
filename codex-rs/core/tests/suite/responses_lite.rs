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
use serde_json::Value;

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
    let auth_manager = codex_core::test_support::auth_manager_from_auth(auth.clone());
    let mut extension_builder = ExtensionRegistryBuilder::<Config>::new();
    install_web_search_extension(&mut extension_builder, Arc::clone(&auth_manager));
    install_image_generation_extension(&mut extension_builder, auth_manager);
    let extensions: Arc<ExtensionRegistry<Config>> = Arc::new(extension_builder.build());

    let mut builder = test_codex()
        .with_auth(auth)
        .with_extensions(extensions)
        .with_model_info_override("gpt-5.4", |model_info| {
            model_info.use_responses_lite = true;
            model_info.input_modalities = vec![InputModality::Text, InputModality::Image];
        })
        .with_config(|config| {
            config
                .web_search_mode
                .set(WebSearchMode::Live)
                .expect("live web search should satisfy test constraints");
            config
                .features
                .disable(Feature::StandaloneWebSearch)
                .expect("standalone web search should not be constrained");
            config
                .features
                .disable(Feature::ImageGeneration)
                .expect("image generation should not be constrained");
            config
                .features
                .disable(Feature::ImageGenExt)
                .expect("image generation extension should not be constrained");
        });
    let test = builder.build(&server).await?;

    test.submit_turn("Use standalone tools").await?;

    let request = response_mock.single_request();
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
    assert!(
        !tools.iter().any(|tool| {
            matches!(
                tool.get("type").and_then(Value::as_str),
                Some("web_search" | "image_generation")
            )
        }),
        "Responses Lite should omit hosted Responses tools"
    );

    Ok(())
}
