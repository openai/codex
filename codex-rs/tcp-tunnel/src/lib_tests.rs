use std::io::Cursor;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::Result;
use bytes::Buf;
use bytes::Bytes;
use http::Response;
use pretty_assertions::assert_eq;
use rcgen::CertifiedKey;
use rcgen::generate_simple_self_signed;
use rustls::pki_types::PrivatePkcs8KeyDer;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::watch;
use tokio::time::timeout;
use url::Url;

use super::ProxyConnection;
use super::ProxyTarget;
use super::TunnelHeaders;
use super::control::MAX_CONNECT_HEADER_VALUE_BYTES;
use super::control::MAX_CONNECT_HEADERS;
use super::control::MAX_CONNECT_METADATA_BYTES;
use super::control::MAX_TOKEN_BYTES;
use super::control::read_connect_headers;
use super::control_input;
use super::parse_trusted_origins;
use super::serve;
use super::update_auth_tokens;

#[test]
fn proxy_policy_and_target_admission() {
    let origins = parse_trusted_origins("\nhttps://proxy.example.org\n").unwrap();
    assert_eq!(origins, vec!["https://proxy.example.org".to_string()]);
    let proxy = Url::parse("https://proxy.example.org").unwrap();
    for target in ["127.0.0.1:22", "[::1]:2222", "target.example:443"] {
        assert_eq!(
            ProxyTarget::parse(&proxy, &origins, target).unwrap(),
            ProxyTarget {
                host: "proxy.example.org".into(),
                port: 443,
                authority: target.into()
            },
        );
    }
    let ipv6 = "https://[::1]:8443";
    assert_eq!(
        ProxyTarget::parse(
            &Url::parse(ipv6).unwrap(),
            &parse_trusted_origins(ipv6).unwrap(),
            "target.example:22"
        )
        .unwrap(),
        ProxyTarget {
            host: "::1".into(),
            port: 8443,
            authority: "target.example:22".into()
        },
    );
    for origin in [
        "http://proxy.example.org",
        "https://proxy.example.org/path",
        "https://user@proxy.example.org",
        "https://proxy.example.org?query",
        "https://proxy.example.org#fragment",
    ] {
        assert!(parse_trusted_origins(origin).is_err(), "{origin}");
        assert!(
            ProxyTarget::parse(&Url::parse(origin).unwrap(), &origins, "127.0.0.1:22").is_err(),
            "{origin}"
        );
    }
    for origin in [
        "",
        "https://proxy.example.org:443",
        "https://proxy.example.org/",
    ] {
        assert!(parse_trusted_origins(origin).is_err(), "{origin}");
    }
    for (proxy, target) in [
        ("https://evil.example", "127.0.0.1:22"),
        ("https://proxy.example.org.attacker.net", "127.0.0.1:22"),
        ("https://proxy.example.org", "127.0.0.1:0"),
        ("https://proxy.example.org", "target.example"),
        ("https://proxy.example.org", "user@target.example:22"),
        ("https://proxy.example.org", "target.example:65536"),
        ("https://proxy.example.org", "target.example:22/path"),
    ] {
        assert!(
            ProxyTarget::parse(&Url::parse(proxy).unwrap(), &origins, target).is_err(),
            "{proxy} {target}"
        );
    }
}

#[tokio::test]
async fn control_input_acknowledges_bearer_renewal_with_or_without_metadata() -> Result<()> {
    for (input, includes_metadata) in [
        (&b"first-secret\nsecond-secret\n"[..], false),
        (
            &b"[[\"x-test-route\",\"private-value\"]]\nfirst-secret\nsecond-secret\n"[..],
            true,
        ),
    ] {
        let (metadata, mut tokens) = control_input(Cursor::new(input), includes_metadata)?;
        let metadata = metadata.await??;
        if includes_metadata {
            assert_eq!(metadata["x-test-route"], "private-value");
            assert!(metadata["x-test-route"].is_sensitive());
        } else {
            assert!(metadata.is_empty());
        }
        let initial = tokens.recv().await.unwrap()?.unwrap();
        assert_eq!(initial, "Bearer first-secret");
        assert!(initial.is_sensitive());
        let (updates, latest) = watch::channel(initial);
        let (ready, readiness) = oneshot::channel();
        let mut output = Vec::new();
        ready.send(()).unwrap();
        update_auth_tokens(&mut tokens, updates, readiness, &mut output).await?;
        assert_eq!(*latest.borrow(), "Bearer second-secret");
        assert!(latest.borrow().is_sensitive());
        assert_eq!(output, b"AUTH_UPDATED\n");
    }
    Ok(())
}

#[tokio::test]
async fn closed_parent_pipe_finishes_even_before_proxy_readiness() -> Result<()> {
    let (_, mut tokens) = control_input(Cursor::new([]), /*connect_headers_stdin*/ false)?;
    let (updates, _) = watch::channel("Bearer initial".parse()?);
    let (_ready, readiness) = oneshot::channel();
    let mut output = Vec::new();
    timeout(
        Duration::from_secs(1),
        update_auth_tokens(&mut tokens, updates, readiness, &mut output),
    )
    .await??;
    assert!(output.is_empty());
    Ok(())
}

#[tokio::test]
async fn invalid_control_input_stops_credentials_without_exposing_secrets() -> Result<()> {
    let oversized_header = format!(
        "[[\"x-test\",\"private-value{}\"]]\n",
        "x".repeat(MAX_CONNECT_HEADER_VALUE_BYTES)
    );
    let oversized_metadata = vec![b'x'; MAX_CONNECT_METADATA_BYTES + 1];
    let mut oversized_count = serde_json::to_vec(
        &(0..=MAX_CONNECT_HEADERS)
            .map(|index| (format!("x-test-{index}"), "private-value"))
            .collect::<Vec<_>>(),
    )?;
    oversized_count.push(b'\n');
    let invalid_metadata: &[(&str, &[u8])] = &[
        (
            "authorization",
            b"[[\"authorization\",\"private-value\"]]\n",
        ),
        ("forwarding", b"[[\"x-forwarded-for\",\"private-value\"]]\n"),
        ("real IP", b"[[\"x-real-ip\",\"private-value\"]]\n"),
        (
            "duplicate",
            b"[[\"x-test\",\"one\"],[\"X-Test\",\"private-value\"]]\n",
        ),
        (
            "invalid value",
            b"[[\"x-test\",\"private-value\\nmore\"]]\n",
        ),
        ("invalid JSON", b"\"private-value\"\n"),
        ("missing newline", b"[[\"x-test\",\"private-value\"]]"),
        ("value limit", oversized_header.as_bytes()),
        ("metadata limit", &oversized_metadata),
        ("header count", &oversized_count),
    ];
    for &(case, input) in invalid_metadata {
        let mut input = input.to_vec();
        input.extend_from_slice(b"private-secret\n");
        let (metadata, mut tokens) =
            control_input(Cursor::new(input), /*connect_headers_stdin*/ true)?;
        let error = metadata.await?.expect_err(case);
        assert!(!format!("{error:#}").contains("private-"), "{case}");
        assert!(tokens.recv().await.is_none(), "{case}");
    }
    for (case, input) in [
        ("invalid bearer", b" private-secret\0 \n".to_vec()),
        ("empty bearer", b"\n".to_vec()),
        ("bearer limit", vec![b'x'; MAX_TOKEN_BYTES + 1]),
    ] {
        let (metadata, mut tokens) =
            control_input(Cursor::new(input), /*connect_headers_stdin*/ false)?;
        assert!(metadata.await??.is_empty(), "{case}");
        let error = tokens.recv().await.expect(case).expect_err(case);
        assert!(!format!("{error:#}").contains("private-"), "{case}");
        assert!(tokens.recv().await.is_none(), "{case}");
    }
    Ok(())
}

#[tokio::test]
async fn transport_reconnect_keeps_listener_and_does_not_replay_old_tcp_streams() -> Result<()> {
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec!["127.0.0.1".to_string()])?;
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut server_tls = rustls::ServerConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()?
        .with_no_client_auth()
        .with_single_cert(
            vec![cert.der().clone()],
            PrivatePkcs8KeyDer::from(signing_key.serialize_der()).into(),
        )?;
    server_tls.alpn_protocols = vec![b"h3".to_vec()];
    let server_config = quinn::ServerConfig::with_crypto(Arc::new(
        quinn::crypto::rustls::QuicServerConfig::try_from(server_tls)?,
    ));
    let endpoint = quinn::Endpoint::server(server_config, "127.0.0.1:0".parse()?)?;
    let proxy_address = endpoint.local_addr()?;

    let mut roots = rustls::RootCertStore::empty();
    roots.add(cert.der().clone())?;
    let mut client_tls = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .with_root_certificates(roots)
        .with_no_client_auth();
    client_tls.alpn_protocols = vec![b"h3".to_vec()];
    let config = quinn::ClientConfig::new(Arc::new(
        quinn::crypto::rustls::QuicClientConfig::try_from(client_tls)?,
    ));

    let request_count = Arc::new(AtomicUsize::new(0));
    let server_request_count = request_count.clone();
    let (accepted, mut connections) = mpsc::unbounded_channel();
    let (authorizations, mut tokens) = mpsc::unbounded_channel();
    let server = tokio::spawn(async move {
        while let Some(incoming) = endpoint.accept().await {
            let accepted = accepted.clone();
            let request_count = server_request_count.clone();
            let authorizations = authorizations.clone();
            tokio::spawn(async move {
                let quic = incoming.await.unwrap();
                let (shutdown, mut shutdown_requests) = mpsc::channel::<oneshot::Sender<()>>(1);
                accepted.send((quic.clone(), shutdown)).unwrap();
                let mut http = h3::server::builder()
                    .enable_extended_connect(true)
                    .build(h3_quinn::Connection::new(quic))
                    .await
                    .unwrap();
                loop {
                    let request = tokio::select! {
                        Some(finished) = shutdown_requests.recv() => {
                            http.shutdown(/*max_requests*/ 0).await.unwrap();
                            let _ = finished.send(());
                            continue;
                        }
                        request = http.accept() => match request {
                            Ok(Some(request)) => request,
                            _ => break,
                        },
                    };
                    let (request, mut stream) = request.resolve_request().await.unwrap();
                    authorizations
                        .send((
                            request.headers()[http::header::AUTHORIZATION].clone(),
                            request.headers()["x-test-route"].clone(),
                        ))
                        .unwrap();
                    request_count.fetch_add(1, Ordering::SeqCst);
                    tokio::spawn(async move {
                        if stream
                            .send_response(Response::builder().status(200).body(()).unwrap())
                            .await
                            .is_ok()
                            && stream.send_data(Bytes::from_static(b"ready")).await.is_ok()
                        {
                            while let Ok(Some(mut data)) = stream.recv_data().await {
                                let count = data.remaining();
                                if stream.send_data(data.copy_to_bytes(count)).await.is_err() {
                                    break;
                                }
                            }
                        }
                    });
                }
            });
        }
    });

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let target = ProxyTarget {
        host: proxy_address.ip().to_string(),
        port: proxy_address.port(),
        authority: "127.0.0.1:22".to_string(),
    };
    let (updates, auth) = watch::channel("Bearer local-test".parse()?);
    let headers = TunnelHeaders {
        auth,
        connect: read_connect_headers(&mut &b"[[\"x-test-route\",\"custom-route\"]]\n"[..])?,
    };
    let initial = ProxyConnection::connect(&target, &config).await?;
    let client = tokio::spawn(serve(listener, target, config, headers, initial));
    let (first_connection, _) = timeout(Duration::from_secs(5), connections.recv())
        .await?
        .unwrap();
    let mut first = TcpStream::connect(address).await?;
    let mut data = [0; 5];
    timeout(Duration::from_secs(5), first.read_exact(&mut data)).await??;
    assert_eq!(data, *b"ready");
    assert_eq!(
        tokens.recv().await.unwrap(),
        ("Bearer local-test".parse()?, "custom-route".parse()?)
    );

    updates.send_replace("Bearer replacement-test".parse()?);
    let mut refreshed = TcpStream::connect(address).await?;
    timeout(Duration::from_secs(5), refreshed.read_exact(&mut data)).await??;
    assert_eq!(
        tokens.recv().await.unwrap(),
        ("Bearer replacement-test".parse()?, "custom-route".parse()?)
    );

    first_connection.close(/*error_code*/ 0_u8.into(), b"test interruption");
    let old_stream = timeout(Duration::from_secs(5), first.read(&mut data)).await?;
    assert!(matches!(old_stream, Ok(0) | Err(_)));
    let (_, graceful_shutdown) = timeout(Duration::from_secs(10), connections.recv())
        .await?
        .unwrap();
    assert_eq!(request_count.load(Ordering::SeqCst), 2);

    let connect_ready_listener = move || async move {
        loop {
            let mut candidate = TcpStream::connect(address).await?;
            let mut data = [0; 5];
            if timeout(Duration::from_millis(250), candidate.read_exact(&mut data))
                .await
                .is_ok_and(|result| result.is_ok())
            {
                return Ok::<_, anyhow::Error>((data, candidate));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    };
    let (recovered, mut persistent) =
        timeout(Duration::from_secs(5), connect_ready_listener()).await??;
    assert_eq!(recovered, *b"ready");
    assert_eq!(
        tokens.recv().await.unwrap(),
        ("Bearer replacement-test".parse()?, "custom-route".parse()?)
    );
    assert_eq!(request_count.load(Ordering::SeqCst), 3);

    let (sent, goaway_sent) = oneshot::channel();
    graceful_shutdown.send(sent).await?;
    goaway_sent.await?;
    persistent.write_all(b"still").await?;
    timeout(Duration::from_secs(5), persistent.read_exact(&mut data)).await??;
    assert_eq!(data, *b"still");

    let new_request = tokio::spawn(connect_ready_listener());
    timeout(Duration::from_secs(10), connections.recv())
        .await?
        .unwrap();
    let (ready, _) = timeout(Duration::from_secs(5), new_request).await???;
    assert_eq!(ready, *b"ready");
    persistent.write_all(b"alive").await?;
    timeout(Duration::from_secs(5), persistent.read_exact(&mut data)).await??;
    assert_eq!(data, *b"alive");
    client.abort();
    server.abort();
    Ok(())
}
