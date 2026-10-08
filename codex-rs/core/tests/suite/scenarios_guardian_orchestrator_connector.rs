//! A host-owned Apps connector contributes its exact identity to Guardian's complete request history.

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use anyhow::Result;
use codex_core::config::Constrained;
use codex_features::Feature;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::protocol::AskForApproval;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::SEARCH_CALENDAR_LIST_TOOL;
use core_test_support::apps_test_server::SEARCH_CALENDAR_NAMESPACE;
use core_test_support::apps_test_server::apps_enabled_builder;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::context_snapshot::SnapshotEntry;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::wait_for_mcp_server;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use wiremock::Mock;
use wiremock::matchers::body_partial_json;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn host_owned_orchestrator_connector_reaches_guardian_context() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let apps = AppsTestServer::mount(&server).await?;
    let parent_responses = [
        responses::sse(vec![
            responses::ev_function_call_with_namespace(
                "connector",
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

    let test = apps_enabled_builder(apps.chatgpt_base_url)
        .with_model("test-gpt-5.1-codex")
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
    assert_eq!(parent_index.load(Ordering::SeqCst), 2);

    let mut requests = server
        .received_requests()
        .await
        .expect("mock server request history")
        .into_iter()
        .filter(|request| {
            request.method.as_str() == "POST" && request.url.path() == "/v1/responses"
        })
        .map(|request| serde_json::from_slice::<Value>(&request.body))
        .collect::<serde_json::Result<Vec<_>>>()?;
    let trusted_messages = requests
        .iter()
        .filter(|request| {
            request["model"] == "gpt-5.6-luna"
                || request["client_metadata"]["x-openai-subagent"] == "guardian"
        })
        .flat_map(|request| request["input"].as_array().into_iter().flatten())
        .filter(|item| {
            item["internal_chat_message_metadata_passthrough"]["content_item_kinds"]
                == json!(["guardian.trusted_tool"])
        })
        .map(|item| {
            let text = item["content"][0]["text"].as_str().expect("trusted text");
            let (_, evidence) = text.split_once('\n').expect("trusted connector evidence");
            serde_json::from_str::<Value>(evidence)
        })
        .collect::<serde_json::Result<Vec<_>>>()?;
    assert_eq!(
        trusted_messages,
        vec![json!({
            "server": "codex_apps",
            "connector_id": "calendar",
            "source": "plugin_service_orchestrator",
        })]
    );
    // Normalize tool JSON before long snapshot lines are clipped and fingerprinted.
    for request in &mut requests {
        for item in request["input"].as_array_mut().into_iter().flatten() {
            if let Some(Value::String(output)) = item.get_mut("output") {
                *output = context_snapshot::normalize_json_lines(output);
            }
        }
    }
    let entries = requests.iter().map(SnapshotEntry::body).collect::<Vec<_>>();
    let snapshot = context_snapshot::format_context_snapshot(
        "Guardian reviews a host-owned orchestrator connector, then the parent uses its calendar tool and continues.",
        &entries,
        &ContextSnapshotOptions::default().rewrite_known_segments(),
    );
    let snapshot = regex_lite::Regex::new(
        r#"(The active permission profile for environment )"(?:local|remote)""#,
    )?
    .replace_all(&snapshot, "$1\"<ENVIRONMENT>\"")
    .into_owned();
    insta::assert_snapshot!(
        "host_owned_orchestrator_connector_request_history",
        context_snapshot::normalize_json_lines(&snapshot)
    );
    Ok(())
}
