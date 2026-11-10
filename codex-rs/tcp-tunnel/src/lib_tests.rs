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
use super::diagnostics::Diagnostics;
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
        let (metadata, mut tokens) = control_input(
            Cursor::new(input),
            includes_metadata,
            "https://proxy.example",
        )?;
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
async fn portals_jwt_listener_requires_v2_at_startup_and_on_renewal() -> Result<()> {
    // Inert parser fixtures with a placeholder signature. No signing key or live credential.
    const HEADER: &str = "eyJ0eXAiOiJjY2EtdHVubmVsK2p3dCJ9";
    let v1 = format!("{HEADER}.eyJ2ZXIiOjF9.fixture");
    let v2 = format!("{HEADER}.eyJ2ZXIiOjJ9.fixture");
    let origin = super::ssh::PUBLIC_GATEWAY;
    for invalid in [
        &v1[..],
        "opaque",
        "malformed.parts",
        &format!("{HEADER}.e30.fixture"),
    ] {
        let (_, mut tokens) = control_input(
            Cursor::new(invalid.to_owned()),
            /*connect_headers_stdin*/ false,
            origin,
        )?;
        let error = tokens.recv().await.unwrap().unwrap_err();
        assert_eq!(
            error.to_string(),
            "Portals requires an ERS-issued v2 tunnel JWT"
        );
    }

    let (_, mut tokens) = control_input(
        Cursor::new(format!("{v2}\n{v2}\n{v1}\n")),
        /*connect_headers_stdin*/ false,
        origin,
    )?;
    let initial = tokens.recv().await.unwrap()?.unwrap();
    assert_eq!(initial.to_str()?, format!("Bearer {v2}"));
    let (updates, latest) = watch::channel(initial.clone());
    let (ready, readiness) = oneshot::channel();
    ready.send(()).unwrap();
    let mut output = Vec::new();
    let error = update_auth_tokens(&mut tokens, updates, readiness, &mut output)
        .await
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "Portals requires an ERS-issued v2 tunnel JWT"
    );
    assert_eq!(output, b"AUTH_UPDATED\n");
    assert_eq!(*latest.borrow(), initial);
    Ok(())
}

#[tokio::test]
async fn other_bearer_modes_reject_ers_v1_and_accept_unrelated_jwts() -> Result<()> {
    let unrelated = "eyJ0eXAiOiJKV1QifQ.eyJ2ZXIiOjF9.fixture";
    let ers_v1 = "eyJ0eXAiOiJjY2EtdHVubmVsK2p3dCJ9.eyJ2ZXIiOjF9.fixture";
    for (origin, metadata) in [
        (
            super::ssh::PUBLIC_GATEWAY,
            r#"[["x-portals-auth-mode","oauth"]]"#,
        ),
        ("https://proxy.example", "[]"),
    ] {
        let (headers, mut tokens) = control_input(
            Cursor::new(format!("{metadata}\n{unrelated}\n{ers_v1}\n")),
            /*connect_headers_stdin*/ true,
            origin,
        )?;
        headers.await??;
        let initial = tokens.recv().await.unwrap()?.unwrap();
        assert_eq!(initial.to_str()?, format!("Bearer {unrelated}"));
        let error = tokens.recv().await.unwrap().unwrap_err();
        assert_eq!(
            error.to_string(),
            "Portals requires an ERS-issued v2 tunnel JWT"
        );
    }
    Ok(())
}

#[tokio::test]
async fn closed_parent_pipe_finishes_even_before_proxy_readiness() -> Result<()> {
    let (_, mut tokens) = control_input(
        Cursor::new([]),
        /*connect_headers_stdin*/ false,
        "https://proxy.example",
    )?;
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
        let (metadata, mut tokens) = control_input(
            Cursor::new(input),
            /*connect_headers_stdin*/ true,
            "https://proxy.example",
        )?;
        let error = metadata.await?.expect_err(case);
        assert!(!format!("{error:#}").contains("private-"), "{case}");
        assert!(tokens.recv().await.is_none(), "{case}");
    }
    for (case, input) in [
        ("invalid bearer", b" private-secret\0 \n".to_vec()),
        ("empty bearer", b"\n".to_vec()),
        ("bearer limit", vec![b'x'; MAX_TOKEN_BYTES + 1]),
    ] {
        let (metadata, mut tokens) = control_input(
            Cursor::new(input),
            /*connect_headers_stdin*/ false,
            "https://proxy.example",
        )?;
        assert!(metadata.await??.is_empty(), "{case}");
        let error = tokens.recv().await.expect(case).expect_err(case);
        assert!(!format!("{error:#}").contains("private-"), "{case}");
        assert!(tokens.recv().await.is_none(), "{case}");
    }
    Ok(())
}

fn local_proxy() -> Result<(quinn::Endpoint, quinn::ClientConfig)> {
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

    Ok((endpoint, config))
}

#[tokio::test]
async fn stdio_propagates_half_close_and_exits_on_gateway_eof_or_rejection() -> Result<()> {
    let (endpoint, config) = local_proxy()?;
    let address = endpoint.local_addr()?;
    let target = ProxyTarget {
        host: address.ip().to_string(),
        port: address.port(),
        authority: address.to_string(),
    };
    let (done, keep_alive) = oneshot::channel();
    let server = tokio::spawn(async move {
        let quic = endpoint.accept().await.unwrap().await?;
        let mut http = h3::server::Connection::new(h3_quinn::Connection::new(quic)).await?;
        for (index, status) in [200, 200, 401].into_iter().enumerate() {
            let (_, mut stream) = http.accept().await?.unwrap().resolve_request().await?;
            stream
                .send_response(Response::builder().status(status).body(())?)
                .await?;
            if status == 200 {
                stream.send_data(Bytes::from_static(b"\0\xff")).await?;
            }
            // First request must forward the final bytes and FIN before reading the reply.
            // The next request sees remote EOF with its own input still open.
            if index == 0 {
                let mut data = stream.recv_data().await?.unwrap();
                let size = data.remaining();
                assert_eq!(data.copy_to_bytes(size), Bytes::from_static(b"request"));
                assert!(stream.recv_data().await?.is_none());
                stream.send_data(Bytes::from_static(b"after FIN")).await?;
            }
            stream.finish().await?;
        }
        let _ = keep_alive.await;
        anyhow::Ok(())
    });
    let ProxyConnection {
        endpoint: _endpoint,
        quic: _quic,
        mut connection,
        mut sender,
    } = ProxyConnection::connect(&target, &config).await?;
    let driver = tokio::spawn(async move { connection.wait_idle().await });
    for index in 0..3 {
        let request = target.request(http::HeaderMap::new(), "Bearer fixture".parse()?)?;
        let (mut input, reader) = tokio::io::duplex(16);
        if index == 0 {
            input.write_all(b"request").await?;
            input.shutdown().await?;
        }
        let mut output = Vec::new();
        let result = timeout(
            Duration::from_secs(3),
            super::bridge(
                reader,
                &mut output,
                &mut sender,
                request,
                super::BridgeMode::ProxyCommand,
            ),
        )
        .await?;
        match index {
            0 => {
                result?;
                assert_eq!(output, b"\0\xffafter FIN");
            }
            1 => {
                result?;
                assert_eq!(output, b"\0\xff");
            }
            _ => {
                assert_eq!(
                    result.unwrap_err().to_string(),
                    "MASQUE CONNECT rejected with status 401"
                );
                assert!(output.is_empty());
            }
        }
    }
    done.send(()).unwrap();
    server.await??;
    driver.abort();
    Ok(())
}

#[tokio::test]
async fn connect_rejects_qpack_expansion_and_keeps_connection_usable() -> Result<()> {
    timeout(Duration::from_secs(15), async {
        let (endpoint, config) = local_proxy()?;
        let address = endpoint.local_addr()?;

        let (done_tx, done_rx) = oneshot::channel();
        let server = tokio::spawn(async move {
            let quic = endpoint.accept().await.unwrap().await.unwrap();
            let mut control = quic.open_uni().await.unwrap();
            // HTTP/3 control stream with empty SETTINGS.
            control.write_all(&[0x00, 0x04, 0x00]).await.unwrap();
            for fields in [648_u32, 649, 32_768, 1] {
                let (mut send, _recv) = quic.accept_bi().await.unwrap();
                // Bypass the server library's enforcement of peer SETTINGS.
                // HEADERS with a four-byte QUIC varint length, QPACK prefix,
                // :status=200, then one-byte static references to a 101-byte field.
                let mut response = vec![0x01];
                response.extend_from_slice(&(0x8000_0000 | (fields + 3)).to_be_bytes());
                response.extend_from_slice(&[0x00, 0x00, 0xd9]);
                response.resize(response.len() + fields as usize, 0xfa);
                send.write_all(&response).await.unwrap();
                send.finish().unwrap();
            }
            let _ = done_rx.await;
        });
        let target = ProxyTarget {
            host: address.ip().to_string(),
            port: address.port(),
            authority: "127.0.0.1:22".to_string(),
        };
        let ProxyConnection {
            endpoint: _endpoint,
            quic: _quic,
            mut connection,
            mut sender,
        } = ProxyConnection::connect(&target, &config).await?;
        let driver = tokio::spawn(async move { connection.wait_idle().await });
        for oversized in [false, true, true, false] {
            let request = http::Request::builder()
                .method(http::Method::CONNECT)
                .uri(http::Uri::builder().authority("127.0.0.1:22").build()?)
                .body(())?;
            let mut stream = sender.send_request(request).await?;
            let result = stream.recv_response().await;
            if oversized {
                let error = result.expect_err("oversized response must be rejected");
                assert!(matches!(
                    error,
                    h3::error::StreamError::HeaderTooBig {
                        actual_size,
                        max_size: 65_536,
                        ..
                    } if actual_size > 65_536
                ));
            } else {
                assert_eq!(result?.status(), http::StatusCode::OK);
            }
        }
        done_tx.send(()).unwrap();
        server.await?;
        driver.abort();
        Ok(())
    })
    .await?
}

#[tokio::test]
async fn transport_reconnect_keeps_listener_and_does_not_replay_old_tcp_streams() -> Result<()> {
    let (endpoint, config) = local_proxy()?;
    let proxy_address = endpoint.local_addr()?;

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
                    if request.headers()[http::header::AUTHORIZATION] == "Bearer rejected-test" {
                        stream
                            .send_response(Response::builder().status(403).body(()).unwrap())
                            .await
                            .unwrap();
                        stream.finish().await.unwrap();
                        continue;
                    }
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
    let client = tokio::spawn(serve(
        listener,
        target,
        config,
        headers,
        initial,
        Diagnostics::Json,
    ));
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

    updates.send_replace("Bearer rejected-test".parse()?);
    let mut rejected = TcpStream::connect(address).await?;
    assert_eq!(
        timeout(Duration::from_secs(5), rejected.read(&mut data)).await??,
        0
    );
    assert_eq!(
        tokens.recv().await.unwrap(),
        ("Bearer rejected-test".parse()?, "custom-route".parse()?)
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

#[test]
fn diagnostics_flag_is_optional_and_advertised() {
    #[derive(clap::Parser)]
    struct Cli {
        #[command(flatten)]
        args: super::Args,
    }
    use clap::CommandFactory;
    use clap::Parser;
    let args = [
        "tcp-tunnel",
        "--proxy-url",
        "https://proxy.example",
        "--proxy-origins-file",
        "origins",
        "--target",
        "target:22",
        "--auth-token-stdin",
    ];
    assert!(!Cli::try_parse_from(args).unwrap().args.diagnostics_json);
    assert!(
        Cli::try_parse_from(args.into_iter().chain(["--diagnostics-json"]))
            .unwrap()
            .args
            .diagnostics_json
    );
    assert!(
        Cli::command()
            .render_help()
            .to_string()
            .contains("--diagnostics-json")
    );
}
