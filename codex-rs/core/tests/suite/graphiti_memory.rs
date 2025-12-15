use anyhow::Result;
use codex_core::config::types::GraphitiRecallScopesMode;
use codex_protocol::config_types::TrustLevel;
use core_test_support::responses;
use core_test_support::test_codex::TestCodexHarness;
use core_test_support::test_codex::test_codex;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test]
async fn injects_graphiti_memory_and_ingests_turn() -> Result<()> {
    let graphiti_server = MockServer::start().await;
    let graphiti_endpoint = graphiti_server.uri();

    Mock::given(method("POST"))
        .and(path("/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "facts": [{
                "uuid": "fact-1",
                "name": "fact-1",
                "fact": "Prefer `rg` over `grep` for searches in this repo.",
                "valid_at": null,
                "invalid_at": null,
                "created_at": "2025-01-01T00:00:00Z",
                "expired_at": null
            }]
        })))
        .expect(1)
        .mount(&graphiti_server)
        .await;

    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(202).set_body_json(json!({
            "message": "accepted",
            "success": true
        })))
        .mount(&graphiti_server)
        .await;

    let harness = TestCodexHarness::with_builder(test_codex().with_config(move |config| {
        config.active_project.trust_level = Some(TrustLevel::Trusted);
        config.graphiti.enabled = true;
        config.graphiti.consent = true;
        config.graphiti.endpoint = Some(graphiti_endpoint);
        config.graphiti.recall.enabled = true;
        config.graphiti.recall.scopes_mode = GraphitiRecallScopesMode::Auto;
        config.graphiti.global.enabled = true;
        config.graphiti.user_scope_key = Some("user-key-1".to_string());
    }))
    .await?;

    let responses_mock = responses::mount_sse_once(
        harness.server(),
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_assistant_message("msg-1", "ok"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;

    harness
        .submit("What should we use for searching in this repo?")
        .await?;

    let request = responses_mock.single_request();
    let system_texts = request.message_input_texts("system");
    assert!(
        system_texts
            .iter()
            .any(|text| text.contains("<graphiti_memory>")),
        "expected injected <graphiti_memory> section; got: {system_texts:#?}"
    );
    assert!(
        system_texts
            .iter()
            .any(|text| text.contains("Prefer `rg` over `grep`")),
        "expected injected fact; got: {system_texts:#?}"
    );

    let search_requests: Vec<wiremock::Request> = graphiti_server
        .received_requests()
        .await
        .expect("mock server should not fail")
        .into_iter()
        .filter(|req| req.method == "POST" && req.url.path() == "/search")
        .collect();
    assert_eq!(search_requests.len(), 1);

    let search_body: serde_json::Value =
        serde_json::from_slice(&search_requests[0].body).expect("search body should be JSON");
    let group_ids = search_body["group_ids"]
        .as_array()
        .expect("group_ids should be an array")
        .iter()
        .filter_map(|v| v.as_str())
        .collect::<Vec<_>>();

    assert_eq!(group_ids.len(), 2);
    assert!(
        !group_ids.iter().any(|id| id.starts_with("codex-global-")),
        "expected auto recall to exclude global for this query; got group_ids={group_ids:?}"
    );

    let message_requests = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            let requests = graphiti_server
                .received_requests()
                .await
                .expect("mock server should not fail");
            let messages: Vec<wiremock::Request> = requests
                .into_iter()
                .filter(|req| req.method == "POST" && req.url.path() == "/messages")
                .collect();
            if messages.len() >= 2 {
                return messages;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("timed out waiting for Graphiti /messages requests");

    let mut group_ids: Vec<String> = Vec::new();
    for req in message_requests {
        let body: serde_json::Value =
            serde_json::from_slice(&req.body).expect("messages body should be JSON");
        let group_id = body["group_id"]
            .as_str()
            .expect("group_id should be string")
            .to_string();
        group_ids.push(group_id);
    }

    group_ids.sort();
    assert!(
        group_ids.iter().any(|id| id.starts_with("codex-session-")),
        "expected a session-scoped group id; got: {group_ids:?}"
    );
    assert!(
        group_ids
            .iter()
            .any(|id| id.starts_with("codex-workspace-")),
        "expected a workspace-scoped group id; got: {group_ids:?}"
    );

    Ok(())
}

#[tokio::test]
async fn auto_promotes_memory_directives_to_global() -> Result<()> {
    let graphiti_server = MockServer::start().await;
    let graphiti_endpoint = graphiti_server.uri();

    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(202).set_body_json(json!({
            "message": "accepted",
            "success": true
        })))
        .mount(&graphiti_server)
        .await;

    let harness = TestCodexHarness::with_builder(test_codex().with_config(move |config| {
        config.active_project.trust_level = Some(TrustLevel::Trusted);
        config.graphiti.enabled = true;
        config.graphiti.consent = true;
        config.graphiti.endpoint = Some(graphiti_endpoint);
        config.graphiti.global.enabled = true;
        config.graphiti.user_scope_key = Some("user-key-1".to_string());
        config.graphiti.auto_promote.enabled = true;
    }))
    .await?;

    let responses_mock = responses::mount_sse_once(
        harness.server(),
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_assistant_message("msg-1", "ok"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;

    harness
        .submit("preference (global): Keep diffs small and avoid inline comments.")
        .await?;

    let _request = responses_mock.single_request();

    let message_requests = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            let requests = graphiti_server
                .received_requests()
                .await
                .expect("mock server should not fail");
            let messages: Vec<wiremock::Request> = requests
                .into_iter()
                .filter(|req| req.method == "POST" && req.url.path() == "/messages")
                .collect();
            if messages.len() >= 3 {
                return messages;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("timed out waiting for Graphiti /messages requests");

    let mut saw_global = false;
    let mut saw_episode = false;

    for req in message_requests {
        let body: serde_json::Value =
            serde_json::from_slice(&req.body).expect("messages body should be JSON");
        let group_id = body["group_id"]
            .as_str()
            .expect("group_id should be string");
        if group_id.starts_with("codex-global-") {
            saw_global = true;
            if let Some(messages) = body["messages"].as_array() {
                saw_episode = messages.iter().any(|m| {
                    m["content"]
                        .as_str()
                        .is_some_and(|c| c.contains("<graphiti_episode kind=\"preference\">"))
                        && m["content"]
                            .as_str()
                            .is_some_and(|c| c.contains("scope: global"))
                });
            }
        }
    }

    assert!(
        saw_global,
        "expected an auto-promotion write to global scope"
    );
    assert!(
        saw_episode,
        "expected a promoted <graphiti_episode> in global scope"
    );

    Ok(())
}

#[tokio::test]
async fn ingests_ownership_context_system_episode_when_enabled() -> Result<()> {
    let graphiti_server = MockServer::start().await;
    let graphiti_endpoint = graphiti_server.uri();

    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(202).set_body_json(json!({
            "message": "accepted",
            "success": true
        })))
        .mount(&graphiti_server)
        .await;

    let harness = TestCodexHarness::with_builder(test_codex().with_config(move |config| {
        config.active_project.trust_level = Some(TrustLevel::Trusted);
        config.graphiti.enabled = true;
        config.graphiti.consent = true;
        config.graphiti.endpoint = Some(graphiti_endpoint);
        config.graphiti.include_system_messages = true;
    }))
    .await?;

    let responses_mock = responses::mount_sse_once(
        harness.server(),
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_assistant_message("msg-1", "ok"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;

    harness.submit("hello").await?;
    let _request = responses_mock.single_request();

    let message_requests = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            let requests = graphiti_server
                .received_requests()
                .await
                .expect("mock server should not fail");
            let messages: Vec<wiremock::Request> = requests
                .into_iter()
                .filter(|req| req.method == "POST" && req.url.path() == "/messages")
                .collect();
            if messages.len() >= 2 {
                return messages;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("timed out waiting for Graphiti /messages requests");

    let mut contents: Vec<String> = Vec::new();
    for req in message_requests {
        let body: serde_json::Value =
            serde_json::from_slice(&req.body).expect("messages body should be JSON");
        if let Some(messages) = body["messages"].as_array() {
            for m in messages {
                if let Some(content) = m["content"].as_str() {
                    contents.push(content.to_string());
                }
            }
        }
    }

    assert!(
        contents
            .iter()
            .any(|c| c.contains("<graphiti_episode kind=\"ownership_context\">")),
        "expected an ownership_context system episode; got contents={contents:#?}"
    );

    Ok(())
}
