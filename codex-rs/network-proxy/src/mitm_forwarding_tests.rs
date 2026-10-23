//! Exercise authorized MITM forwarding over TLS, including transport-owned request bodies.

use super::*;
use crate::aws_sigv4::AwsSigV4Config;
use crate::aws_sigv4::ProxySecretSource;
use crate::aws_sigv4::tests::guest_authorization;
use crate::config::NetworkProxyConfig;
use crate::mitm_hook::MitmHookActionsConfig;
use crate::mitm_hook::MitmHookConfig;
use crate::mitm_hook::MitmHookMatchConfig;
use crate::runtime::network_proxy_state_for_policy;
use pretty_assertions::assert_eq;
use rama_tls_rustls::dep::pki_types::PrivateKeyDer;
use rama_tls_rustls::dep::pki_types::pem::PemObject;
use rama_tls_rustls::dep::rcgen::CertificateParams;
use rama_tls_rustls::dep::rcgen::KeyPair;
use rama_tls_rustls::dep::tokio_rustls::TlsAcceptor;
use sha2::Digest as _;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;
use tempfile::tempdir;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tokio::time::timeout;

const FORWARD_TIMEOUT: Duration = Duration::from_secs(15);
const RESPONSE: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";

async fn tls_origin(
    dir: &Path,
    service: &str,
    alpn: &[u8],
) -> (MitmRequestContext, TcpListener, TlsAcceptor) {
    tls_origin_with_hook(dir, service, alpn, |_, _| {}).await
}

async fn tls_origin_with_hook(
    dir: &Path,
    service: &str,
    alpn: &[u8],
    configure: impl FnOnce(&mut MitmHookConfig, u16),
) -> (MitmRequestContext, TcpListener, TlsAcceptor) {
    ensure_rustls_crypto_provider();
    let key = KeyPair::generate().expect("generate origin key");
    let cert = CertificateParams::new(vec!["localhost".to_string()])
        .expect("origin certificate parameters")
        .self_signed(&key)
        .expect("origin certificate");
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(cert.der().clone())
        .expect("trust origin certificate");
    let key =
        PrivateKeyDer::from_pem_slice(key.serialize_pem().as_bytes()).expect("parse origin key");
    let mut server_config =
        rustls::ServerConfig::builder_with_protocol_versions(rustls::ALL_VERSIONS)
            .with_no_client_auth()
            .with_single_cert(vec![cert.der().clone()], key)
            .expect("origin TLS configuration");
    server_config.alpn_protocols = vec![alpn.to_vec()];
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind TLS origin");
    let port = listener.local_addr().expect("origin address").port();

    let access_key = dir.join("access-key");
    let secret_key = dir.join("secret-key");
    std::fs::write(&access_key, "AKIDEXAMPLE").expect("write access key");
    std::fs::write(&secret_key, "test-secret-key").expect("write secret key");
    let mut config = NetworkProxyConfig {
        mitm: true,
        mode: NetworkMode::Full,
        allow_local_binding: Some(true),
        mitm_hooks: vec![MitmHookConfig {
            host: "localhost".to_string(),
            matcher: MitmHookMatchConfig {
                schemes: vec!["https".to_string()],
                ports: vec![port],
                methods: vec!["GET".to_string(), "POST".to_string()],
                path_prefixes: vec!["/".to_string()],
                ..Default::default()
            },
            actions: MitmHookActionsConfig {
                strip_request_headers: vec!["x-amz-trailer".to_string()],
                aws_sigv4: Some(AwsSigV4Config {
                    placeholder_access_key_id: "MASKEDACCESSKEY".to_string(),
                    region: "us-east-1".to_string(),
                    service: service.to_string(),
                    access_key_id: ProxySecretSource::File(access_key.display().to_string()),
                    secret_access_key: ProxySecretSource::File(secret_key.display().to_string()),
                    session_token: None,
                    port,
                    max_body_bytes: 1024,
                }),
                ..Default::default()
            },
        }],
        ..Default::default()
    };
    configure(&mut config.mitm_hooks[0], port);
    config.set_credential_broker_enabled(/*enabled*/ true);
    config.set_allowed_domains(vec!["localhost".to_string()]);
    let app_state = Arc::new(network_proxy_state_for_policy(config));
    let roots = Arc::new(roots);
    let upstream =
        UpstreamClient::direct_with_tls_root_store(Arc::clone(&app_state), Arc::clone(&roots));
    let mitm = Arc::new(MitmState {
        ca: ManagedMitmCa::for_test(dir).expect("create test MITM CA"),
        allow_upstream_proxy: false,
        upstream_tls_root_store: roots,
        inspect: false,
        max_body_bytes: MITM_MAX_BODY_BYTES,
    });
    let context = MitmRequestContext {
        policy: MitmPolicyContext {
            target_host: "localhost".to_string(),
            target_port: port,
            scheme: Scheme::HTTPS,
            mode: NetworkMode::Full,
            app_state,
        },
        mitm,
        upstream,
    };
    (
        context,
        listener,
        TlsAcceptor::from(Arc::new(server_config)),
    )
}

struct ReceivedRequest {
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

async fn read_request(stream: &mut (impl AsyncRead + Unpin)) -> ReceivedRequest {
    let mut head = Vec::new();
    while !head.ends_with(b"\r\n\r\n") {
        assert!(head.len() < 16 * 1024, "bounded origin request headers");
        head.push(stream.read_u8().await.expect("read request headers"));
    }
    let head = String::from_utf8(head).expect("ASCII request headers");
    let mut lines = head.split("\r\n");
    let path = lines
        .next()
        .expect("request line")
        .split_whitespace()
        .nth(1)
        .expect("request target")
        .to_string();
    let headers = lines
        .filter(|line| !line.is_empty())
        .map(|line| {
            let (name, value) = line.split_once(':').expect("header separator");
            (name.to_ascii_lowercase(), value.trim().to_string())
        })
        .collect::<HashMap<_, _>>();
    assert!(!headers.contains_key("transfer-encoding"));
    let length = headers
        .get("content-length")
        .map_or(0, |length| length.parse::<usize>().expect("content length"));
    assert!(length <= 1024, "bounded origin request body");
    let mut body = vec![0; length];
    stream
        .read_exact(&mut body)
        .await
        .expect("drain request body");
    ReceivedRequest {
        path,
        headers,
        body,
    }
}

fn request(
    context: &MitmRequestContext,
    service: &str,
    method: &str,
    path: &str,
    body: &[u8],
) -> Request {
    Request::builder()
        .method(method)
        .uri(path)
        .header(HOST, format!("localhost:{}", context.policy.target_port))
        .header(
            "authorization",
            guest_authorization(service, "host;x-amz-content-sha256;x-amz-date"),
        )
        .header("x-amz-date", "20150830T123600Z")
        .header("x-amz-content-sha256", "guest-hash")
        .body(Body::from(body.to_vec()))
        .expect("inner MITM request")
}

#[tokio::test]
async fn forwards_aoss_payload_hashes_and_signatures_over_tls() {
    let dir = tempdir().expect("TLS fixture directory");
    let (context, listener, acceptor) = tls_origin(dir.path(), "aoss", b"http/1.1").await;
    let builder = crate::NetworkProxy::builder()
        .state(Arc::clone(&context.policy.app_state))
        .managed_proxy_routing(crate::ManagedProxyRouting::DedicatedListeners);
    let proxy = crate::certs::TEST_MITM_CA
        .scope(Arc::clone(&context.mitm.ca), builder.build())
        .await
        .expect("local proxy");
    let mut env = HashMap::from([("ALIAS".to_string(), "prefix:test-secret-key".to_string())]);
    proxy.apply_to_env_for_snapshot(&mut env);
    assert_eq!(env["AWS_ACCESS_KEY_ID"], "MASKEDACCESSKEY");
    assert_eq!(env["AWS_CA_BUNDLE"], env["SSL_CERT_FILE"]);
    assert_eq!(Path::new(&env["AWS_CA_BUNDLE"]).parent(), Some(dir.path()));
    assert!(crate::brokered_credential_env_keys(&env).any(|key| key == "AWS_ACCESS_KEY_ID"));
    assert_eq!(
        env["ALIAS"],
        format!("prefix:{}", env["AWS_SECRET_ACCESS_KEY"])
    );
    let mut snapshot = "export COPY=test-secret-key".to_string();
    assert!(proxy.virtualize_brokered_text(&mut snapshot, &env));
    assert_eq!(
        snapshot,
        format!("export COPY={}", env["AWS_SECRET_ACCESS_KEY"])
    );
    let remote = proxy
        .prepare_for_remote_environment(HashMap::new(), "remote")
        .expect("remote environment");
    assert!(!remote.env.contains_key("AWS_ACCESS_KEY_ID"));
    assert!(!remote.env.contains_key("AWS_CA_BUNDLE"));
    let cases: [(&str, &[u8]); 4] = [
        ("/unsigned", b""),
        ("/other-key", b""),
        ("/get", b""),
        ("/post", b"{\"message\":\"hello\"}"),
    ];
    let other_key =
        guest_authorization("aoss", "host;x-amz-date").replace("MASKEDACCESSKEY", "OTHERKEY");
    let expected_other = other_key.clone();
    let server = tokio::spawn(async move {
        for (path, body) in cases {
            let (stream, _) = listener.accept().await.expect("accept origin connection");
            let mut stream = acceptor.accept(stream).await.expect("origin TLS handshake");
            let received = read_request(&mut stream).await;
            assert_eq!(received.path, path);
            assert_eq!(received.body, body);
            match path {
                "/unsigned" => assert!(!received.headers.contains_key("authorization")),
                "/other-key" => assert_eq!(received.headers["authorization"], expected_other),
                _ => {
                    assert_eq!(
                        received.headers["x-amz-content-sha256"],
                        format!("{:x}", sha2::Sha256::digest(body))
                    );
                    assert!(received.headers["authorization"].contains(
                        "Credential=AKIDEXAMPLE/20150830/us-east-1/aoss/aws4_request, SignedHeaders=host;x-amz-content-sha256;x-amz-date,"
                    ));
                }
            }
            stream.write_all(RESPONSE).await.expect("origin response");
        }
    });

    for (path, body) in cases {
        let method = if body.is_empty() { "GET" } else { "POST" };
        let mut req = request(&context, "aoss", method, path, body);
        if path == "/unsigned" {
            req.headers_mut().remove("authorization");
        } else if path == "/other-key" {
            req.headers_mut()
                .insert("authorization", HeaderValue::from_str(&other_key).unwrap());
        }
        let response = timeout(FORWARD_TIMEOUT, forward_request(req, &context))
            .await
            .expect("MITM forwarding deadline")
            .expect("MITM forwarding");
        assert_eq!(response.status(), StatusCode::OK);
    }
    timeout(FORWARD_TIMEOUT, server)
        .await
        .expect("origin completion deadline")
        .expect("origin assertions");
}

#[tokio::test]
async fn signs_only_supported_http1_requests_for_an_http2_origin() {
    use rama_core::ServiceInput;
    use rama_http::body::util::BodyExt;

    let dir = tempdir().expect("TLS fixture directory");
    let (context, listener, acceptor) = tls_origin(dir.path(), "s3", b"h2").await;
    let context = Arc::new(context);
    let (observed, mut received) = mpsc::unbounded_channel();
    let origin = tokio::spawn(async move {
        loop {
            let (socket, _) = listener.accept().await.expect("accept origin");
            let stream = acceptor.accept(socket).await.expect("origin TLS");
            let observed = observed.clone();
            let service = service_fn(move |request: Request| {
                let observed = observed.clone();
                async move {
                    let (parts, body) = request.into_parts();
                    let body = body.collect().await.expect("origin body").to_bytes();
                    observed.send((parts, body)).expect("report origin request");
                    Ok::<_, std::convert::Infallible>(Response::new(Body::empty()))
                }
            });
            HttpServer::h2(Executor::default())
                .service(service)
                .serve(ServiceInput::new(stream))
                .await
                .expect("serve HTTP/2");
        }
    });

    let auth = guest_authorization("s3", "host;x-amz-date");
    for (extra_headers, status) in [
        ("Content-Length: 1\r\nTransfer-Encoding: chunked\r\n", 403),
        ("Transfer-Encoding: gzip, chunked\r\n", 403),
        (
            "Transfer-Encoding: chunked\r\nConnection: Authorization\r\n",
            403,
        ),
        (
            "Transfer-Encoding: chunked\r\nX-Amz-Trailer: checksum\r\n",
            403,
        ),
        (
            "Transfer-Encoding: chunked\r\nContent-Encoding: aws-chunked\r\nConnection: content-encoding\r\n",
            403,
        ),
        // Rama discards Content-Length when it appears after Transfer-Encoding.
        // Verify that the retained chunk framing and the signed payload agree.
        (
            "Transfer-Encoding: chunked\r\nContent-Length: 1\r\nKeep-Alive: timeout=5\r\nSec-WebSocket-Key: test\r\n",
            200,
        ),
    ] {
        let (mut guest, io) = tokio::io::duplex(4096);
        let handler = service_fn({
            let context = Arc::clone(&context);
            move |request| handle_mitm_request(request, Arc::clone(&context))
        });
        let guest_server = tokio::spawn(async move {
            HttpServer::http1()
                .service(handler)
                .serve(ServiceInput::new(io))
                .await
                .expect("serve guest");
        });
        let request = format!(
            "POST /key HTTP/1.1\r\nHost: localhost:{}\r\nAuthorization: {auth}\r\nX-Amz-Date: 20150830T123600Z\r\n{extra_headers}Connection: close\r\n\r\n3\r\nabc\r\n0\r\n\r\n",
            context.policy.target_port
        );
        guest
            .write_all(request.as_bytes())
            .await
            .expect("send guest request");
        let mut response = Vec::new();
        timeout(FORWARD_TIMEOUT, guest.read_to_end(&mut response))
            .await
            .expect("response deadline")
            .expect("read guest response");
        guest_server.await.expect("guest server");
        assert!(
            response.starts_with(format!("HTTP/1.1 {status} ").as_bytes()),
            "{extra_headers}: {}",
            String::from_utf8_lossy(&response)
        );
    }
    let (parts, body) = received.try_recv().expect("one allowed request");
    assert!(received.try_recv().is_err());
    assert_eq!(parts.version, rama_http::Version::HTTP_2);
    assert_eq!(body.as_ref(), b"abc");
    assert_eq!(
        parts.headers["x-amz-content-sha256"],
        format!("{:x}", sha2::Sha256::digest(&body))
    );
    let authorization = parts.headers["authorization"].to_str().expect("signature");
    let (fields, _) = authorization
        .split_once("SignedHeaders=")
        .expect("signed fields")
        .1
        .split_once(',')
        .expect("signature suffix");
    for name in fields.split(';') {
        // HTTP/2 transports Host in :authority.
        assert!(name == "host" || parts.headers.contains_key(name), "{name}");
    }
    origin.abort();
}

#[tokio::test]
async fn drained_signed_bodies_release_capacity_before_response_headers() {
    let dir = tempdir().expect("TLS fixture directory");
    let (context, listener, acceptor) = tls_origin(dir.path(), "sqs", b"http/1.1").await;
    let (arrived, mut arrivals) = mpsc::channel(9);
    let (release, released) = watch::channel(false);
    let server = tokio::spawn(async move {
        let mut requests = JoinSet::new();
        for _ in 0..9 {
            let (stream, _) = listener.accept().await.expect("accept origin connection");
            let acceptor = acceptor.clone();
            let arrived = arrived.clone();
            let mut released = released.clone();
            requests.spawn(async move {
                let mut stream = acceptor.accept(stream).await.expect("origin TLS handshake");
                let received = read_request(&mut stream).await;
                let wait = received.path.starts_with("/wait/");
                let expected_body = if wait {
                    b"Action=ReceiveMessage".as_slice()
                } else {
                    b"Action=SendMessage&MessageBody=hello".as_slice()
                };
                assert_eq!(received.body, expected_body);
                assert!(received.headers["authorization"].contains("/sqs/aws4_request"));
                arrived
                    .send(received.path)
                    .await
                    .expect("report drained body");
                if wait {
                    released
                        .wait_for(|released| *released)
                        .await
                        .expect("release delayed headers");
                }
                stream.write_all(RESPONSE).await.expect("origin response");
            });
        }
        while let Some(result) = requests.join_next().await {
            result.expect("origin request assertions");
        }
    });

    let mut pending = JoinSet::new();
    for index in 0..8 {
        let context = context.clone();
        pending.spawn(async move {
            let req = request(
                &context,
                "sqs",
                "POST",
                &format!("/wait/{index}"),
                b"Action=ReceiveMessage",
            );
            forward_request(req, &context).await
        });
    }
    for _ in 0..8 {
        let path = timeout(FORWARD_TIMEOUT, arrivals.recv())
            .await
            .expect("eight request bodies must drain")
            .expect("origin remains connected");
        assert!(path.starts_with("/wait/"));
    }

    // All eight responses still lack headers; only their request bodies have completed.
    let ninth = timeout(
        FORWARD_TIMEOUT,
        forward_request(
            request(
                &context,
                "sqs",
                "POST",
                "/send",
                b"Action=SendMessage&MessageBody=hello",
            ),
            &context,
        ),
    )
    .await;
    release.send(true).expect("release long polls");
    while let Some(result) = timeout(FORWARD_TIMEOUT, pending.join_next())
        .await
        .expect("long poll completion deadline")
    {
        let response = result.expect("forward task").expect("long poll response");
        assert_eq!(response.status(), StatusCode::OK);
    }
    // Aborting the origin also aborts its request tasks if the ninth request never arrived.
    server.abort();
    let response = ninth
        .expect("ninth forwarding deadline")
        .expect("drained bodies must release buffer slots before response headers");
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn forwards_substituted_body_before_signing() {
    let marker = b"codex-secret-0123456789abcdef";
    let dir = tempdir().expect("TLS fixture directory");
    let secret = dir.path().join("body-secret");
    std::fs::write(&secret, "expanded-payload").unwrap();
    let configure = |hook: &mut MitmHookConfig, port| {
        hook.matcher.path_prefixes = vec!["/allowed/".into()];
        hook.actions.strip_request_headers.push("digest".into());
        hook.actions.body_secret_substitution = Some(crate::BodySecretSubstitutionConfig {
            replacements: vec![crate::BodySecretReplacement {
                placeholder: String::from_utf8(marker.to_vec()).unwrap(),
                secret: ProxySecretSource::File(secret.display().to_string()),
            }],
            port,
            max_body_bytes: 64,
        });
    };
    let (context, listener, acceptor) =
        tls_origin_with_hook(dir.path(), "s3", b"http/1.1", configure).await;
    let context = Arc::new(context);
    let signed_headers = "content-length;cookie;host;x-amz-date";
    let make_request = |path: &str| {
        let mut req = request(&context, "s3", "POST", path, marker);
        *req.version_mut() = rama_http::Version::HTTP_2;
        let headers = req.headers_mut();
        headers.remove("x-amz-content-sha256");
        headers.insert(
            "authorization",
            HeaderValue::from_str(&guest_authorization("s3", signed_headers)).unwrap(),
        );
        headers.insert("content-length", HeaderValue::from(marker.len()));
        headers.append("cookie", HeaderValue::from_static("a=1"));
        headers.append("cookie", HeaderValue::from_static("b=2"));
        req
    };
    let denied = handle_mitm_request(make_request("/denied"), context.clone())
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    let mut invalid = make_request("/allowed/item");
    invalid
        .headers_mut()
        .insert("digest", HeaderValue::from_static("sha-256=guest"));
    let denied = handle_mitm_request(invalid, context.clone()).await.unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    let origin = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut stream = acceptor.accept(socket).await.unwrap();
        let wire = read_request(&mut stream).await;
        stream.write_all(RESPONSE).await.unwrap();
        wire
    });
    let req = make_request("/allowed/item");
    let response = timeout(FORWARD_TIMEOUT, handle_mitm_request(req, context))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let wire = origin.await.unwrap();
    assert_eq!(wire.body.as_slice(), b"expanded-payload");
    assert_eq!(wire.path, "/allowed/item");
    let headers = wire.headers;
    assert_eq!(headers["cookie"], "a=1; b=2");
    let expected_hash = format!("{:x}", sha2::Sha256::digest(&wire.body));
    assert_eq!(headers["x-amz-content-sha256"], expected_hash);
    // Rebuild the canonical request from the received HTTP/1.1 headers, without
    // the signer's request preparation or the SDK's header canonicalization.
    let canonical_headers = signed_headers
        .split(';')
        .map(|name| format!("{name}:{}\n", headers[name]))
        .collect::<String>();
    let canonical_request =
        format!("POST\n/allowed/item\n\n{canonical_headers}\n{signed_headers}\n{expected_hash}");
    let timestamp = &headers["x-amz-date"];
    let signing_time = chrono::NaiveDateTime::parse_from_str(timestamp, "%Y%m%dT%H%M%SZ")
        .unwrap()
        .and_utc();
    let scope = format!(
        "{}/us-east-1/s3/aws4_request",
        signing_time.format("%Y%m%d")
    );
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{timestamp}\n{scope}\n{:x}",
        sha2::Sha256::digest(canonical_request.as_bytes())
    );
    let signing_key = aws_sigv4::sign::v4::generate_signing_key(
        "test-secret-key",
        signing_time.into(),
        "us-east-1",
        "s3",
    );
    let signature =
        aws_sigv4::sign::v4::calculate_signature(signing_key, string_to_sign.as_bytes());
    assert_eq!(
        headers["authorization"],
        format!(
            "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/{scope}, SignedHeaders={signed_headers}, Signature={signature}"
        )
    );
}
