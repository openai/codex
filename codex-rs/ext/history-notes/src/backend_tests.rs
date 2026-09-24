use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_login::AuthHeaders;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_model_provider::create_model_provider;
use codex_model_provider_info::ModelProviderInfo;
use codex_utils_output_truncation::TruncationPolicy;
use http::HeaderMap;
use http::HeaderValue;
use pretty_assertions::assert_eq;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::ENCRYPTED_TOOL_ARGUMENTS_HEADER;
use super::HistoryNotesBackend;

#[tokio::test]
async fn routes_through_codex_backend_and_injects_trusted_session_agent_context() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/backend-api/codex/alpha/notes/v2/read_file"))
        .and(header("x-openai-actor-authorization", "actor-biscuit"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "encrypted_output": "enc_payload"
        })))
        .mount(&server)
        .await;
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-openai-actor-authorization",
        HeaderValue::from_static("actor-biscuit"),
    );
    let auth_manager =
        AuthManager::from_auth_for_testing(CodexAuth::Headers(AuthHeaders::new(headers)));
    let provider = create_model_provider(
        ModelProviderInfo::create_openai_provider(Some(format!(
            "{}/backend-api/codex",
            server.uri()
        ))),
        Some(auth_manager),
    );
    let backend = HistoryNotesBackend::new(
        provider,
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    );

    let response = backend
        .call(
            "alpha/notes/v2/read_file",
            "session-123",
            "/root/worker",
            json!({
                "path": "notes.md",
                "context": {
                    "session_id": "spoofed-session",
                    "current_agent_name": "/root/spoofed",
                }
            }),
            TruncationPolicy::Bytes(1024),
        )
        .await
        .expect("History request should succeed");

    assert_eq!(response, json!({"encrypted_output": "enc_payload"}));
    let requests = server.received_requests().await.expect("recorded requests");
    assert_eq!(requests.len(), 1);
    assert!(
        requests[0]
            .headers
            .get(ENCRYPTED_TOOL_ARGUMENTS_HEADER)
            .is_none()
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&requests[0].body).expect("JSON body"),
        json!({
            "path": "notes.md",
            "context": {
                "session_id": "session-123",
                "current_agent_name": "/root/worker",
            }
        })
    );
}

#[tokio::test]
async fn marks_encrypted_history_and_notes_arguments_without_changing_the_json_body() {
    let server = MockServer::start().await;
    let cases = [
        (
            "history/v2/search_contents",
            json!({"query": "encrypted-query"}),
        ),
        (
            "notes/v2/search_contents",
            json!({"query": "encrypted-query"}),
        ),
        (
            "notes/v2/append_to_file",
            json!({"path": "notes.md", "text": "encrypted-text"}),
        ),
        (
            "notes/v2/write_file",
            json!({"path": "notes.md", "text": "encrypted-text"}),
        ),
    ];
    for (route, _) in &cases {
        Mock::given(method("POST"))
            .and(path(format!("/backend-api/codex/alpha/{route}")))
            .and(header(ENCRYPTED_TOOL_ARGUMENTS_HEADER, "true"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
            .expect(1)
            .mount(&server)
            .await;
    }

    let auth_manager =
        AuthManager::from_auth_for_testing(CodexAuth::Headers(AuthHeaders::new(HeaderMap::new())));
    let backend = HistoryNotesBackend::new(
        create_model_provider(
            ModelProviderInfo::create_openai_provider(Some(format!(
                "{}/backend-api/codex",
                server.uri()
            ))),
            Some(auth_manager),
        ),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    );

    for (route, arguments) in cases {
        backend
            .call(
                &format!("alpha/{route}"),
                "session-123",
                "/root",
                arguments.clone(),
                TruncationPolicy::Bytes(1024),
            )
            .await
            .expect("encrypted argument request should succeed");
    }
}
