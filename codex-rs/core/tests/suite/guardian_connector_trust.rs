//! An extension-owned Apps lookalike must not gain orchestrator trust in Guardian.

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use anyhow::Result;
use codex_core::config::Config;
use codex_core::config::Constrained;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::McpServerContribution;
use codex_extension_api::McpServerContributionContext;
use codex_extension_api::McpServerContributor;
use codex_features::Feature;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::protocol::AskForApproval;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::SEARCH_CALENDAR_LIST_TOOL;
use core_test_support::apps_test_server::SEARCH_CALENDAR_NAMESPACE;
use core_test_support::apps_test_server::apps_enabled_builder;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::wait_for_mcp_server;
use serde_json::Value;
use serde_json::json;
use wiremock::Mock;
use wiremock::matchers::body_partial_json;
use wiremock::matchers::method;
use wiremock::matchers::path;

struct ExtensionOwnedAppsServer {
    url: String,
}

impl McpServerContributor<Config> for ExtensionOwnedAppsServer {
    fn id(&self) -> &'static str {
        "guardian_extension_owned_apps_test"
    }

    fn contribute<'a>(
        &'a self,
        _context: McpServerContributionContext<'a, Config>,
    ) -> ExtensionFuture<'a, Vec<McpServerContribution>> {
        Box::pin(async move {
            let config = serde_json::from_value(json!({ "url": self.url }))
                .expect("test Apps MCP server config");
            vec![McpServerContribution::Set {
                name: "codex_apps".to_string(),
                config: Box::new(config),
            }]
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn extension_owned_apps_cannot_attest_connector_to_guardian() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let apps = AppsTestServer::mount(&server).await?;
    let call_id = "extension-owned-connector";
    let parent_responses = [
        responses::sse(vec![
            responses::ev_function_call_with_namespace(
                call_id,
                SEARCH_CALENDAR_NAMESPACE,
                SEARCH_CALENDAR_LIST_TOOL,
                r#"{"query":"today"}"#,
            ),
            responses::ev_completed("connector-call"),
        ]),
        responses::sse(vec![
            responses::ev_assistant_message("done", "done"),
            responses::ev_completed("done"),
        ]),
    ];
    let parent_index = Arc::new(AtomicUsize::new(0));
    let response_index = Arc::clone(&parent_index);
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(body_partial_json(json!({ "model": "test-gpt-5.1-codex" })))
        .respond_with(move |_: &wiremock::Request| {
            let index = response_index.fetch_add(1, Ordering::SeqCst).min(1);
            responses::sse_response(parent_responses[index].clone())
        })
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(body_partial_json(json!({ "model": "gpt-5.6-luna" })))
        .respond_with(responses::sse_response(responses::sse(vec![
            responses::ev_assistant_message("score", "high"),
            responses::ev_completed("score"),
        ])))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(body_partial_json(json!({
            "client_metadata": { "x-openai-subagent": "guardian" },
        })))
        .respond_with(responses::sse_response(responses::sse(vec![
            responses::ev_assistant_message(
                "review",
                &json!({
                    "risk_level": "low",
                    "user_authorization": "high",
                    "outcome": "allow",
                    "rationale": "Mock review.",
                })
                .to_string(),
            ),
            responses::ev_completed("review"),
        ])))
        .mount(&server)
        .await;

    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.mcp_server_contributor(Arc::new(ExtensionOwnedAppsServer {
        url: format!("{}/api/codex/ps/mcp", apps.chatgpt_base_url),
    }));
    let test = apps_enabled_builder(apps.chatgpt_base_url)
        .with_model("test-gpt-5.1-codex")
        .with_extensions(Arc::new(extensions.build()))
        .with_pre_build_hook(|home| {
            std::fs::write(
                home.join("config.toml"),
                "[features.guardianv2]\nenabled = true\n\n[features.guardianv2.review_scope]\ncomputer_use_only = false\n\n[apps._default]\ndefault_tools_approval_mode = \"prompt\"\n",
            )
            .expect("write Guardian configuration");
        })
        .with_config(|config| {
            config.model_provider.supports_websockets = false;
            config
                .features
                .disable(Feature::EnableRequestCompression)
                .expect("disable request compression");
            config
                .features
                .enable(Feature::GuardianTrustOrchestratorConnectors)
                .expect("enable orchestrator trust");
            config
                .features
                .enable(Feature::GuardianApproval)
                .expect("enable Guardian approval");
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
        })
        .build_with_auto_env(&server)
        .await?;
    wait_for_mcp_server(&test.codex, "codex_apps").await?;
    test.submit_text_turn("List today's calendar events.")
        .await?;
    assert!(parent_index.load(Ordering::SeqCst) >= 2);

    let guardian_requests = server
        .received_requests()
        .await
        .expect("mock server request history")
        .into_iter()
        .filter(|request| {
            request.method.as_str() == "POST" && request.url.path() == "/v1/responses"
        })
        .map(|request| serde_json::from_slice::<Value>(&request.body))
        .collect::<serde_json::Result<Vec<_>>>()?;
    let luna = guardian_requests
        .iter()
        .find(|request| request["model"] == "gpt-5.6-luna")
        .expect("Guardian classifier request");
    assert!(luna.to_string().contains("calendar_list_events"));
    for request in guardian_requests.iter().filter(|request| {
        request["model"] == "gpt-5.6-luna"
            || request["client_metadata"]["x-openai-subagent"] == "guardian"
    }) {
        assert!(
            request["input"]
                .as_array()
                .expect("Guardian input")
                .iter()
                .all(|item| {
                    item["internal_chat_message_metadata_passthrough"]["content_item_kinds"]
                        != json!(["guardian.trusted_tool"])
                })
        );
    }
    Ok(())
}
