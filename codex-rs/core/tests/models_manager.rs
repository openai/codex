use codex_core::WireApi;
use codex_core::create_oss_provider_with_base_url;
use codex_core::openai_models::models_manager::ModelsManager;
use codex_protocol::openai_models::ClientVersion;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::ReasoningLevel;
use codex_protocol::openai_models::ShellType;
use core_test_support::responses::mount_models_once;
use core_test_support::responses::start_mock_server;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn fetches_models_via_models_endpoint() {
    let server = start_mock_server().await;
    let body = ModelsResponse {
        models: vec![ModelInfo {
            slug: "gpt-test".to_string(),
            display_name: "gpt-test".to_string(),
            description: Some("desc".to_string()),
            default_reasoning_level: ReasoningLevel::Medium,
            supported_reasoning_levels: vec![
                ReasoningLevel::Low,
                ReasoningLevel::Medium,
                ReasoningLevel::High,
            ],
            shell_type: ShellType::ShellCommand,
            visibility: ModelVisibility::List,
            minimal_client_version: ClientVersion(0, 99, 0),
            supported_in_api: true,
            priority: 1,
        }],
    };
    let models_mock = mount_models_once(&server, body.clone()).await;

    let base_url = format!("{}/api/codex", server.uri());
    let provider = create_oss_provider_with_base_url(&base_url, WireApi::Responses);
    let manager = ModelsManager::new(None);

    let models = manager
        .fetch_models_from_api(&provider, None)
        .await
        .expect("fetch models");

    assert_eq!(models, body.models);

    let request = models_mock
        .requests()
        .into_iter()
        .next()
        .expect("models request captured");
    assert_eq!(request.url.path(), "/api/codex/models");

    let client_version = request
        .url
        .query_pairs()
        .find(|(k, _)| k == "client_version")
        .map(|(_, v)| v.to_string())
        .expect("client_version query param");
    assert_eq!(client_version, env!("CARGO_PKG_VERSION"));
}
