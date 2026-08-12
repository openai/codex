use codex_login::AuthHeaders;
use codex_login::CodexAuth;
use codex_login::ExternalAuth;
use codex_login::ExternalAuthFuture;
use codex_login::ExternalAuthRefreshContext;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use http::HeaderMap;
use http::HeaderValue;
use http::header::AUTHORIZATION;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use std::sync::Mutex;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

const CHATGPT_ACCOUNT_ID: &str = "workspace-one";
const INITIAL_ACCESS_TOKEN: &str = "header.e30.initial";
const REFRESHED_ACCESS_TOKEN: &str = "header.e30.refreshed";

struct ScriptedExternalAuth {
    current: Mutex<CodexAuth>,
    refreshed: CodexAuth,
}

impl ExternalAuth for ScriptedExternalAuth {
    fn resolve(&self) -> ExternalAuthFuture<'_, CodexAuth> {
        let auth = self
            .current
            .lock()
            .map_err(|_| std::io::Error::other("external auth lock is poisoned"))
            .map(|current| current.clone());
        Box::pin(async move { auth })
    }

    fn refresh(&self, context: ExternalAuthRefreshContext) -> ExternalAuthFuture<'_, CodexAuth> {
        let auth = if context.previous_account_id.as_deref() != Some(CHATGPT_ACCOUNT_ID) {
            Err(std::io::Error::other(
                "external auth refresh changed the ChatGPT workspace",
            ))
        } else {
            self.current
                .lock()
                .map_err(|_| std::io::Error::other("external auth lock is poisoned"))
                .map(|mut current| {
                    *current = self.refreshed.clone();
                    self.refreshed.clone()
                })
        };
        Box::pin(async move { auth })
    }
}

fn external_chatgpt_auth(access_token: &str) -> std::io::Result<CodexAuth> {
    CodexAuth::from_external_chatgpt_tokens(access_token, CHATGPT_ACCOUNT_ID, Some("enterprise"))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn header_auth_is_attached_to_responses_requests() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let response_mock = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer external"));
    headers.insert("x-external-auth", HeaderValue::from_static("enabled"));
    let mut builder = test_codex().with_auth(CodexAuth::Headers(AuthHeaders::new(headers)));
    let test = builder.build_with_auto_env(&server).await?;

    test.submit_turn("hello").await?;

    let request = response_mock.single_request();
    assert_eq!(
        request.header("authorization").as_deref(),
        Some("Bearer external")
    );
    assert_eq!(
        request.header("x-external-auth").as_deref(),
        Some("enabled")
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn external_auth_401_retry_uses_refreshed_chatgpt_headers() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header(
            "authorization",
            format!("Bearer {INITIAL_ACCESS_TOKEN}"),
        ))
        .and(header("chatgpt-account-id", CHATGPT_ACCOUNT_ID))
        .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header(
            "authorization",
            format!("Bearer {REFRESHED_ACCESS_TOKEN}"),
        ))
        .and(header("chatgpt-account-id", CHATGPT_ACCOUNT_ID))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(sse(vec![
                    ev_response_created("resp-1"),
                    ev_completed("resp-1"),
                ])),
        )
        .expect(1)
        .mount(&server)
        .await;

    let mut builder = test_codex().with_auth(CodexAuth::from_api_key("seed"));
    let test = builder.build_with_auto_env(&server).await?;
    let external_auth = Arc::new(ScriptedExternalAuth {
        current: Mutex::new(external_chatgpt_auth(INITIAL_ACCESS_TOKEN)?),
        refreshed: external_chatgpt_auth(REFRESHED_ACCESS_TOKEN)?,
    });
    test.thread_manager
        .auth_manager()
        .set_external_auth(external_auth.clone())
        .await?;

    test.submit_turn("hello").await?;

    server.verify().await;
    let requests = server
        .received_requests()
        .await
        .expect("mock server should capture requests");
    let authorization_headers = requests
        .iter()
        .filter(|request| request.url.path() == "/v1/responses")
        .filter_map(|request| request.headers.get("authorization"))
        .filter_map(|value| value.to_str().ok())
        .map(str::to_string)
        .collect::<Vec<_>>();
    assert_eq!(
        authorization_headers,
        vec![
            format!("Bearer {INITIAL_ACCESS_TOKEN}"),
            format!("Bearer {REFRESHED_ACCESS_TOKEN}"),
        ]
    );
    Ok(())
}
