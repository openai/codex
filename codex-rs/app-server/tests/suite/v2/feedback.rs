use std::time::Duration;

use anyhow::Result;
use app_test_support::TestAppServer;
use codex_app_server_protocol::RequestId;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::net::TcpListener;
use tokio::time::timeout;

#[tokio::test]
async fn feedback_upload_limits_concurrency_and_releases_failed_uploads() -> Result<()> {
    let proxy = TcpListener::bind("127.0.0.1:0").await?;
    let proxy_uri = format!("http://{}", proxy.local_addr()?);
    let mut app_server = TestAppServer::builder()
        .with_env_overrides(&[
            ("HTTPS_PROXY", Some(proxy_uri.as_str())),
            ("https_proxy", Some(proxy_uri.as_str())),
            ("NO_PROXY", Some("")),
            ("no_proxy", Some("")),
        ])
        .build_initialized()
        .await?;

    let mut pending = Vec::new();
    for _ in 0..3 {
        let request_id = app_server
            .send_raw_request(
                "feedback/upload",
                Some(json!({ "classification": "bug", "includeLogs": false })),
            )
            .await?;
        let (stream, _) = timeout(Duration::from_secs(/*secs*/ 15), proxy.accept()).await??;
        let mut stream = BufReader::new(stream);
        let mut request = String::new();
        timeout(
            Duration::from_secs(/*secs*/ 15),
            stream.read_line(&mut request),
        )
        .await??;
        assert!(request.starts_with("CONNECT "));
        pending.push((request_id, stream.into_inner()));
    }

    let excess_id = app_server
        .send_raw_request(
            "feedback/upload",
            Some(json!({ "classification": "bug", "includeLogs": false })),
        )
        .await?;
    let error = timeout(
        Duration::from_secs(/*secs*/ 15),
        app_server.read_stream_until_error_message(RequestId::Integer(excess_id)),
    )
    .await??;

    assert_eq!(error.error.code, -32001);

    let unavailable =
        b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
    for (request_id, mut stream) in pending {
        stream.write_all(unavailable).await?;
        stream.shutdown().await?;
        let error = timeout(
            Duration::from_secs(/*secs*/ 15),
            app_server.read_stream_until_error_message(RequestId::Integer(request_id)),
        )
        .await??;
        assert_eq!(error.error.code, -32603);
        assert!(error.error.message.contains("failed to upload feedback"));
    }

    let request_id = app_server
        .send_raw_request(
            "feedback/upload",
            Some(json!({ "classification": "bug", "includeLogs": false })),
        )
        .await?;
    let (mut stream, _) = timeout(Duration::from_secs(/*secs*/ 15), proxy.accept()).await??;
    stream.write_all(unavailable).await?;
    stream.shutdown().await?;
    let error = timeout(
        Duration::from_secs(/*secs*/ 15),
        app_server.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(error.error.code, -32603);
    Ok(())
}

#[tokio::test]
async fn feedback_upload_includes_sqlite_flush_and_query_failures() -> Result<()> {
    use std::io::Read;
    use std::sync::Arc;

    use app_test_support::MockResponsesConfig;
    use codex_app_server_protocol::ClientNotification;
    use codex_app_server_protocol::ThreadStartParams;
    use codex_state::SqliteConfig;
    use codex_utils_absolute_path::test_support::PathExt;
    use tokio::io::AsyncReadExt;
    use tokio_rustls::TlsAcceptor;
    use tokio_rustls::rustls;

    // Terminate HTTPS locally and capture real Sentry envelopes. Never forward uploads.
    let home = tempfile::tempdir()?;
    let cert = rcgen::generate_simple_self_signed(vec!["o33249.ingest.us.sentry.io".to_string()])?;
    let cert_path = home.path().join("feedback-ca.pem");
    std::fs::write(&cert_path, cert.cert.pem())?;
    let tls = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::aws_lc_rs::default_provider(),
    ))
    .with_safe_default_protocol_versions()?
    .with_no_client_auth()
    .with_single_cert(
        vec![cert.cert.der().clone()],
        rustls::pki_types::PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der()).into(),
    )?;
    let acceptor = TlsAcceptor::from(Arc::new(tls));
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let proxy_uri = format!("http://{}", listener.local_addr()?);
    let (uploads_tx, mut uploads_rx) = tokio::sync::mpsc::unbounded_channel();
    let _proxy = tokio_util::task::AbortOnDropHandle::new(tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let mut stream = BufReader::new(stream);
            loop {
                let mut line = String::new();
                anyhow::ensure!(stream.read_line(&mut line).await? > 0, "missing CONNECT");
                if line == "\r\n" {
                    break;
                }
            }
            stream
                .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                .await?;
            let mut stream = BufReader::new(acceptor.accept(stream.into_inner()).await?);
            let mut length = None;
            loop {
                let mut line = String::new();
                anyhow::ensure!(
                    stream.read_line(&mut line).await? > 0,
                    "missing HTTP headers"
                );
                if line == "\r\n" {
                    break;
                }
                if let Some((name, value)) = line.split_once(':')
                    && name.eq_ignore_ascii_case("content-length")
                {
                    length = Some(value.trim().parse::<usize>()?);
                }
            }
            let length = length.ok_or_else(|| anyhow::anyhow!("missing content length"))?;
            anyhow::ensure!(length < 16 * 1024 * 1024, "unexpected envelope size");
            let mut body = vec![0; length];
            stream.read_exact(&mut body).await?;
            if body.starts_with(&[0x1f, 0x8b]) {
                let mut decoded = Vec::new();
                flate2::read::GzDecoder::new(body.as_slice()).read_to_end(&mut decoded)?;
                body = decoded;
            }
            uploads_tx.send(body)?;
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}")
                .await?;
            stream.flush().await?;
        }
        Ok::<(), anyhow::Error>(())
    }));

    let models = wiremock::MockServer::start().await;
    MockResponsesConfig::new(&models.uri()).write(home.path())?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(home.path())
        .with_env_overrides(&[
            ("HTTPS_PROXY", Some(proxy_uri.as_str())),
            ("https_proxy", Some(proxy_uri.as_str())),
            ("NO_PROXY", Some("127.0.0.1,localhost")),
            ("no_proxy", Some("127.0.0.1,localhost")),
            ("SSL_CERT_FILE", cert_path.to_str()),
        ])
        .build_initialized()
        .await?;
    let request = app_server
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            ephemeral: Some(true),
            ..Default::default()
        })
        .await?;
    let response = app_server
        .read_stream_until_response_message(RequestId::Integer(request))
        .await?;
    let thread_id = response.result["thread"]["id"].as_str().unwrap();
    let sqlite = SqliteConfig::new_for_testing(home.path().abs());
    let pool = sqlite.open_read_write_pool(&sqlite.logs_db_path()).await?;
    // This old history exists only in SQLite, as if it had already left the memory ring.
    sqlx::query("INSERT INTO logs (ts, ts_nanos, level, target, feedback_log_body, thread_id) VALUES (1, 0, 'INFO', 'test', 'sqlite-only-history', ?)")
        .bind(thread_id).execute(&pool).await?;
    sqlx::query("CREATE TRIGGER fail_log_insert BEFORE INSERT ON logs BEGIN SELECT RAISE(ABORT, 'synthetic-secret'); END")
        .execute(&pool).await?;

    #[derive(Debug, PartialEq)]
    enum Phase {
        WriteFailure,
        Corrupt,
    }
    for phase in [Phase::WriteFailure, Phase::Corrupt] {
        match phase {
            Phase::Corrupt => {
                sqlx::raw_sql("PRAGMA writable_schema = ON; UPDATE sqlite_schema SET rootpage = 2147483647 WHERE name = 'logs'; PRAGMA schema_version = 1000000;").execute(&pool).await?;
            }
            Phase::WriteFailure => {}
        }
        // Notifications emit an INFO log, ensuring feedback/upload has a batch to flush.
        app_server
            .send_notification(ClientNotification::Initialized)
            .await?;
        // The periodic writer must notify the user before they choose to submit feedback.
        if phase == Phase::WriteFailure {
            let notification = timeout(
                Duration::from_secs(/*secs*/ 20),
                app_server.read_stream_until_notification_message("warning"),
            )
            .await??;
            let warning: codex_app_server_protocol::WarningNotification =
                serde_json::from_value(notification.params.unwrap())?;
            assert_eq!(warning, codex_app_server_protocol::WarningNotification {
                thread_id: None,
                message: "Codex couldn't save diagnostic logs to its local database. Use /feedback with logs included before closing Codex, or run `codex doctor` for diagnostics.".to_string(),
            });
        }
        let request = app_server
            .send_raw_request(
                "feedback/upload",
                Some(json!({
                    "classification": "bug", "includeLogs": true, "threadId": thread_id
                })),
            )
            .await?;
        timeout(
            Duration::from_secs(/*secs*/ 30),
            app_server.read_stream_until_response_message(RequestId::Integer(request)),
        )
        .await??;
        let mut attachments = std::collections::BTreeMap::new();
        while let Ok(envelope) = uploads_rx.try_recv() {
            let mut bytes = envelope.as_slice();
            // Skip the envelope header, then decode each length-delimited item.
            let header_end = bytes.iter().position(|byte| *byte == b'\n').unwrap();
            bytes = &bytes[header_end + 1..];
            while !bytes.is_empty() {
                let header_end = bytes.iter().position(|byte| *byte == b'\n').unwrap();
                let header: serde_json::Value = serde_json::from_slice(&bytes[..header_end])?;
                let length = header["length"].as_u64().unwrap() as usize;
                bytes = &bytes[header_end + 1..];
                if let Some(filename) = header["filename"].as_str() {
                    attachments.insert(
                        filename.to_string(),
                        String::from_utf8_lossy(&bytes[..length]).into_owned(),
                    );
                }
                bytes = &bytes[length..];
                if bytes.starts_with(b"\n") {
                    bytes = &bytes[1..];
                }
            }
        }
        let logs = &attachments["codex-logs.log"];
        if phase == Phase::Corrupt {
            assert!(
                !app_server
                    .pending_notification_methods()
                    .iter()
                    .any(|method| method == "warning")
            );
            assert!(logs.contains("failed to flush logs to SQLite error=\"corrupt\""));
            assert!(logs.contains("failed to query feedback logs from sqlite"));
        } else {
            assert!(logs.contains("failed to flush logs to SQLite error=\"constraint\""));
            assert!(!logs.contains("sqlite-only-history"));
            assert!(!logs.contains("synthetic-secret"));
        }
    }
    pool.close().await;
    Ok(())
}
