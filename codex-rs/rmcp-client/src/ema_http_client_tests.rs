//! Renewable sessions use cached bearers without credential reads and never replay operations.

use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use codex_exec_server::RouteAwareHttpClient;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use pretty_assertions::assert_eq;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::*;
use crate::ema_auth_policy::EmaAuthFailure;
use crate::oauth::RefreshCredentialLock;

struct Source {
    endpoint: Url,
    renewals: AtomicUsize,
    valid: AtomicBool,
    invalidate_on_refresh: AtomicBool,
    credential_gate: Mutex<Option<CredentialGate>>,
    renewal_gate: Mutex<Option<CredentialGate>>,
    renewal_error: Mutex<Option<anyhow::Error>>,
    active_leases: AtomicUsize,
    renewal_with_lease: AtomicBool,
    lock_identity: Mutex<Option<(String, String)>>,
}

struct CredentialGate {
    entered: tokio::sync::oneshot::Sender<()>,
    release: tokio::sync::oneshot::Receiver<()>,
}

impl CredentialGate {
    fn new() -> (
        Self,
        tokio::sync::oneshot::Receiver<()>,
        tokio::sync::oneshot::Sender<()>,
    ) {
        let (entered, waiting) = tokio::sync::oneshot::channel();
        let (release, released) = tokio::sync::oneshot::channel();
        (
            Self {
                entered,
                release: released,
            },
            waiting,
            release,
        )
    }

    async fn wait(self) {
        let _ = self.entered.send(());
        let _ = self.release.await;
    }
}

struct CredentialLease {
    source: Arc<Source>,
    _lock: Option<RefreshCredentialLock>,
}

impl Drop for CredentialLease {
    fn drop(&mut self) {
        self.source
            .active_leases
            .fetch_sub(/*val*/ 1, Ordering::SeqCst);
    }
}

impl EmaTokenSource for Arc<Source> {
    fn endpoint(&self) -> &Url {
        &self.endpoint
    }
    async fn access_token(&self) -> anyhow::Result<EmaAccessToken> {
        if !self.valid.load(Ordering::SeqCst) {
            return Err(anyhow::Error::new(EmaAuthFailure::ReauthenticationRequired)
                .context("credentials were removed"));
        }
        let identity = self.lock_identity.lock().unwrap().clone();
        let lock = match identity {
            Some((name, issuer)) => {
                Some(RefreshCredentialLock::acquire_for_server(&name, &issuer).await?)
            }
            None => None,
        };
        self.active_leases.fetch_add(/*val*/ 1, Ordering::SeqCst);
        let _lease = CredentialLease {
            source: Arc::clone(self),
            _lock: lock,
        };
        let gate = self.credential_gate.lock().unwrap().take();
        if let Some(gate) = gate {
            gate.wait().await;
        }
        let renewal = self.renewals.fetch_add(/*val*/ 1, Ordering::SeqCst) + 1;
        self.renewal_with_lease.fetch_or(
            self.active_leases.load(Ordering::SeqCst) != 0,
            Ordering::SeqCst,
        );
        if let Some(error) = self.renewal_error.lock().unwrap().take() {
            return Err(error);
        }
        let received_at = Instant::now();
        let gate = self.renewal_gate.lock().unwrap().take();
        if let Some(gate) = gate {
            gate.wait().await;
        }
        tokio::task::yield_now().await;
        if self.invalidate_on_refresh.load(Ordering::SeqCst) {
            self.valid.store(false, Ordering::SeqCst);
            return Err(anyhow::Error::new(EmaAuthFailure::ReauthenticationRequired));
        }
        Ok(EmaAccessToken {
            access_token: format!("renewed-{renewal}"),
            expires_in: Some(Duration::from_secs(/*secs*/ 300)),
            received_at,
        })
    }
}

fn fixture(
    endpoint: &str,
) -> (
    EmaAuthenticatedHttpClient<Arc<Source>>,
    Arc<Source>,
    HttpRequestParams,
) {
    let source = Arc::new(Source {
        endpoint: Url::parse(endpoint).expect("endpoint"),
        renewals: AtomicUsize::new(0),
        valid: AtomicBool::new(true),
        invalidate_on_refresh: AtomicBool::new(false),
        credential_gate: Mutex::new(/*t*/ None),
        renewal_gate: Mutex::new(/*t*/ None),
        renewal_error: Mutex::new(/*t*/ None),
        active_leases: AtomicUsize::new(/*v*/ 0),
        renewal_with_lease: AtomicBool::new(/*v*/ false),
        lock_identity: Mutex::new(None),
    });
    let client = EmaAuthenticatedHttpClient::new(
        Arc::new(RouteAwareHttpClient::new(HttpClientFactory::new(
            OutboundProxyPolicy::ReqwestDefault,
        ))),
        Arc::clone(&source),
        EmaAccessToken {
            access_token: "initial".to_string(),
            expires_in: Some(Duration::from_secs(300)),
            received_at: Instant::now(),
        },
        EmaRequestAuthority::default(),
    );
    let params = HttpRequestParams {
        method: "POST".to_string(),
        url: endpoint.to_string(),
        headers: vec![HttpHeader {
            name: "Authorization".to_string(),
            value: "discard-me".to_string(),
            value_env_var: None,
        }],
        body: Some(br#"{"method":"tools/call"}"#.to_vec().into()),
        timeout_ms: Some(5000),
        redirect_policy: HttpRedirectPolicy::Follow,
        request_id: "operation".to_string(),
        stream_response: false,
    };
    (client, source, params)
}

#[tokio::test]
async fn cached_bearer_does_not_read_credentials_until_renewal() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    let (client, source, params) = fixture(&format!("{}/mcp", server.uri()));
    crate::oauth::test_support::warm_http_client(client.inner.as_ref()).await?;
    assert_eq!(client.http_request(params.clone()).await?.status, 200);
    source.valid.store(false, Ordering::SeqCst);
    assert_eq!(client.http_request(params.clone()).await?.status, 200);
    *client.cached_token() = None;
    assert!(matches!(
        client.http_request(params).await,
        Err(ExecServerError::AuthenticationRequired(_))
    ));
    assert_eq!(source.renewals.load(Ordering::SeqCst), 0);
    let requests = server.received_requests().await.expect("recorded requests");
    let bearers = requests
        .iter()
        .map(|request| {
            request.headers["authorization"]
                .to_str()
                .expect("bearer header")
                .to_string()
        })
        .collect::<Vec<_>>();
    assert_eq!(bearers, ["Bearer initial", "Bearer initial"]);
    Ok(())
}

#[tokio::test]
async fn revocation_during_renewal_prevents_dispatch() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    let endpoint = format!("{}/mcp", server.uri());
    let (gate, validated_rx, release_tx) = CredentialGate::new();
    let (client, source, params) = fixture(&endpoint);
    *client.cached_token() = None;
    *source.credential_gate.lock().unwrap() = Some(gate);
    let authority = client.authority.clone();
    let request = tokio::spawn(async move { client.http_request(params).await });
    validated_rx.await?;
    authority.revoke();
    let _ = release_tx.send(());
    tokio::time::timeout(Duration::from_secs(1), authority.quiesce()).await?;
    assert!(matches!(
        request.await?,
        Err(ExecServerError::AuthenticationRequired(_))
    ));
    assert!(
        server
            .received_requests()
            .await
            .expect("requests")
            .is_empty()
    );
    Ok(())
}

#[tokio::test]
async fn revocation_ends_an_existing_response_stream() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_string("account-a-data"))
        .mount(&server)
        .await;
    let (client, _source, params) = fixture(&format!("{}/mcp", server.uri()));
    crate::oauth::test_support::warm_http_client(client.inner.as_ref()).await?;
    let authority = client.authority.clone();
    let (_response, mut body) = client.http_request_stream(params).await?;

    authority.revoke();

    assert_eq!(body.recv().await?, None);
    Ok(())
}

#[tokio::test]
async fn revocation_interrupts_an_active_multichunk_response() -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("http://{}/mcp", listener.local_addr()?);
    let (chunks, receiver) = tokio::sync::mpsc::channel::<Result<bytes::Bytes, std::io::Error>>(2);
    let receiver = Arc::new(Mutex::new(Some(receiver)));
    let app = axum::Router::new().route(
        "/mcp",
        axum::routing::post(move || {
            let receiver = receiver.lock().unwrap().take().expect("one request");
            async move {
                let stream = futures::stream::unfold(receiver, |mut receiver| async {
                    receiver.recv().await.map(|chunk| (chunk, receiver))
                });
                (
                    [("content-type", "text/event-stream")],
                    axum::body::Body::from_stream(stream),
                )
            }
        }),
    );
    let server = tokio_util::task::AbortOnDropHandle::new(tokio::spawn(async move {
        axum::serve(listener, app).await
    }));
    let (client, _source, params) = fixture(&endpoint);
    chunks
        .send(Ok(bytes::Bytes::from_static(b"data: first\n\n")))
        .await?;
    let (_, mut body) = client.http_request_stream(params).await?;
    assert_eq!(body.recv().await?, Some(b"data: first\n\n".to_vec()));
    let mut next = Box::pin(body.recv());
    assert!(futures::poll!(&mut next).is_pending());

    client.authority.revoke();
    tokio::time::timeout(Duration::from_secs(1), client.authority.quiesce()).await?;
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(1), next).await??,
        None
    );
    // Even a later chunk cannot revive a stream whose authority was revoked.
    let _ = chunks
        .send(Ok(bytes::Bytes::from_static(b"data: second\n\n")))
        .await;
    assert_eq!(body.recv().await?, None);
    server.abort();
    let _ = server.await;
    Ok(())
}

#[tokio::test]
async fn unauthorized_requests_are_not_replayed_and_later_requests_renew() -> anyhow::Result<()> {
    for streaming in [false, true] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/mcp"))
            .respond_with(|request: &wiremock::Request| {
                ResponseTemplate::new(if request.headers["authorization"] == "Bearer initial" {
                    401
                } else {
                    200
                })
            })
            .mount(&server)
            .await;
        let (client, source, params) = fixture(&format!("{}/mcp", server.uri()));
        crate::oauth::test_support::warm_http_client(client.inner.as_ref()).await?;
        for (status, count) in [(401, 1), (200, 2), (200, 3)] {
            let result = if streaming {
                client
                    .http_request_stream(params.clone())
                    .await
                    .map(|response| response.0.status)
            } else {
                client
                    .http_request(params.clone())
                    .await
                    .map(|response| response.status)
            };
            if status == 401 {
                let error = result.expect_err("unauthorized operation fails without replay");
                assert!(is_authentication_required_error(&anyhow::Error::new(error)));
            } else {
                assert_eq!(result?, status);
            }
            assert_eq!(
                server.received_requests().await.expect("requests").len(),
                count
            );
        }
        assert_eq!(source.renewals.load(Ordering::SeqCst), 1);
        // Concurrent callers of an expiring token share one renewal.
        client
            .cached_token()
            .as_mut()
            .expect("cached token")
            .refresh_at = Instant::now();
        let results =
            futures::future::join_all((0..8).map(|_| client.http_request(params.clone()))).await;
        assert!(results.iter().all(Result::is_ok));
        assert_eq!(source.renewals.load(Ordering::SeqCst), 2);
    }
    Ok(())
}

#[path = "ema_http_client_regression_tests.rs"]
mod regressions;

#[tokio::test]
async fn initial_bearer_aged_during_validation_is_renewed_before_dispatch() -> anyhow::Result<()> {
    for streaming in [false, true] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;
        let (client, source, params) = fixture(&format!("{}/mcp", server.uri()));
        let client = EmaAuthenticatedHttpClient::new(
            client.inner,
            Arc::clone(&source),
            EmaAccessToken {
                access_token: "aged-in-validation".to_string(),
                expires_in: Some(Duration::from_secs(300)),
                received_at: Instant::now() - Duration::from_secs(301),
            },
            client.authority,
        );
        if streaming {
            client.http_request_stream(params).await?;
        } else {
            client.http_request(params).await?;
        }
        let requests = server.received_requests().await.expect("requests");
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].headers["authorization"], "Bearer renewed-1");
        assert_eq!(source.renewals.load(Ordering::SeqCst), 1);
    }
    Ok(())
}

#[tokio::test]
async fn renewal_aged_during_validation_is_not_dispatched_or_cached() -> anyhow::Result<()> {
    for streaming in [false, true] {
        let server = MockServer::start().await;
        let (client, source, mut params) = fixture(&format!("{}/mcp", server.uri()));
        params.timeout_ms = None;
        *client.cached_token() = None;
        let client = Arc::new(client);
        let (gate, waiting, release) = CredentialGate::new();
        *source.renewal_gate.lock().unwrap() = Some(gate);
        tokio::time::pause();
        let request = {
            let client = Arc::clone(&client);
            tokio::spawn(async move {
                if streaming {
                    client
                        .http_request_stream(params)
                        .await
                        .map(|(response, _)| response)
                } else {
                    client.http_request(params).await
                }
            })
        };
        waiting.await?;
        tokio::time::advance(Duration::from_secs(301)).await;
        release.send(()).expect("release validation");
        let result = request.await?;
        tokio::time::resume();
        assert!(matches!(result, Err(ExecServerError::HttpRequest(message))
            if message == "enterprise MCP bearer expired during credential validation"));
        assert!(client.cached_token().is_none());
        assert!(
            server
                .received_requests()
                .await
                .expect("requests")
                .is_empty()
        );
        assert_eq!(source.renewals.load(Ordering::SeqCst), 1);
    }
    Ok(())
}
