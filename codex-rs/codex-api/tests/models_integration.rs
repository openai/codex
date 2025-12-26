use codex_api::AuthProvider;
use codex_api::ModelsClient;
use codex_api::provider::Provider;
use codex_api::provider::RetryConfig;
use codex_api::provider::WireApi;
use codex_client::ReqwestTransport;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;
use codex_protocol::openai_models::TruncationPolicyConfig;
use http::HeaderMap;
use http::Method;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[derive(Clone, Default)]
struct DummyAuth;

impl AuthProvider for DummyAuth {
    fn bearer_token(&self) -> Option<String> {
        None
    }
}

fn provider(base_url: &str) -> Provider {
    Provider {
        name: "test".to_string(),
        base_url: base_url.to_string(),
        query_params: None,
        wire: WireApi::Responses,
        headers: HeaderMap::new(),
        retry: RetryConfig {
            max_attempts: 1,
            base_delay: std::time::Duration::from_millis(1),
            retry_429: false,
            retry_5xx: true,
            retry_transport: true,
        },
        stream_idle_timeout: std::time::Duration::from_secs(1),
    }
}

#[tokio::test]
async fn models_client_hits_models_endpoint() {
    let server = MockServer::start().await;
    let base_url = format!("{}/api/codex", server.uri());

    let response = ModelsResponse {
        models: vec![ModelInfo {
            slug: "gpt-test".to_string(),
            display_name: "gpt-test".to_string(),
            description: Some("desc".to_string()),
            default_reasoning_level: ReasoningEffort::Medium,
            supported_reasoning_levels: vec![
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Low,
                    description: ReasoningEffort::Low.to_string(),
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Medium,
                    description: ReasoningEffort::Medium.to_string(),
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::High,
                    description: ReasoningEffort::High.to_string(),
                },
            ],
            shell_type: ConfigShellToolType::ShellCommand,
            visibility: ModelVisibility::List,
            supported_in_api: true,
            priority: 1,
            upgrade: None,
            base_instructions: None,
            supports_reasoning_summaries: false,
            support_verbosity: false,
            default_verbosity: None,
            apply_patch_tool_type: None,
            truncation_policy: TruncationPolicyConfig::bytes(10_000),
            supports_parallel_tool_calls: false,
            context_window: None,
            experimental_supported_tools: Vec::new(),
        }],
        etag: String::new(),
    };

    Mock::given(method("GET"))
        .and(path("/api/codex/models"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_json(&response),
        )
        .mount(&server)
        .await;

    let transport = ReqwestTransport::new(reqwest::Client::new());
    let client = ModelsClient::new(transport, provider(&base_url), DummyAuth);

    let result = client
        .list_models("0.1.0", HeaderMap::new())
        .await
        .expect("models request should succeed");

    assert_eq!(result.models.len(), 1);
    assert_eq!(result.models[0].slug, "gpt-test");

    let received = server
        .received_requests()
        .await
        .expect("should capture requests");
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].method, Method::GET.as_str());
    assert_eq!(received[0].url.path(), "/api/codex/models");
}
