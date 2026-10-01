//! Authentication, deadlines, and concurrent responses share the same request invariants.

use pretty_assertions::assert_eq;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use super::*;
use crate::ema_auth_policy::EmaInvalidGrantSource;

type Client = EmaAuthenticatedHttpClient<Arc<Source>>;

#[derive(Clone, Copy)]
enum RequestMode {
    Buffered,
    Streaming,
}

impl RequestMode {
    fn start(
        self,
        client: Arc<Client>,
        mut params: HttpRequestParams,
    ) -> tokio::task::JoinHandle<Result<HttpRequestResponse, ExecServerError>> {
        tokio::spawn(async move {
            params.stream_response = matches!(self, Self::Streaming);
            match self {
                Self::Buffered => client.http_request(params).await,
                Self::Streaming => client
                    .http_request_stream(params)
                    .await
                    .map(|(response, _body)| response),
            }
        })
    }
}

#[derive(Debug)]
struct PendingRequest {
    params: HttpRequestParams,
    response: oneshot::Sender<u16>,
}

struct ControlledHttpClient(mpsc::UnboundedSender<PendingRequest>);

impl HttpClient for ControlledHttpClient {
    fn http_request(
        &self,
        params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<HttpRequestResponse, ExecServerError>> {
        Box::pin(async move {
            let (response, receiver) = oneshot::channel();
            self.0
                .send(PendingRequest { params, response })
                .expect("capture HTTP request");
            Ok(HttpRequestResponse {
                status: receiver.await.expect("provide HTTP response"),
                headers: Vec::new(),
                body: Vec::new().into(),
            })
        })
    }

    fn http_request_stream(
        &self,
        params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<(HttpRequestResponse, HttpResponseBodyStream), ExecServerError>> {
        Box::pin(async move {
            Ok((
                self.http_request(params).await?,
                HttpResponseBodyStream::from_chunks(Vec::new()),
            ))
        })
    }
}

fn controlled_fixture() -> (
    Arc<Client>,
    Arc<Source>,
    HttpRequestParams,
    mpsc::UnboundedReceiver<PendingRequest>,
) {
    controlled_fixture_for_endpoint("https://mcp.example/mcp")
}

fn controlled_fixture_for_endpoint(
    endpoint: &str,
) -> (
    Arc<Client>,
    Arc<Source>,
    HttpRequestParams,
    mpsc::UnboundedReceiver<PendingRequest>,
) {
    let (mut client, source, params) = fixture(endpoint);
    let (sender, receiver) = mpsc::unbounded_channel();
    client.inner = Arc::new(ControlledHttpClient(sender));
    (Arc::new(client), source, params, receiver)
}

#[tokio::test]
async fn exact_endpoint_and_renewal_credentials_are_checked_before_network_use()
-> anyhow::Result<()> {
    let endpoint = "https://mcp.example/mcp?tenant=one";
    for mode in [RequestMode::Buffered, RequestMode::Streaming] {
        let (client, source, params, mut requests) = controlled_fixture_for_endpoint(endpoint);
        for target in [
            endpoint.replace("one", "two"),
            endpoint.replace("/mcp", "/other"),
            "https://other.example/mcp?tenant=one".to_string(),
            format!("{endpoint}#fragment"),
        ] {
            let mut request = params.clone();
            request.url = target;
            assert!(matches!(
                mode.start(Arc::clone(&client), request).await?,
                Err(ExecServerError::HttpRequest(message))
                    if message == "refusing to send enterprise bearer outside the configured MCP endpoint"
            ));
        }
        source.valid.store(false, Ordering::SeqCst);
        *client.cached_token() = None;
        assert!(matches!(
            mode.start(Arc::clone(&client), params.clone()).await?,
            Err(ExecServerError::AuthenticationRequired(_))
        ));
        source.valid.store(true, Ordering::SeqCst);
        source.invalidate_on_refresh.store(true, Ordering::SeqCst);
        *client.cached_token() = None;
        assert!(matches!(
            mode.start(client, params).await?,
            Err(ExecServerError::AuthenticationRequired(_))
        ));
        assert!(requests.try_recv().is_err());
    }
    Ok(())
}

#[tokio::test]
async fn outgoing_requests_replace_all_authorization_headers_and_stop_redirects()
-> anyhow::Result<()> {
    for mode in [RequestMode::Buffered, RequestMode::Streaming] {
        let (client, _source, mut params, mut requests) = controlled_fixture();
        params
            .headers
            .extend(["authorization", "aUtHoRiZaTiOn"].map(|name| HttpHeader {
                name: name.to_string(),
                value: "another-stale-credential".to_string(),
                value_env_var: None,
            }));
        let response = mode.start(client, params);
        let request = requests.recv().await.expect("authenticated request");
        assert_eq!(request.params.redirect_policy, HttpRedirectPolicy::Stop);
        assert_eq!(
            request.params.headers,
            vec![HttpHeader {
                name: "authorization".to_string(),
                value: "Bearer initial".to_string(),
                value_env_var: None,
            }],
            "replace every case variant with exactly one bearer"
        );
        request.response.send(/*t*/ 200).expect("complete request");
        assert_eq!(response.await??.status, 200);
    }
    Ok(())
}

#[tokio::test]
async fn resource_bearers_never_follow_redirects() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(307).insert_header("location", "/mcp"))
        .expect(2)
        .mount(&server)
        .await;
    let (client, _source, params) = fixture(&format!("{}/mcp", server.uri()));
    crate::oauth::test_support::warm_http_client(client.inner.as_ref()).await?;
    let client = crate::http_client_redirect::SameOriginRedirectHttpClient::new(Arc::new(client));
    for result in [
        client.http_request(params.clone()).await.map(|_| ()),
        client.http_request_stream(params).await.map(|_| ()),
    ] {
        assert!(matches!(
            result,
            Err(ExecServerError::HttpRequest(message))
                if message == "enterprise MCP redirects are not allowed"
        ));
    }
    Ok(())
}

fn bearer(request: &PendingRequest) -> &str {
    &request
        .params
        .headers
        .iter()
        .find(|header| header.name == "authorization")
        .expect("bearer header")
        .value
}

#[tokio::test]
async fn credential_failures_are_classified_before_dispatch() -> anyhow::Result<()> {
    for mode in [RequestMode::Buffered, RequestMode::Streaming] {
        for failure in [
            EmaAuthFailure::InvalidGrant {
                grant_source: EmaInvalidGrantSource::EnterpriseIdentity,
            },
            EmaAuthFailure::InsufficientUserAuthentication,
            EmaAuthFailure::ReauthenticationRequired,
        ] {
            let (client, source, params, mut requests) = controlled_fixture();
            *client.cached_token() = None;
            match failure {
                EmaAuthFailure::ReauthenticationRequired => {
                    source.valid.store(/*val*/ false, Ordering::SeqCst);
                }
                failure => {
                    *source.renewal_error.lock().unwrap() =
                        Some(anyhow::Error::new(failure).context("token exchange failed"));
                }
            }
            assert!(matches!(
                mode.start(client, params).await?,
                Err(ExecServerError::AuthenticationRequired(_))
            ));
            assert!(requests.try_recv().is_err());
        }
        for error in [
            anyhow::Error::new(EmaAuthFailure::InvalidGrant {
                grant_source: EmaInvalidGrantSource::ResourceAuthorization,
            })
            .context("resource authorization failed without invalidating the enterprise identity"),
            anyhow::anyhow!("temporary network failure"),
        ] {
            let (client, source, params, mut requests) = controlled_fixture();
            *client.cached_token() = None;
            *source.renewal_error.lock().unwrap() = Some(error);
            assert!(matches!(
                mode.start(client, params).await?,
                Err(ExecServerError::HttpRequest(_))
            ));
            assert!(requests.try_recv().is_err());
        }
    }
    Ok(())
}

#[tokio::test]
async fn delayed_unauthorized_response_preserves_the_renewed_bearer() -> anyhow::Result<()> {
    for mode in [RequestMode::Buffered, RequestMode::Streaming] {
        let (client, source, params, mut requests) = controlled_fixture();
        let first = mode.start(Arc::clone(&client), params.clone());
        let first_request = requests.recv().await.expect("first request");
        let delayed = mode.start(Arc::clone(&client), params.clone());
        let delayed_request = requests.recv().await.expect("delayed request");
        assert_eq!(
            (bearer(&first_request), bearer(&delayed_request)),
            ("Bearer initial", "Bearer initial")
        );

        first_request.response.send(/*t*/ 401).expect("first 401");
        assert!(matches!(
            first.await?,
            Err(ExecServerError::AuthenticationRequired(_))
        ));
        let renewed = mode.start(Arc::clone(&client), params.clone());
        let renewed_request = requests.recv().await.expect("renewed request");
        assert_eq!(bearer(&renewed_request), "Bearer renewed-1");
        renewed_request
            .response
            .send(/*t*/ 200)
            .expect("renewed response");
        assert_eq!(renewed.await??.status, 200);

        delayed_request
            .response
            .send(/*t*/ 401)
            .expect("delayed 401");
        assert!(matches!(
            delayed.await?,
            Err(ExecServerError::AuthenticationRequired(_))
        ));
        let next = mode.start(Arc::clone(&client), params);
        let next_request = requests.recv().await.expect("next request");
        assert_eq!(bearer(&next_request), "Bearer renewed-1");
        next_request
            .response
            .send(/*t*/ 200)
            .expect("next response");
        assert_eq!(next.await??.status, 200);
        assert_eq!(source.renewals.load(Ordering::SeqCst), 1);
        assert!(requests.try_recv().is_err(), "no operation was replayed");
    }
    Ok(())
}

#[tokio::test]
async fn authentication_waits_share_the_request_deadline() -> anyhow::Result<()> {
    #[derive(Clone, Copy)]
    enum Wait {
        Authority,
        Renewal,
        Credentials,
        Exchange,
    }
    for mode in [RequestMode::Buffered, RequestMode::Streaming] {
        for wait in [
            Wait::Authority,
            Wait::Renewal,
            Wait::Credentials,
            Wait::Exchange,
        ] {
            let (client, source, mut params, mut requests) = controlled_fixture();
            let (gate, _entered, _release) = CredentialGate::new();
            let mut authority = None;
            let mut renewal = None;
            match wait {
                Wait::Authority => authority = Some(client.authority.responses.write().await),
                Wait::Renewal => {
                    *client.cached_token() = None;
                    renewal = Some(client.renewal.acquire().await?);
                }
                Wait::Credentials => {
                    *client.cached_token() = None;
                    *source.credential_gate.lock().unwrap() = Some(gate);
                }
                Wait::Exchange => {
                    *client.cached_token() = None;
                    *source.renewal_gate.lock().unwrap() = Some(gate);
                }
            }
            params.timeout_ms = Some(20);
            let response = tokio::time::timeout(
                Duration::from_secs(/*secs*/ 1),
                mode.start(Arc::clone(&client), params),
            )
            .await??;
            assert!(
                matches!(response, Err(ExecServerError::HttpRequest(message))
                if message == "enterprise MCP request timed out")
            );
            assert!(requests.try_recv().is_err());
            drop((authority, renewal));
            assert_eq!(source.active_leases.load(Ordering::SeqCst), 0);
        }
        let (client, source, mut params, mut requests) = controlled_fixture();
        params.timeout_ms = Some(0);
        assert!(matches!(
            mode.start(client, params).await?,
            Err(ExecServerError::HttpRequest(_))
        ));
        assert_eq!(source.renewals.load(Ordering::SeqCst), 0);
        assert!(requests.try_recv().is_err());
    }
    Ok(())
}

#[tokio::test]
async fn authentication_time_is_removed_from_the_inner_timeout() -> anyhow::Result<()> {
    for mode in [RequestMode::Buffered, RequestMode::Streaming] {
        for timeout in [Some(1000), None] {
            let (client, source, mut params, mut requests) = controlled_fixture();
            let (gate, entered, release) = CredentialGate::new();
            *client.cached_token() = None;
            *source.credential_gate.lock().unwrap() = Some(gate);
            params.timeout_ms = timeout;
            let response = mode.start(client, params);
            entered.await?;
            tokio::time::sleep(Duration::from_millis(/*millis*/ 30)).await;
            release.send(()).expect("release credentials");
            let request = requests.recv().await.expect("authenticated request");
            match timeout {
                Some(timeout) => assert!(
                    request
                        .params
                        .timeout_ms
                        .is_some_and(|remaining| remaining > 0 && remaining <= timeout - 30)
                ),
                None => assert_eq!(request.params.timeout_ms, None),
            }
            request.response.send(/*t*/ 200).expect("complete request");
            assert_eq!(response.await??.status, 200);
        }
    }
    Ok(())
}
