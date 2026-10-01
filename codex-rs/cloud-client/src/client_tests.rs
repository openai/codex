//! Exercises native HTTP/2 framing, metadata, trailers and cancellation with synthetic data.
use super::*;
use bytes::Bytes;
use codex_http_client::OutboundProxyPolicy;
use http_body_util::StreamBody;
use hyper::body::Frame;
use hyper::service::service_fn;
use hyper_util::rt::TokioExecutor;
use hyper_util::rt::TokioIo;
use pretty_assertions::assert_eq;
use std::convert::Infallible;
use tokio::sync::mpsc;

struct Fixture {
    client: Client,
    requests: mpsc::UnboundedReceiver<(http::request::Parts, Bytes)>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn fixture(response: http::Response<Body>) -> Fixture {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (sender, requests) = mpsc::unbounded_channel();
    let response = std::sync::Arc::new(std::sync::Mutex::new(Some(response)));
    let task = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let service = service_fn(move |request: http::Request<hyper::body::Incoming>| {
            let sender = sender.clone();
            let response = response.lock().unwrap().take().unwrap();
            async move {
                let (parts, body) = request.into_parts();
                sender
                    .send((parts, body.collect().await.unwrap().to_bytes()))
                    .unwrap();
                Ok::<_, Infallible>(response)
            }
        });
        let _ = hyper::server::conn::http2::Builder::new(TokioExecutor::new())
            .serve_connection(TokioIo::new(socket), service)
            .await;
    });
    let client = Client::new(
        &HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        &endpoint,
        Credentials::new("synthetic-token", "synthetic-account").unwrap(),
    )
    .unwrap();
    Fixture {
        client,
        requests,
        task,
    }
}
fn frame(payload: &[u8]) -> Bytes {
    let mut bytes = vec![0];
    bytes.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    bytes.extend_from_slice(payload);
    bytes.into()
}
fn response(messages: Vec<Bytes>, status: tonic::Status) -> http::Response<Body> {
    let frames = messages
        .into_iter()
        .map(Frame::data)
        .chain([Frame::trailers(
            status.into_http::<Body>().into_parts().0.headers,
        )]);
    http::Response::builder()
        .header("content-type", "application/grpc")
        .body(Body::new(StreamBody::new(futures::stream::iter(
            frames.map(Ok::<_, Infallible>),
        ))))
        .unwrap()
}
async fn assert_request(fixture: &mut Fixture, method: &str, expected: &[u8]) {
    let (parts, body) = tokio::time::timeout(Duration::from_secs(5), fixture.requests.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(parts.method, http::Method::POST);
    assert_eq!(parts.version, http::Version::HTTP_2);
    assert_eq!(
        parts.uri.path(),
        format!("/oaiproto.codex_app_server_backend.ThreadService/{method}")
    );
    assert_eq!(parts.headers["authorization"], "Bearer synthetic-token");
    assert_eq!(parts.headers["chatgpt-account-id"], "synthetic-account");
    assert_eq!(parts.headers["content-type"], "application/grpc");
    assert!(!parts.headers.contains_key("grpc-timeout"));
    assert_eq!(body, frame(expected));
}
#[tokio::test]
async fn resume_sends_protobuf_and_preserves_admission_flags() {
    for wait in [false, true] {
        let mut fixture = fixture(response(vec![frame(&[])], tonic::Status::ok(""))).await;
        fixture
            .client
            .resume(&ResumeRequest {
                thread_id: "t".into(),
                wait,
                shared_access: true,
            })
            .await
            .unwrap();
        // Independent protobuf wire fixtures, not the client's encoder.
        let expected: &[u8] = if wait {
            &[10, 1, b't', 16, 1, 24, 1]
        } else {
            &[10, 1, b't', 24, 1]
        };
        assert_request(&mut fixture, "Resume", expected).await;
    }
}
#[tokio::test]
async fn attach_preserves_opaque_events_and_native_error_details() {
    let secret = Bytes::from_static(b"private-error-details");
    let status = tonic::Status::with_details(
        tonic::Code::PermissionDenied,
        "private-message",
        secret.clone(),
    );
    let data = [frame(&[10, 3, 10, 1, b'n']), frame(&[18, 3, 10, 1, b'r'])].concat();
    let mut fixture = fixture(response(
        data.iter().map(|b| Bytes::from(vec![*b])).collect(),
        status,
    ))
    .await;
    let mut events = fixture.client.attach("t").await.unwrap();
    assert_request(&mut fixture, "Attach", &[10, 1, b't']).await;
    let notification = events.next().await.unwrap().unwrap();
    assert_eq!(format!("{notification:?}"), "Notification([redacted])");
    let Event::Notification(bytes) = notification else {
        panic!("notification expected")
    };
    assert_eq!(bytes.as_ref(), &[10, 1, b'n']);
    let Event::ServerRequest(bytes) = events.next().await.unwrap().unwrap() else {
        panic!("server request expected")
    };
    assert_eq!(bytes.as_ref(), &[10, 1, b'r']);
    let error = events.next().await.unwrap().unwrap_err();
    assert!(!format!("{error:?} {error}").contains("private"));
    let Error::Rpc(status) = error else {
        panic!("native status expected")
    };
    assert_eq!(
        (status.code, status.message, status.details),
        (7, "private-message".into(), secret)
    );
    assert!(events.next().await.is_none());
}
#[tokio::test]
async fn successful_stream_ends_and_bad_frames_terminate() {
    let fixture = fixture(response(vec![], tonic::Status::ok(""))).await;
    assert!(
        fixture
            .client
            .attach("t")
            .await
            .unwrap()
            .next()
            .await
            .is_none()
    );
    for message in [
        frame(&[]),
        frame(&[10, 99]),
        Bytes::from_static(&[0, 4, 0, 0, 1]),
    ] {
        let fixture = self::fixture(response(vec![message], tonic::Status::ok(""))).await;
        let mut stream = fixture.client.attach("t").await.unwrap();
        assert!(stream.next().await.unwrap().is_err());
        assert!(stream.next().await.is_none());
    }
}
#[tokio::test]
async fn unary_checks_status_trailers_and_preserves_details() {
    let fixture = fixture(response(
        vec![frame(&[])],
        tonic::Status::with_details(
            tonic::Code::Unavailable,
            "secret",
            Bytes::from_static(b"details"),
        ),
    ))
    .await;
    let error = fixture
        .client
        .resume(&ResumeRequest {
            thread_id: "t".into(),
            wait: false,
            shared_access: false,
        })
        .await
        .unwrap_err();
    let Error::Rpc(status) = error else {
        panic!("RPC status expected")
    };
    assert_eq!(
        (status.code, status.details),
        (14, Bytes::from_static(b"details"))
    );
}
#[tokio::test]
async fn dropping_attachment_cancels_idle_response() {
    let (sender, dropped) = tokio::sync::oneshot::channel::<()>();
    let stream = async_stream::stream! {
        let _sender = sender;
        yield Ok::<_, Infallible>(Frame::data(frame(&[10, 0])));
        std::future::pending::<()>().await;
    };
    let fixture = fixture(
        http::Response::builder()
            .header("content-type", "application/grpc")
            .body(Body::new(StreamBody::new(stream)))
            .unwrap(),
    )
    .await;
    let mut events = fixture.client.attach("t").await.unwrap();
    events.next().await.unwrap().unwrap();
    drop(events);
    assert!(
        tokio::time::timeout(Duration::from_secs(5), dropped)
            .await
            .unwrap()
            .is_err()
    );
}
#[tokio::test]
async fn redirects_and_http_failures_are_not_followed() {
    for status in [307, 403] {
        let fixture = fixture(
            http::Response::builder()
                .status(status)
                .header("location", "http://127.0.0.1:1/private")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        let error = match fixture.client.attach("t").await {
            Ok(mut stream) => stream.next().await.unwrap().unwrap_err(),
            Err(error) => error,
        };
        assert!(matches!(error, Error::Rpc(_)));
        assert!(!format!("{error:?} {error}").contains("private"));
    }
}
#[test]
fn rejects_unsafe_origins_and_redacts_credentials() {
    for endpoint in [
        "http://example.com",
        "https://user:secret@example.com",
        "https://example.com?token=secret",
        "https://example.com#secret",
        "https://example.com/grpc",
    ] {
        assert!(matches!(
            Client::new(
                &HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
                endpoint,
                Credentials::new("token", "account").unwrap(),
            ),
            Err(Error::InvalidEndpoint)
        ));
    }
    assert!(Credentials::new("secret\nvalue", "account").is_err());
    assert!(
        !format!(
            "{:?}",
            Credentials::new("synthetic-token", "synthetic-account").unwrap()
        )
        .contains("synthetic")
    );
}

#[test]
fn loopback_http_rejects_managed_and_endpoint_restricted_policies() {
    for policy in [
        codex_http_client::NetworkPolicyController::default().policy(),
        codex_http_client::NetworkPolicy::unmanaged().restrict_to_endpoints(Default::default()),
    ] {
        let factory = HttpClientFactory::new(OutboundProxyPolicy::RespectSystemProxy)
            .with_network_policy(policy);
        assert!(matches!(
            Client::new(
                &factory,
                "http://127.0.0.1:1234",
                Credentials::new("token", "account").unwrap(),
            ),
            Err(Error::InvalidEndpoint)
        ));
    }
}

#[tokio::test]
async fn https_advertises_h2_in_tls_client_hello() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client = HttpClientBuilder::new()
        .http2_prior_knowledge()
        .build_direct()
        .unwrap();
    let request = client.get(format!("https://{}", listener.local_addr().unwrap()));
    let handshake = async {
        let (socket, _) = listener.accept().await.unwrap();
        let hello = tokio_rustls::LazyConfigAcceptor::new(Default::default(), socket)
            .await
            .unwrap();
        let protocols: Vec<_> = hello
            .client_hello()
            .alpn()
            .into_iter()
            .flatten()
            .map(<[u8]>::to_vec)
            .collect();
        assert_eq!(protocols, vec![b"h2".to_vec()]);
        // Inspect negotiation without trusting a test CA or completing the TLS handshake.
    };
    tokio::time::timeout(Duration::from_secs(5), async {
        let (_, result) = tokio::join!(handshake, request.send());
        assert!(result.is_err());
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn https_enforces_unmanaged_endpoint_restrictions_before_connecting() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let factory = HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault).with_network_policy(
        codex_http_client::NetworkPolicy::unmanaged().restrict_to_endpoints(Default::default()),
    );
    let client = Client::new(
        &factory,
        &format!("https://{}", listener.local_addr().unwrap()),
        Credentials::new("token", "account").unwrap(),
    )
    .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        tokio::select! {
            result = client.attach("t") => assert!(result.is_err()),
            _ = listener.accept() => panic!("denied destination was contacted"),
        }
    })
    .await
    .unwrap();
}
