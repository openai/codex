use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use codex_extension_api::ConversationHistorySnapshot;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ResponseItem;
use codex_extension_api::ToolCallSource;
use codex_extension_api::ToolName;
use codex_extension_api::ToolStartInput;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_login::AgentIdentityAuthPolicy;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_model_provider::create_model_provider;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::protocol::SessionSource;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use serde_json::json;

use crate::LunaSampler;
use crate::LunaSamplerConfig;

struct EmptyConversationHistory;

impl ConversationHistorySnapshot for EmptyConversationHistory {
    fn items(&self) -> Box<dyn Iterator<Item = &ResponseItem> + Send + '_> {
        Box::new(std::iter::empty())
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contributor_samples_tool_calls_with_the_existing_luna_pool() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let events = vec![
        ev_assistant_message("sample", r#"{"scores":{"action_risk":0.25}}"#),
        ev_completed("response-1"),
    ];
    let server = responses::start_websocket_server(vec![Vec::new(), vec![events]]).await;
    let provider_info = ModelProviderInfo::create_openai_provider(Some(format!(
        "http://{}/v1",
        server.uri().trim_start_matches("ws://")
    )));
    let sampler = LunaSampler::connect(LunaSamplerConfig {
        provider: create_model_provider(
            provider_info,
            Some(AuthManager::from_auth_for_testing(CodexAuth::from_api_key(
                "test-api-key",
            ))),
        ),
        http_client_factory: HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        agent_identity_policy: AgentIdentityAuthPolicy::JwtOnly,
        session_source: SessionSource::Exec,
        session_id: "session-1".to_owned(),
        thread_id: "thread-1".to_owned(),
        originator: None,
        service_tier: None,
    })
    .await?;
    let mut builder = ExtensionRegistryBuilder::<()>::new();
    crate::install(&mut builder, Arc::new(sampler));
    let registry = builder.build();
    let session_store = ExtensionData::new("session-1");
    let thread_store = ExtensionData::new("thread-1");
    let turn_store = ExtensionData::new("turn-1");
    let tool_name = ToolName::plain("read_file");

    registry.tool_lifecycle_contributors()[0]
        .on_tool_start(ToolStartInput {
            session_store: &session_store,
            thread_store: &thread_store,
            turn_store: &turn_store,
            turn_id: "turn-1",
            call_id: "call-1",
            tool_name: &tool_name,
            conversation_history: Arc::new(EmptyConversationHistory),
            source: ToolCallSource::Direct,
        })
        .await;

    let request = tokio::time::timeout(
        Duration::from_secs(5),
        server.wait_for_request(/*connection_index*/ 1, /*request_index*/ 0),
    )
    .await?;
    let request = request.body_json();
    assert_eq!(request["model"], "gpt-5.6-luna");
    assert_eq!(request["client_metadata"]["thread_id"], "thread-1");
    assert_eq!(request["client_metadata"]["turn_id"], "turn-1");
    assert_eq!(request["reasoning"]["effort"], "low");
    assert_eq!(request["text"]["format"]["strict"], true);
    assert_eq!(
        request["text"]["format"]["schema"]["properties"]["scores"]["properties"]["action_risk"],
        json!({"type": "number", "minimum": 0.0, "maximum": 1.0})
    );
    assert_eq!(
        request["input"][2]["content"][0]["text"],
        "Tool: read_file\nCall ID: call-1"
    );

    Ok(())
}
