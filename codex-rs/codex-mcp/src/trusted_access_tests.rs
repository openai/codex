use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use codex_exec_server::ByteChunk;
use codex_exec_server::ExecServerError;
use codex_exec_server::HttpClient;
use codex_exec_server::HttpHeader;
use codex_exec_server::HttpRedirectPolicy;
use codex_exec_server::HttpRequestParams;
use codex_exec_server::HttpRequestResponse;
use codex_exec_server::HttpResponseBodyStream;
use codex_login::AuthCredentialsStoreMode;
use codex_login::AuthHeaders;
use codex_login::AuthKeyringBackendKind;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_login::ExternalAuth;
use codex_login::ExternalAuthFuture;
use codex_login::ExternalAuthRefreshContext;
use futures::FutureExt;
use futures::future::BoxFuture;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tokio::sync::Notify;

use super::MAX_VERIFIED_ACCESS_RESPONSE_BYTES;
use super::TrustedAccessContext;

struct RecordingHttpClient {
    requests: Mutex<Vec<HttpRequestParams>>,
    status: u16,
    response: Vec<u8>,
    response_chunks: Option<Vec<Vec<u8>>>,
    response_gate: Option<(Notify, Notify)>,
}

impl RecordingHttpClient {
    fn new(status: u16, response: Value) -> Self {
        Self {
            requests: Mutex::new(Vec::new()),
            status,
            response: serde_json::to_vec(&response).expect("serialize response"),
            response_chunks: None,
            response_gate: None,
        }
    }
}

impl HttpClient for RecordingHttpClient {
    fn http_request(
        &self,
        params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<HttpRequestResponse, ExecServerError>> {
        self.requests.lock().expect("record request").push(params);
        let response = HttpRequestResponse {
            status: self.status,
            headers: Vec::new(),
            body: ByteChunk::from(self.response.clone()),
        };
        async move {
            if let Some((requested, release)) = &self.response_gate {
                requested.notify_one();
                release.notified().await;
            }
            Ok(response)
        }
        .boxed()
    }

    fn http_request_stream(
        &self,
        params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<(HttpRequestResponse, HttpResponseBodyStream), ExecServerError>> {
        self.requests.lock().expect("record request").push(params);
        let response = HttpRequestResponse {
            status: self.status,
            headers: Vec::new(),
            body: ByteChunk::from(Vec::new()),
        };
        let chunks = self
            .response_chunks
            .clone()
            .unwrap_or_else(|| vec![self.response.clone()]);
        async move {
            if let Some((requested, release)) = &self.response_gate {
                requested.notify_one();
                release.notified().await;
            }
            Ok((response, HttpResponseBodyStream::from_chunks(chunks)))
        }
        .boxed()
    }
}

struct StaticExternalAuth(CodexAuth);

impl ExternalAuth for StaticExternalAuth {
    fn resolve(&self) -> ExternalAuthFuture<'_, CodexAuth> {
        Box::pin(async { Ok(self.0.clone()) })
    }

    fn refresh(&self, _context: ExternalAuthRefreshContext) -> ExternalAuthFuture<'_, CodexAuth> {
        self.resolve()
    }
}

struct SequencedExternalAuth(Mutex<VecDeque<CodexAuth>>);

impl ExternalAuth for SequencedExternalAuth {
    fn resolve(&self) -> ExternalAuthFuture<'_, CodexAuth> {
        Box::pin(async {
            let mut sequence = self.0.lock().expect("auth sequence");
            Ok(if sequence.len() > 1 {
                sequence.pop_front().expect("next auth")
            } else {
                sequence.front().expect("last auth").clone()
            })
        })
    }

    fn refresh(&self, _context: ExternalAuthRefreshContext) -> ExternalAuthFuture<'_, CodexAuth> {
        self.resolve()
    }
}

fn header_auth() -> CodexAuth {
    CodexAuth::Headers(AuthHeaders::new(
        [
            (
                "authorization".parse().unwrap(),
                "Bearer synthetic-pat".parse().unwrap(),
            ),
            (
                "chatgpt-account-id".parse().unwrap(),
                "account-a".parse().unwrap(),
            ),
        ]
        .into_iter()
        .collect(),
    ))
}

fn chatgpt_auth(account_id: &str) -> CodexAuth {
    CodexAuth::from_external_chatgpt_tokens(
        "header.e30.same",
        account_id,
        /*chatgpt_plan_type*/ None,
    )
    .expect("test auth")
}

fn fedramp_chatgpt_auth(account_id: &str) -> CodexAuth {
    CodexAuth::from_external_chatgpt_tokens(
        "header.eyJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF9hY2NvdW50X2lzX2ZlZHJhbXAiOnRydWV9fQ.same",
        account_id,
        /*chatgpt_plan_type*/ None,
    )
    .expect("FedRAMP test auth")
}

fn context(auth: CodexAuth, client: Arc<RecordingHttpClient>) -> TrustedAccessContext {
    TrustedAccessContext::new(
        auth.clone(),
        AuthManager::from_auth_for_testing(auth),
        "https://chatgpt.com/backend-api".to_string(),
        client,
    )
}

fn cyber_response(state: &str, grants: Value) -> Value {
    json!({ "programs": [{ "program": "cyber", "state": state, "grants": grants }] })
}

fn expected_metadata(status: &str, grants: Value) -> Value {
    json!({ "openai/entitlementContext": {
        "schemaVersion": 1,
        "entitlements": { "cyber_trusted_access": {
            "schemaVersion": 1,
            "status": status,
            "grants": grants,
            "stale": false
        } }
    } })
}

#[tokio::test]
async fn delivers_account_bound_verified_access_to_the_calling_plugin() {
    let client = Arc::new(RecordingHttpClient::new(
        /*status*/ 200,
        cyber_response(
            "active",
            json!([
                { "level": "tac2", "source": "individual" },
                { "level": "government", "source": "organization" }
            ]),
        ),
    ));
    let mut context = context(
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        client.clone(),
    );
    context.chatgpt_base_url.push('/');
    let metadata = context
        .add_context(Some(json!({ "threadId": "thread-1" })))
        .await;
    let mut expected = expected_metadata(
        "granted",
        json!([
            { "level": "tac2", "source": "user" },
            { "level": "government", "source": "current_account" }
        ]),
    );
    expected["threadId"] = json!("thread-1");
    assert_eq!(metadata, Some(expected));

    let mut requests = client.requests.lock().expect("inspect recorded requests");
    for request in requests.iter_mut() {
        request
            .headers
            .sort_by(|left, right| left.name.cmp(&right.name));
    }
    assert_eq!(
        *requests,
        vec![HttpRequestParams {
            method: "GET".to_string(),
            url: "https://chatgpt.com/backend-api/accounts/verified_access".to_string(),
            headers: vec![
                HttpHeader {
                    name: "authorization".to_string(),
                    value: "Bearer Access Token".to_string(),
                    value_env_var: None,
                },
                HttpHeader {
                    name: "chatgpt-account-id".to_string(),
                    value: "account_id".to_string(),
                    value_env_var: None,
                },
            ],
            body: None,
            timeout_ms: Some(2_500),
            redirect_policy: HttpRedirectPolicy::Stop,
            request_id: "trusted-access-status".to_string(),
            stream_response: true,
        }]
    );
}

#[tokio::test]
async fn rejects_auth_without_a_nonempty_account_id() -> anyhow::Result<()> {
    for account_id in [None, Some(""), Some(" ")] {
        let home = tempfile::tempdir()?;
        std::fs::write(
            home.path().join("auth.json"),
            serde_json::to_vec(&json!({
                "auth_mode": "chatgpt",
                "tokens": {
                    "id_token": "header.eyJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF91c2VyX2lkIjoidXNlci1hIn19.signature",
                    "access_token": "synthetic-access-token",
                    "refresh_token": "synthetic-refresh-token",
                    "account_id": account_id
                },
                "last_refresh": "2099-01-01T00:00:00Z"
            }))?,
        )?;
        let auth = CodexAuth::from_auth_storage(
            home.path(),
            AuthCredentialsStoreMode::File,
            /*chatgpt_base_url*/ None,
            AuthKeyringBackendKind::default(),
            &codex_login::test_support::transport_default_auth_route_config(),
        )
        .await?
        .expect("managed ChatGPT auth");
        let client = Arc::new(RecordingHttpClient::new(
            /*status*/ 200,
            cyber_response(
                "active",
                json!([{ "level": "tac1", "source": "individual" }]),
            ),
        ));
        assert_eq!(
            context(auth, client.clone())
                .add_context(/*meta*/ None)
                .await,
            Some(expected_metadata("unknown", json!([]))),
            "account_id={account_id:?}"
        );
        assert!(client.requests.lock().expect("inspect requests").is_empty());
    }
    Ok(())
}

#[tokio::test]
async fn rejects_api_key_authentication_without_sending_credentials() {
    let client = Arc::new(RecordingHttpClient::new(
        /*status*/ 200,
        json!({ "programs": [] }),
    ));
    let context = context(CodexAuth::from_api_key("test-api-key"), client.clone());

    let metadata = context
        .add_context(Some(json!({
            "threadId": "thread-1",
            "openai/entitlementContext": { "forged": true }
        })))
        .await;

    let mut expected = expected_metadata("unknown", json!([]));
    expected["threadId"] = json!("thread-1");
    assert_eq!(metadata, Some(expected));
    assert!(client.requests.lock().expect("inspect requests").is_empty());
}

#[tokio::test]
async fn rejects_initial_unsupported_auth_after_switch_to_chatgpt() {
    let client = Arc::new(RecordingHttpClient::new(
        /*status*/ 200,
        cyber_response(
            "active",
            json!([{ "level": "tac1", "source": "individual" }]),
        ),
    ));
    let mut context = context(header_auth(), client.clone());
    context.auth_manager = AuthManager::from_auth_for_testing(chatgpt_auth("account-a"));

    assert_eq!(
        context.add_context(/*meta*/ None).await,
        Some(expected_metadata("unknown", json!([])))
    );
    assert!(client.requests.lock().expect("inspect requests").is_empty());
}

#[tokio::test]
async fn maps_verified_access_states_and_rejects_invalid_provider_responses() {
    let cases = [
        (
            200,
            json!({ "programs": [
                { "program": "future", "state": "pending", "grants": [{ "level": "premium", "source": "subscription" }] },
                { "program": "cyber", "state": "active", "grants": [{ "level": "tac1", "source": "individual" }] }
            ] }),
            expected_metadata("granted", json!([{ "level": "tac1", "source": "user" }])),
        ),
        (
            200,
            cyber_response("inactive", json!([])),
            expected_metadata("not_granted", json!([])),
        ),
        (
            200,
            cyber_response("unavailable", json!([])),
            expected_metadata("unknown", json!([])),
        ),
        (
            200,
            json!({ "programs": [{ "program": "other", "state": "active", "grants": [] }] }),
            expected_metadata("unknown", json!([])),
        ),
        (
            200,
            cyber_response(
                "active",
                json!([{ "level": "admin", "source": "individual" }]),
            ),
            expected_metadata("unknown", json!([])),
        ),
        (
            200,
            cyber_response("active", json!([])),
            expected_metadata("unknown", json!([])),
        ),
        (
            403,
            json!({ "error": "forbidden" }),
            expected_metadata("unknown", json!([])),
        ),
    ];

    for (response_status, response, expected) in cases {
        let response_description = response.to_string();
        let context = context(
            CodexAuth::create_dummy_chatgpt_auth_for_testing(),
            Arc::new(RecordingHttpClient::new(response_status, response)),
        );
        let metadata = context.add_context(/*meta*/ None).await.expect("metadata");
        assert_eq!(
            metadata, expected,
            "HTTP {response_status}: {response_description}"
        );
    }
}

#[tokio::test]
async fn replaces_untrusted_entitlements_on_malformed_response() {
    let mut client = RecordingHttpClient::new(/*status*/ 200, Value::Null);
    client.response = b"{\"programs\":[".to_vec();
    let context = context(chatgpt_auth("account-a"), Arc::new(client));
    let mut expected = expected_metadata("unknown", json!([]));
    expected["threadId"] = json!("thread-1");

    assert_eq!(
        context
            .add_context(Some(json!({
                "threadId": "thread-1",
                "openai/entitlementContext": { "forged": true }
            })))
            .await,
        Some(expected)
    );
}

#[tokio::test]
async fn rejects_verified_access_response_larger_than_one_mebibyte() {
    let valid_response = serde_json::to_vec(&cyber_response(
        "active",
        json!([{ "level": "tac1", "source": "individual" }]),
    ))
    .expect("serialize response");
    let mut client = RecordingHttpClient::new(/*status*/ 200, Value::Null);
    client.response_chunks = Some(vec![
        vec![b' '; MAX_VERIFIED_ACCESS_RESPONSE_BYTES],
        valid_response,
    ]);
    client.response = client
        .response_chunks
        .as_ref()
        .expect("response chunks")
        .concat();

    assert_eq!(
        context(chatgpt_auth("account-a"), Arc::new(client))
            .add_context(/*meta*/ None)
            .await,
        Some(expected_metadata("unknown", json!([])))
    );
}

#[tokio::test(start_paused = true)]
async fn returns_unknown_at_the_lookup_deadline() {
    let mut client = RecordingHttpClient::new(/*status*/ 200, Value::Null);
    client.response_gate = Some((Notify::new(), Notify::new()));
    let client = Arc::new(client);
    let context = context(chatgpt_auth("account-a"), client.clone());
    let lookup = context.add_context(/*meta*/ None);
    tokio::pin!(lookup);

    assert!(futures::poll!(lookup.as_mut()).is_pending());
    assert_eq!(client.requests.lock().expect("recorded requests").len(), 1);
    tokio::time::advance(Duration::from_millis(2_499)).await;
    assert!(futures::poll!(lookup.as_mut()).is_pending());
    tokio::time::advance(Duration::from_millis(1)).await;
    assert_eq!(
        lookup
            .now_or_never()
            .expect("lookup must finish at the deadline"),
        Some(expected_metadata("unknown", json!([])))
    );
}

#[tokio::test]
async fn rejects_duplicate_cyber_programs() {
    let active = json!({
        "program": "cyber", "state": "active",
        "grants": [{ "level": "tac1", "source": "individual" }]
    });
    for other in [
        active.clone(),
        json!({ "program": "cyber", "state": "inactive", "grants": [] }),
        json!({ "program": "cyber" }),
    ] {
        for programs in [json!([active, other]), json!([other, active])] {
            let context = context(
                CodexAuth::create_dummy_chatgpt_auth_for_testing(),
                Arc::new(RecordingHttpClient::new(
                    /*status*/ 200,
                    json!({ "programs": programs }),
                )),
            );
            assert_eq!(
                context.add_context(/*meta*/ None).await,
                Some(expected_metadata("unknown", json!([]))),
                "duplicate programs: {programs}"
            );
        }
    }
}

#[tokio::test]
async fn rejects_identity_changes_before_sending_credentials() {
    for (description, initial_auth, selected_auth) in [
        (
            "account switch",
            chatgpt_auth("account-a"),
            chatgpt_auth("account-b"),
        ),
        (
            "standard to FedRAMP",
            chatgpt_auth("account-a"),
            fedramp_chatgpt_auth("account-a"),
        ),
        (
            "FedRAMP to standard",
            fedramp_chatgpt_auth("account-a"),
            chatgpt_auth("account-a"),
        ),
    ] {
        let client = Arc::new(RecordingHttpClient::new(
            /*status*/ 200,
            cyber_response(
                "active",
                json!([{ "level": "tac1", "source": "individual" }]),
            ),
        ));
        let mut context = context(initial_auth, client.clone());
        context.auth_manager = AuthManager::from_auth_for_testing(selected_auth);
        assert_eq!(
            context.add_context(/*meta*/ None).await,
            Some(expected_metadata("unknown", json!([]))),
            "identity change before the request: {description}"
        );
        assert!(
            client.requests.lock().expect("inspect requests").is_empty(),
            "identity change before the request: {description}"
        );
    }
}

#[tokio::test]
async fn uses_the_checked_auth_snapshot_for_request_headers() -> anyhow::Result<()> {
    let client = Arc::new(RecordingHttpClient::new(
        /*status*/ 200,
        cyber_response("inactive", json!([])),
    ));
    let context = context(chatgpt_auth("account-a"), client.clone());
    let refreshed = CodexAuth::from_external_chatgpt_tokens(
        "header.e30.refreshed",
        "account-a",
        /*chatgpt_plan_type*/ None,
    )?;
    context
        .auth_manager
        .set_external_auth(Arc::new(SequencedExternalAuth(Mutex::new(VecDeque::from(
            [context.auth.clone(), refreshed, header_auth()],
        )))))
        .await?;

    assert_eq!(
        context.add_context(/*meta*/ None).await,
        Some(expected_metadata("not_granted", json!([])))
    );
    let requests = client.requests.lock().expect("recorded requests");
    assert_eq!(requests.len(), 1);
    let authorization = requests[0]
        .headers
        .iter()
        .find(|header| header.name == "authorization");
    assert_eq!(
        authorization,
        Some(&HttpHeader {
            name: "authorization".to_string(),
            value: "Bearer header.e30.refreshed".to_string(),
            value_env_var: None,
        })
    );
    Ok(())
}

#[tokio::test]
async fn rejects_identity_changes_while_request_is_in_flight() -> anyhow::Result<()> {
    let personal = chatgpt_auth("account-a");
    let workspace =
        CodexAuth::from_external_chatgpt_tokens("header.e30.same", "account-a", Some("team"))?;
    for (description, initial_auth, selected_auth) in [
        (
            "account switch",
            personal.clone(),
            Some(chatgpt_auth("account-b")),
        ),
        ("logout", personal.clone(), None),
        (
            "personal to workspace",
            personal.clone(),
            Some(workspace.clone()),
        ),
        ("workspace to personal", workspace, Some(personal)),
        (
            "auth mode change",
            chatgpt_auth("account-a"),
            Some(header_auth()),
        ),
        (
            "standard to FedRAMP",
            chatgpt_auth("account-a"),
            Some(fedramp_chatgpt_auth("account-a")),
        ),
        (
            "FedRAMP to standard",
            fedramp_chatgpt_auth("account-a"),
            Some(chatgpt_auth("account-a")),
        ),
    ] {
        let mut client = RecordingHttpClient::new(
            /*status*/ 200,
            cyber_response(
                "active",
                json!([{ "level": "tac1", "source": "individual" }]),
            ),
        );
        client.response_gate = Some((Notify::new(), Notify::new()));
        let client = Arc::new(client);
        let request_is_fedramp = initial_auth.is_fedramp_account();
        let context = context(initial_auth, client.clone());
        let auth_manager = &context.auth_manager;
        auth_manager
            .set_external_auth(Arc::new(StaticExternalAuth(context.auth.clone())))
            .await?;

        let (metadata, auth_change) = tokio::join!(context.add_context(/*meta*/ None), async {
            let (requested, release) = client.response_gate.as_ref().expect("gated response");
            tokio::time::timeout(Duration::from_secs(5), requested.notified()).await?;
            {
                let requests = client.requests.lock().expect("inspect request");
                assert_eq!(requests.len(), 1);
                assert!(requests[0].headers.iter().any(|header| {
                    header.name.eq_ignore_ascii_case("chatgpt-account-id")
                        && header.value == "account-a"
                }));
                assert_eq!(
                    requests[0]
                        .headers
                        .iter()
                        .any(|header| header.name.eq_ignore_ascii_case("x-openai-fedramp")),
                    request_is_fedramp
                );
            }
            if let Some(auth) = selected_auth {
                auth_manager
                    .set_external_auth(Arc::new(StaticExternalAuth(auth)))
                    .await?;
            } else {
                auth_manager.clear_external_auth();
            }
            release.notify_one();
            Ok::<(), anyhow::Error>(())
        });
        auth_change?;

        assert_eq!(
            metadata,
            Some(expected_metadata("unknown", json!([]))),
            "auth change after the request: {description}"
        );
    }
    Ok(())
}
