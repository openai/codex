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
use prost::Message;
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
    fixture_responses(vec![response]).await
}
async fn fixture_responses(responses: Vec<http::Response<Body>>) -> Fixture {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (sender, requests) = mpsc::unbounded_channel();
    let responses = std::sync::Arc::new(std::sync::Mutex::new(std::collections::VecDeque::from(
        responses,
    )));
    let task = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let service = service_fn(move |request: http::Request<hyper::body::Incoming>| {
            let sender = sender.clone();
            let response = responses.lock().unwrap().pop_front().unwrap();
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
async fn activation_sends_only_identity_and_excludes_turns() {
    let mut fixture = fixture(response(vec![frame(&[])], tonic::Status::ok(""))).await;
    assert!(matches!(
        fixture.client.activate("t").await,
        Err(MutationError::OutcomeUnknown(Error::InvalidResponse))
    ));
    // exclude_turns=true (field 7), thread_id="t" (field 18).
    assert_request(&mut fixture, "UpdateConfig", &[56, 1, 0x92, 1, 1, b't']).await;
}

#[tokio::test]
async fn activation_preserves_native_settings_and_thread_metadata() {
    // Native Thread fields encoded independently of the client's wire types.
    let thread_base: &[u8] = &[
        0x22, 1, b'v', // cli_version (4)
        0x28, 1, // created_at (5)
        0x32, 1, b'/', // cwd (6)
        0x48, 0, // ephemeral (9)
        0x72, 1, b't', // id (14)
        0x82, 1, 1, b'p', // model_provider (16)
        0xaa, 1, 0, // preview (21)
        0xda, 1, 1, b's', // session_id (27)
        0xe2, 1, 2, 8, 99, // source (28): future simple value
        0xea, 1, 2, 0x12, 0, // status (29): idle
        0xfa, 1, 0, // turns (31): empty
        0x80, 2, 2, // updated_at (32)
    ];
    let encode_response = |mode: u8, daybreak: Option<bool>, approval: u8| {
        let mut thread = thread_base.to_vec();
        if let Some(enabled) = daybreak {
            thread.extend_from_slice(&[0x38, u8::from(enabled)]); // daybreak_enabled (7)
        }
        // Settings has developer_instructions (1), model (2), and reasoning_effort (3).
        let settings = Bytes::from_static(if mode == 1 {
            b"\x0a\x01d\x12\x01m\x1a\x04high"
        } else {
            b"\x12\x01m"
        });
        let mut collaboration_mode = vec![8, mode];
        prost::encoding::bytes::encode(/* settings */ 2, &settings, &mut collaboration_mode);
        let mut response_bytes = vec![0x12, 2, 8, approval, 0x18, 1]; // approval and reviewer
        prost::encoding::bytes::encode(
            /* collaboration_mode */ 4,
            &collaboration_mode,
            &mut response_bytes,
        );
        // instruction_sources (8) wraps repeated string values (1).
        response_bytes.extend_from_slice(b"\x42\x0c\x0a\x0a/AGENTS.md");
        response_bytes.extend_from_slice(&[
            0x2a, 1, b'/', // cwd (5)
            0x52, 1, b'm', // model (10)
            0x5a, 1, b'p', // model_provider (11)
            0x7a, 4, 0x12, 2, 8, 1, // sandbox (15): read-only with network access
        ]);
        prost::encoding::bytes::encode(/* thread */ 17, &thread, &mut response_bytes);
        (response_bytes, thread)
    };

    // Activation retains stricter runtime requirements than stored list metadata.
    for (session_id, model_provider) in [("", "p"), ("s", "")] {
        let (response_bytes, _) = encode_response(1, Some(true), 2);
        let mut settings =
            crate::activation_wire::UpdateConfigResponse::decode(Bytes::from(response_bytes))
                .unwrap();
        let mut thread =
            crate::activation_wire::Thread::decode(settings.thread.take().unwrap()).unwrap();
        thread.session_id = Some(session_id.into());
        thread.model_provider = Some(model_provider.into());
        settings.thread = Some(thread.encode_to_vec().into());
        assert!(matches!(
            ThreadActivation::from_settings(settings),
            Err(Error::InvalidResponse)
        ));
    }

    for (mode, daybreak) in [(1, Some(true)), (2, Some(false)), (2, None)] {
        let (response_bytes, thread) = encode_response(mode, daybreak, 2);
        let mut server = fixture(response(
            vec![frame(&response_bytes)],
            tonic::Status::ok(""),
        ))
        .await;
        let activation = server.client.activate("t").await.unwrap();
        assert_eq!(activation.thread.protobuf().as_ref(), thread.as_slice());
        let json = serde_json::to_value(&activation).unwrap();
        assert_eq!(json["thread"]["source"], "unknown");
        assert_eq!(
            json["thread"].get("daybreakEnabled"),
            Some(&serde_json::json!(daybreak))
        );
        assert_eq!(
            json["instructionSources"],
            serde_json::json!(["/AGENTS.md"])
        );
        assert_eq!(json["approvalPolicy"], "on-request");
        assert_eq!(json["approvalsReviewer"], "user");
        assert_eq!(
            json["sandbox"],
            serde_json::json!({"type": "readOnly", "networkAccess": true})
        );
        assert_eq!(
            json["collaborationMode"],
            serde_json::json!({
                "mode": if mode == 1 { "plan" } else { "default" },
                "settings": {
                    "model": "m",
                    "developer_instructions": if mode == 1 { Some("d") } else { None },
                    "reasoning_effort": if mode == 1 { Some("high") } else { None }
                }
            })
        );
        assert_request(&mut server, "UpdateConfig", &[56, 1, 0x92, 1, 1, b't']).await;
        assert!(server.requests.try_recv().is_err());
    }

    // Unknown security policy must remain an error, never a default.
    let (response_bytes, _) = encode_response(1, Some(true), 99);
    let mut server = fixture(response(
        vec![frame(&response_bytes)],
        tonic::Status::ok(""),
    ))
    .await;
    assert!(matches!(
        server.client.activate("t").await,
        Err(MutationError::OutcomeUnknown(Error::UnsupportedResponse(
            "approval policy"
        )))
    ));
    assert_request(&mut server, "UpdateConfig", &[56, 1, 0x92, 1, 1, b't']).await;
    assert!(server.requests.try_recv().is_err());
}

#[tokio::test]
async fn prewarm_encodes_blank_and_project_managed_environments() {
    let mut fixture = fixture_responses(vec![
        response(vec![frame(&[])], tonic::Status::ok("")),
        response(vec![frame(&[])], tonic::Status::ok("")),
    ])
    .await;

    assert!(matches!(
        fixture
            .client
            .prewarm_default_thread(/*model*/ None, /*model_provider*/ None)
            .await,
        Err(Error::InvalidResponse)
    ));
    assert_request(&mut fixture, "Prewarm", b"\x52\x04\x0a\x02\x0a\x00").await;

    assert!(matches!(
        fixture
            .client
            .prewarm_thread(
                /*model*/ None,
                /*model_provider*/ None,
                Some(ProjectEnvironment {
                    name: "App".to_owned(),
                    cwd: Some("/workspace/acme/app".to_owned()),
                    origin_urls: vec![
                        "git@github.com:acme/app.git".to_owned(),
                        "https://github.com/acme/lib.git".to_owned(),
                    ],
                }),
            )
            .await,
        Err(Error::InvalidResponse)
    ));
    let expected = [
        b"\x52\x73\x0a\x71\x0a\x6f\x0a\x0d" as &[u8],
        b"asenvcfg_base",
        b"\x12\x13/workspace/acme/app",
        b"\x1a\x49\x0a\x42\x0a\x1d\x2a\x1b",
        b"git@github.com:acme/app.git",
        b"\x0a\x21\x2a\x1f",
        b"https://github.com/acme/lib.git",
        b"\x12\x03App",
    ]
    .concat();
    assert_request(&mut fixture, "Prewarm", &expected).await;
}

#[tokio::test]
async fn prewarm_uses_managed_defaults_and_preserves_resolved_settings() {
    // PrewarmThread uses Thread field numbers but omits turns entirely.
    let thread = Bytes::from_static(&[
        0x22, 1, b'v', // cli_version (4)
        0x28, 1, // created_at (5)
        0x32, 1, b'/', // cwd (6)
        0x48, 0, // ephemeral (9)
        0x72, 1, b't', // id (14)
        0x82, 1, 1, b'p', // model_provider (16)
        0xaa, 1, 0, // preview (21)
        0xda, 1, 1, b's', // session_id (27)
        0xe2, 1, 2, 8, 1, // source (28): cli
        0xea, 1, 2, 0x12, 0, // status (29): idle
        0x80, 2, 2, // updated_at (32)
        0xa0, 6, 7, // unknown field (100)
    ]);
    let native_thread = [b"\xfa\x01\x00".as_slice(), thread.as_ref()].concat(); // empty turns (31)
    // The settings use different field numbers in Prewarm and UpdateConfig.
    let fields = [
        (1, 1, b"\x0a\x01e\x12\x01i".as_slice()), // active permission profile
        (2, 2, b"\x08\x02".as_slice()),           // on-request approval
        (4, 5, b"/".as_slice()),                  // cwd
        (7, 10, b"m".as_slice()),                 // model
        (8, 11, b"p".as_slice()),                 // provider
        (10, 13, b"high".as_slice()),             // reasoning effort
        (11, 14, b"\x0a\x01/".as_slice()),        // workspace roots
        (12, 15, b"\x12\x02\x08\x01".as_slice()), // read-only sandbox
        (13, 16, b"\x0a\x04fast".as_slice()),     // service tier
    ];
    let mut prewarm = vec![0x18, 1]; // approvals reviewer (3): user
    let mut activation = prewarm.clone();
    for (prewarm_tag, activation_tag, bytes) in fields {
        prost::encoding::bytes::encode(prewarm_tag, &bytes.to_vec(), &mut prewarm);
        prost::encoding::bytes::encode(activation_tag, &bytes.to_vec(), &mut activation);
    }
    prost::encoding::bytes::encode(/* tag */ 14, &thread, &mut prewarm);
    prost::encoding::bytes::encode(/* tag */ 17, &native_thread, &mut activation);
    let mut server = fixture_responses(vec![
        response(vec![frame(&activation)], tonic::Status::ok("")),
        response(vec![frame(&prewarm)], tonic::Status::ok("")),
        response(vec![frame(&prewarm)], tonic::Status::ok("")),
    ])
    .await;
    let expected = server.client.activate("t").await.unwrap();
    assert_request(&mut server, "UpdateConfig", &[56, 1, 0x92, 1, 1, b't']).await;
    for (model, model_provider, request) in [
        (None, None, vec![0x52, 4, 0x0a, 2, 0x0a, 0]),
        (
            Some("m".to_owned()),
            Some("p".to_owned()),
            vec![0x52, 4, 0x0a, 2, 0x0a, 0, 0x7a, 1, b'm', 0x82, 1, 1, b'p'],
        ),
    ] {
        let actual = server
            .client
            .prewarm_default_thread(model, model_provider)
            .await
            .unwrap();
        assert_eq!(
            serde_json::to_value(&actual).unwrap(),
            serde_json::to_value(&expected).unwrap()
        );
        assert_eq!(actual.thread.protobuf().as_ref(), native_thread.as_slice());
        // Exactly one default managed selection; no custom settings or Aeon mode.
        assert_request(&mut server, "Prewarm", &request).await;
        assert!(server.requests.try_recv().is_err());
    }

    // Missing authoritative policy must fail rather than supplying local defaults.
    let mut missing_sandbox = vec![0x18, 1];
    for (tag, _, bytes) in fields {
        if tag != 12 {
            prost::encoding::bytes::encode(tag, &bytes.to_vec(), &mut missing_sandbox);
        }
    }
    prost::encoding::bytes::encode(/* tag */ 14, &thread, &mut missing_sandbox);
    let mut server = fixture(response(
        vec![frame(&missing_sandbox)],
        tonic::Status::ok(""),
    ))
    .await;
    assert!(matches!(
        server
            .client
            .prewarm_default_thread(/*model*/ None, /*model_provider*/ None)
            .await,
        Err(Error::InvalidResponse)
    ));
    assert_request(&mut server, "Prewarm", &[0x52, 4, 0x0a, 2, 0x0a, 0]).await;
    assert!(server.requests.try_recv().is_err());
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
#[path = "transcript_tests.rs"]
mod transcript;

#[path = "discovery_tests.rs"]
mod discovery;
