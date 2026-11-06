//! Verifies fresh handshake credentials and rejection after the selected credentials change.

use super::*;
use crate::AuthError;
use crate::AuthHeadersFuture;
use crate::AuthProvider;
use crate::provider::RetryConfig;
use http::HeaderName;
use http::header::AUTHORIZATION;
use pretty_assertions::assert_eq;
use std::sync::atomic::AtomicUsize;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_hdr_async;
use tokio_tungstenite::tungstenite::handshake::server::Request;
use tokio_tungstenite::tungstenite::handshake::server::Response;

struct HandshakeAuth(AtomicUsize);

impl AuthProvider for HandshakeAuth {
    fn add_auth_headers(&self, _headers: &mut HeaderMap) -> Result<(), AuthError> {
        Err(AuthError::Build(
            "authentication requires async header resolution".to_string(),
        ))
    }

    fn apply_auth_headers(&self, mut headers: HeaderMap) -> AuthHeadersFuture<'_> {
        Box::pin(async move {
            let attempt = self.0.fetch_add(1, Ordering::SeqCst) + 1;
            headers.remove("x-openai-agent-assertion");
            headers.insert(
                AUTHORIZATION,
                format!("AgentAssertion proof-{attempt}").parse().unwrap(),
            );
            Ok(headers)
        })
    }
}

fn client(base_url: String) -> RealtimeWebsocketClient {
    RealtimeWebsocketClient::new(
        Provider {
            name: "test".to_string(),
            base_url: base_url.clone(),
            query_params: None,
            headers: HeaderMap::new(),
            retry: RetryConfig {
                max_attempts: 1,
                base_delay: Duration::from_millis(1),
                retry_429: false,
                retry_5xx: true,
                retry_transport: true,
            },
            stream_idle_timeout: Duration::from_secs(5),
        },
        codex_http_client::HttpClientFactory::new(
            codex_http_client::OutboundProxyPolicy::ReqwestDefault,
        ),
    )
    .with_webrtc_sideband_base_url(base_url)
}

fn session() -> RealtimeSessionConfig {
    RealtimeSessionConfig {
        instructions: String::new(),
        initial_items: Vec::new(),
        delegation_ack_filler: None,
        model: None,
        session_id: None,
        event_parser: RealtimeEventParser::V1,
        session_mode: RealtimeSessionMode::Conversational,
        output_modality: RealtimeOutputModality::Audio,
        voice: RealtimeVoice::Cedar,
    }
}

#[tokio::test]
async fn sideband_retry_and_reconnect_replace_cached_auth_headers() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let base_url = format!("http://{}/v1", listener.local_addr()?);
    let server = tokio::spawn(async move {
        let mut requests = Vec::new();
        for attempt in 0..3 {
            let (stream, _) = listener.accept().await.unwrap();
            let result = accept_hdr_async(stream, |request: &Request, response: Response| {
                requests.push(request.headers().clone());
                if attempt == 0 {
                    Err(http::Response::builder().status(503).body(None).unwrap())
                } else {
                    Ok(response)
                }
            })
            .await;
            if attempt == 0 {
                assert!(result.is_err());
            } else {
                let mut stream = result.unwrap();
                assert!(matches!(stream.next().await, Some(Ok(Message::Close(_)))));
            }
        }
        requests
    });
    let client = client(base_url).with_auth(Arc::new(HandshakeAuth(AtomicUsize::new(0))));
    let headers = HeaderMap::from_iter([
        (
            AUTHORIZATION,
            HeaderValue::from_static("Bearer stale-credential"),
        ),
        (
            HeaderName::from_static("x-openai-agent-assertion"),
            HeaderValue::from_static("stale-proof"),
        ),
        (
            HeaderName::from_static("x-request-context"),
            HeaderValue::from_static("keep"),
        ),
    ]);
    for _ in 0..2 {
        let connection = tokio::time::timeout(
            Duration::from_secs(5),
            client.connect_existing_call_sideband(
                session(),
                "rtc_test",
                headers.clone(),
                HeaderMap::new(),
                RealtimeTranscriptState::default(),
            ),
        )
        .await??;
        connection.close().await?;
    }
    let requests = tokio::time::timeout(Duration::from_secs(5), server).await??;
    assert_eq!(requests.len(), 3);
    for (index, headers) in requests.iter().enumerate() {
        assert_eq!(
            headers[AUTHORIZATION],
            format!("AgentAssertion proof-{}", index + 1)
        );
        assert!(!headers.contains_key("x-openai-agent-assertion"));
        assert_eq!(headers["x-request-context"], "keep");
    }
    Ok(())
}

struct RejectedAuth;

impl AuthProvider for RejectedAuth {
    fn add_auth_headers(&self, _headers: &mut HeaderMap) -> Result<(), AuthError> {
        Err(AuthError::Build(
            "authentication requires async header resolution".to_string(),
        ))
    }

    fn apply_auth_headers(&self, _headers: http::HeaderMap) -> AuthHeadersFuture<'_> {
        Box::pin(async { Err(AuthError::Build("runtime revoked".to_string())) })
    }
}

#[tokio::test]
async fn rejected_auth_prevents_the_websocket_request() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let client =
        client(format!("http://{}/v1", listener.local_addr()?)).with_auth(Arc::new(RejectedAuth));
    let result = client
        .connect(session(), HeaderMap::new(), HeaderMap::new())
        .await;
    assert!(result.is_err());
    assert!(
        tokio::time::timeout(Duration::from_millis(50), listener.accept())
            .await
            .is_err()
    );
    Ok(())
}
