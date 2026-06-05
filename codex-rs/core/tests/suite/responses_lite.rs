use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use codex_core::config::Config;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistry;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolContributor;
use codex_extension_api::ToolExecutor;
use codex_extension_api::ToolName;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_models_manager::bundled_models_response;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::openai_models::InputModality;
use codex_tools::JsonToolOutput;
use codex_tools::ResponsesApiNamespace;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolOutput;
use codex_tools::ToolSpec;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use serde_json::Value;
use serde_json::json;

struct ResponsesLiteToolContributor;

impl ToolContributor for ResponsesLiteToolContributor {
    fn tools(
        &self,
        _session_store: &ExtensionData,
        _thread_store: &ExtensionData,
    ) -> Vec<Arc<dyn ToolExecutor<ToolCall>>> {
        vec![
            Arc::new(TestNamespacedTool::new("web", "run")),
            Arc::new(TestNamespacedTool::new("image_gen", "imagegen")),
        ]
    }
}

struct TestNamespacedTool {
    namespace: &'static str,
    name: &'static str,
}

impl TestNamespacedTool {
    fn new(namespace: &'static str, name: &'static str) -> Self {
        Self { namespace, name }
    }
}

#[async_trait::async_trait]
impl ToolExecutor<ToolCall> for TestNamespacedTool {
    fn tool_name(&self) -> ToolName {
        ToolName::namespaced(self.namespace, self.name)
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec::Namespace(ResponsesApiNamespace {
            name: self.namespace.to_string(),
            description: format!("Test {} namespace.", self.namespace),
            tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                name: self.name.to_string(),
                description: format!("Test {} tool.", self.name),
                strict: false,
                defer_loading: None,
                parameters: codex_tools::JsonSchema::default(),
                output_schema: None,
            })],
        })
    }

    async fn handle(
        &self,
        _call: ToolCall,
    ) -> Result<Box<dyn ToolOutput>, codex_tools::FunctionCallError> {
        Ok(Box::new(JsonToolOutput::new(json!({}))))
    }
}

fn responses_lite_test_extensions() -> Arc<ExtensionRegistry<Config>> {
    let mut builder = ExtensionRegistryBuilder::new();
    builder.tool_contributor(Arc::new(ResponsesLiteToolContributor));
    Arc::new(builder.build())
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

    let mut model_catalog = bundled_models_response()
        .unwrap_or_else(|err| panic!("bundled models.json should parse: {err}"));
    let model = model_catalog
        .models
        .iter_mut()
        .find(|model| model.slug == "gpt-5.4")
        .context("gpt-5.4 should exist in bundled models.json")?;
    model.use_responses_lite = true;
    model.input_modalities = vec![InputModality::Text, InputModality::Image];

    let mut builder = test_codex()
        .with_model("gpt-5.4")
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_extensions(responses_lite_test_extensions())
        .with_config(move |config| {
            config.model_catalog = Some(model_catalog);
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
