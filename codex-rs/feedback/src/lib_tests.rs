#![cfg(test)]

//! Unit tests for the crate and its related test suites.

use std::ffi::OsStr;
use std::fs;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use super::*;
use crate::FeedbackDiagnostic;
use codex_http_client::OutboundProxyPolicy;
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use http::StatusCode;
use pretty_assertions::assert_eq;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::header_exists;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[test]
fn ring_buffer_drops_front_when_full() {
    let fb = CodexFeedback::with_capacity(/*max_bytes*/ 8);
    {
        let mut w = fb.make_writer().make_writer();
        w.write_all(b"abcdefgh").unwrap();
        w.write_all(b"ij").unwrap();
    }
    let snap = fb.snapshot(/*session_id*/ None);
    // Capacity 8: after writing 10 bytes, we should keep the last 8.
    pretty_assertions::assert_eq!(std::str::from_utf8(&snap.bytes).unwrap(), "cdefghij");
}

#[test]
fn logger_layer_filters_noisy_trace_payloads() {
    let fb = CodexFeedback::new();
    let _guard = tracing_subscriber::registry()
        // Keep another TRACE subscriber interested so bridged records are
        // emitted; feedback must still reject them with its own filter.
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::sink))
        .with(fb.logger_layer())
        .set_default();

    tracing::trace!(target: "codex_api::responses_websocket_timing", payload = "secret");
    tracing::event!(
        target: "codex_otel.log_only", tracing::Level::INFO,
        event.name = "codex.agent_response", response = "private-agent-response"
    );
    tracing::info!(target: "codex_otel.log_only", rationale = "private-guardian-rationale");
    tracing::trace!(target: "codex_http_client::transport", "transport-trace");
    tracing::trace!(target: "codex_api::sse", "sse-trace");
    tracing::trace!(target: "codex_api::sse::responses", "nested-sse-trace");
    tracing::debug!(target: "codex_http_client::transport", "transport-debug");
    tracing::debug!(target: "codex_api::sse::responses", "sse-debug");
    tracing::trace!(target: "codex_feedback_test", "unrelated-trace");
    log::trace!(target: "codex_feedback_test", "unrelated-log-trace");
    log::trace!(
        target: "tungstenite::handshake::client",
        "websocket-handshake-payload"
    );
    log::trace!(target: "tungstenite::protocol", "websocket-frame-payload");
    log::debug!(target: "tungstenite::protocol", "websocket-debug");

    let logs = String::from_utf8(fb.snapshot(/*session_id*/ None).bytes).unwrap();
    for excluded in [
        "secret",
        "private-agent-response",
        "private-guardian-rationale",
        "transport-trace",
        "sse-trace",
        "nested-sse-trace",
        "websocket-handshake-payload",
        "websocket-frame-payload",
    ] {
        assert!(!logs.contains(excluded));
    }
    for retained in [
        "transport-debug",
        "sse-debug",
        "unrelated-trace",
        "unrelated-log-trace",
        "websocket-debug",
    ] {
        assert!(logs.contains(retained));
    }
}

#[test]
fn metadata_layer_records_tags_from_feedback_target() {
    let fb = CodexFeedback::new();
    let _guard = tracing_subscriber::registry()
        .with(fb.metadata_layer())
        .set_default();

    tracing::info!(target: FEEDBACK_TAGS_TARGET, model = "gpt-5", cached = true, "tags");

    let snap = fb.snapshot(/*session_id*/ None);
    pretty_assertions::assert_eq!(snap.tags.get("model").map(String::as_str), Some("gpt-5"));
    pretty_assertions::assert_eq!(snap.tags.get("cached").map(String::as_str), Some("true"));
}

async fn upload_test_feedback(
    feedback: &CodexFeedback,
    dsn: &str,
    extra_attachments: Vec<FeedbackAttachment>,
) -> Result<()> {
    feedback
        .snapshot(/*session_id*/ None)
        .with_feedback_diagnostics(FeedbackDiagnostics::default())
        .upload_feedback_with_dsn(
            FeedbackUploadOptions {
                classification: "bug",
                reason: Some("private feedback"),
                tags: None,
                include_logs: true,
                extra_attachments,
                extra_attachment_paths: &[],
                session_source: Some(SessionSource::Cli),
                logs_override: Some(b"private log contents".to_vec()),
            },
            &HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
            dsn,
            Instant::now() + UPLOAD_TIMEOUT,
        )
        .await
}

#[tokio::test]
async fn feedback_upload_allows_slow_reports() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/42/envelope/"))
        .respond_with(
            ResponseTemplate::new(StatusCode::OK).set_delay(Duration::from_secs(/*secs*/ 4)),
        )
        .expect(/*r*/ 3)
        .mount(&server)
        .await;

    let dsn = format!("http://public@{}/42", server.address());
    let attachment = FeedbackAttachment {
        filename: "later.txt".to_string(),
        content_type: None,
        buffer: b"later diagnostic".to_vec(),
    };
    upload_test_feedback(&CodexFeedback::new(), &dsn, vec![attachment])
        .await
        .expect("all three envelopes should finish across twelve seconds of network waits");
}

#[tokio::test]
async fn feedback_upload_deadline_stops_retries_and_later_attachments() {
    let server = MockServer::start().await;
    let attempt = AtomicUsize::default();
    Mock::given(method("POST"))
        .and(path("/api/42/envelope/"))
        .respond_with(move |_: &wiremock::Request| {
            match attempt.fetch_add(/*val*/ 1, Ordering::SeqCst) {
                0 | 2 => ResponseTemplate::new(StatusCode::OK)
                    .set_delay(Duration::from_secs(/*secs*/ 1)),
                1 => ResponseTemplate::new(StatusCode::SERVICE_UNAVAILABLE)
                    .set_delay(Duration::from_secs(/*secs*/ 1)),
                _ => ResponseTemplate::new(StatusCode::OK)
                    .set_delay(Duration::from_secs(/*secs*/ 20)),
            }
        })
        .expect(/*r*/ 4)
        .mount(&server)
        .await;

    let dsn = format!("http://public@{}/42", server.address());
    let attachments = ["stalled.txt", "later.txt"].map(|filename| FeedbackAttachment {
        filename: filename.to_string(),
        content_type: None,
        buffer: filename.as_bytes().to_vec(),
    });
    let snapshot = CodexFeedback::new()
        .snapshot(/*session_id*/ None)
        .with_feedback_diagnostics(FeedbackDiagnostics::default());
    let error = tokio::time::timeout(
        Duration::from_secs(/*secs*/ 10),
        snapshot.upload_feedback_with_dsn(
            FeedbackUploadOptions {
                classification: "bug",
                reason: None,
                tags: None,
                include_logs: true,
                extra_attachments: attachments.into(),
                extra_attachment_paths: &[],
                session_source: Some(SessionSource::Cli),
                logs_override: Some(b"log contents".to_vec()),
            },
            &HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
            &dsn,
            Instant::now() + Duration::from_secs(/*secs*/ 8),
        ),
    )
    .await
    .expect("each request must use only the time left in the report deadline")
    .expect_err("the accepted report must report incomplete attachments");
    assert_eq!(
        error.to_string(),
        "feedback report was accepted, but some attachments failed to upload"
    );
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[1].body, requests[2].body);
}

#[tokio::test]
async fn feedback_upload_retries_diagnostics_without_replaying_core() {
    let server = MockServer::start().await;
    let attempt = AtomicUsize::default();
    Mock::given(method("POST"))
        .and(path("/api/42/envelope/"))
        .and(header_exists("X-Sentry-Auth"))
        .and(header("Content-Encoding", "gzip"))
        .respond_with(move |_: &wiremock::Request| {
            match attempt.fetch_add(/*val*/ 1, Ordering::SeqCst) {
                // Accept the core once; accept logs only on their second attempt.
                0 => ResponseTemplate::new(StatusCode::OK).insert_header(
                    "X-Sentry-Rate-Limits",
                    "0:attachment:project,60:transaction:project",
                ),
                2 => ResponseTemplate::new(StatusCode::OK).insert_header("Retry-After", "0"),
                6 => ResponseTemplate::new(StatusCode::OK)
                    .insert_header("X-Sentry-Rate-Limits", "60:transaction:project"),
                1 => ResponseTemplate::new(StatusCode::SERVICE_UNAVAILABLE)
                    .insert_header("Retry-After", "2"),
                5 => ResponseTemplate::new(StatusCode::SERVICE_UNAVAILABLE)
                    .insert_header("Retry-After", "8"),
                _ => ResponseTemplate::new(StatusCode::SERVICE_UNAVAILABLE)
                    .insert_header("Retry-After", "0"),
            }
        })
        .expect(/*r*/ 7)
        .mount(&server)
        .await;

    let dsn = format!("http://public@{}/42", server.address());
    let attachments = ["retry-exhausted.txt", "later.txt"].map(|filename| FeedbackAttachment {
        filename: filename.to_string(),
        content_type: Some("text/plain".to_string()),
        buffer: filename.as_bytes().to_vec(),
    });
    let started = Instant::now();
    upload_test_feedback(&CodexFeedback::new(), &dsn, attachments.into())
        .await
        .expect_err("legacy uploads report incomplete diagnostics");
    assert!(started.elapsed() >= Duration::from_secs(2));
    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests[1].body, requests[2].body,
        "retry exact request bytes"
    );
    // Exhausting one diagnostic must not delay later.txt or replay earlier files.
    assert!(
        requests[3..6]
            .iter()
            .all(|request| request.body == requests[3].body)
    );
    let mut decoded = String::new();
    GzDecoder::new(requests[1].body.as_slice())
        .read_to_string(&mut decoded)
        .unwrap();
    assert!(decoded.ends_with("private log contents\n"));
}

#[tokio::test]
async fn feedback_upload_delivers_large_files_intact() {
    let suffix = ThreadId::new();
    let first_path = std::env::temp_dir().join(format!("feedback-first-{suffix}.jsonl"));
    let second_path = std::env::temp_dir().join(format!("feedback-second-{suffix}.jsonl"));
    let binary_path = std::env::temp_dir().join(format!("feedback-archive-{suffix}.jsonl.gz"));
    let pending_path = second_path.with_extension("pending");
    let block = "{\"type\":\"event_msg\",\"payload\":{\"type\":\"agent_message\",\"message\":\"diagnostic fixture 🍵\"}}\n"
            .repeat(1024);
    let block_counts = [65, 80].map(|mib| mib * 1024 * 1024 / block.len() + 1);
    for (path, blocks) in [&first_path, &pending_path].into_iter().zip(block_counts) {
        let mut file = fs::File::create(path).unwrap();
        for _ in 0..blocks {
            file.write_all(block.as_bytes()).unwrap();
        }
    }
    // Repeats beyond gzip's window keep the archive above the former 38.1 MiB cap.
    let record_block = (0..4096)
        .flat_map(|_| sentry::types::Uuid::new_v4().into_bytes())
        .map(|byte| {
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
                [usize::from(byte % 64)]
        })
        .collect::<Vec<_>>();
    let mut file = GzEncoder::new(fs::File::create(&binary_path).unwrap(), Compression::fast());
    for mib in [32, 30] {
        file.write_all(b"{\"message\":\"").unwrap();
        for _ in 0..mib * 1024 * 1024 / record_block.len() {
            file.write_all(&record_block).unwrap();
        }
        file.write_all(b"\"}\n").unwrap();
    }
    file.finish().unwrap();
    assert!(fs::metadata(&binary_path).unwrap().len() > 40_000_000);

    let server = MockServer::start().await;
    let attempt = AtomicUsize::default();
    let ready_path = second_path.clone();
    Mock::given(method("POST"))
        .and(path("/api/42/envelope/"))
        .respond_with(move |_: &wiremock::Request| {
            if attempt.fetch_add(/*val*/ 1, Ordering::SeqCst) == 1 {
                // The next file becomes readable only after the first file arrives.
                fs::rename(&pending_path, &ready_path).unwrap();
            }
            ResponseTemplate::new(StatusCode::OK)
        })
        .expect(/*r*/ 4)
        .mount(&server)
        .await;

    CodexFeedback::new()
        .snapshot(/*session_id*/ None)
        .upload_feedback_with_dsn(
            FeedbackUploadOptions {
                classification: "bug",
                reason: Some("large diagnostic upload"),
                tags: None,
                include_logs: false,
                extra_attachments: Vec::new(),
                extra_attachment_paths: &[
                    FeedbackAttachmentPath {
                        path: first_path.clone(),
                        attachment_filename_override: None,
                    },
                    FeedbackAttachmentPath {
                        path: second_path.clone(),
                        attachment_filename_override: None,
                    },
                    FeedbackAttachmentPath {
                        path: binary_path.clone(),
                        attachment_filename_override: None,
                    },
                ],
                session_source: Some(SessionSource::Cli),
                logs_override: None,
            },
            &HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
            &format!("http://public@{}/42", server.address()),
            Instant::now() + UPLOAD_TIMEOUT,
        )
        .await
        .unwrap();
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 4);
    let mut event = String::new();
    GzDecoder::new(requests[0].body.as_slice())
        .read_to_string(&mut event)
        .unwrap();
    for (request, (filename, source_path)) in requests[1..].iter().zip([
        (format!("feedback-first-{suffix}.jsonl"), &first_path),
        (format!("feedback-second-{suffix}.jsonl"), &second_path),
        (format!("feedback-archive-{suffix}.jsonl.gz"), &binary_path),
    ]) {
        let mut decoded = Vec::new();
        if request.headers.contains_key("Content-Encoding") {
            GzDecoder::new(request.body.as_slice())
                .read_to_end(&mut decoded)
                .unwrap();
        } else {
            decoded.extend_from_slice(&request.body);
        }
        assert!(decoded.len() <= MAX_DECODED_UPLOAD_BYTES);
        let mut parts = decoded.splitn(3, |byte| *byte == b'\n');
        assert_eq!(
            parts.next().unwrap(),
            event.lines().next().unwrap().as_bytes()
        );
        let header = std::str::from_utf8(parts.next().unwrap()).unwrap();
        assert!(header.contains(&format!("\"filename\":\"{filename}\"")));
        let payload = parts.next().unwrap().strip_suffix(b"\n").unwrap();
        let mut source = fs::File::open(source_path).unwrap();
        let original_bytes = source.metadata().unwrap().len() as usize;
        assert_eq!(payload.len(), original_bytes);
        if filename.ends_with(".jsonl") {
            let text = std::str::from_utf8(payload).unwrap();
            assert!(text.ends_with('\n'));
        } else {
            let mut archive = GzDecoder::new(payload);
            io::copy(&mut archive, &mut io::sink()).expect("received archive must be intact");
        }
        let mut expected = [0; 64 * 1024];
        for chunk in payload.chunks(expected.len()) {
            source.read_exact(&mut expected[..chunk.len()]).unwrap();
            assert_eq!(chunk, &expected[..chunk.len()]);
        }
    }
    fs::remove_file(first_path).unwrap();
    fs::remove_file(second_path).unwrap();
    fs::remove_file(binary_path).unwrap();
}

#[tokio::test]
async fn feedback_upload_reports_unreadable_or_oversized_attachments() {
    let oversized_path =
        std::env::temp_dir().join(format!("feedback-oversized-{}", ThreadId::new()));
    fs::File::create(&oversized_path)
        .unwrap()
        .set_len(MAX_DECODED_UPLOAD_BYTES as u64 + 1)
        .unwrap();
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(StatusCode::OK))
        .expect(/*r*/ 2)
        .mount(&server)
        .await;

    for path in [
        oversized_path.clone(),
        oversized_path.with_extension("missing"),
    ] {
        let result = CodexFeedback::new()
            .snapshot(/*session_id*/ None)
            .upload_feedback_with_dsn(
                FeedbackUploadOptions {
                    classification: "bug",
                    reason: None,
                    tags: None,
                    include_logs: false,
                    extra_attachments: Vec::new(),
                    extra_attachment_paths: &[FeedbackAttachmentPath {
                        path,
                        attachment_filename_override: None,
                    }],
                    session_source: Some(SessionSource::Cli),
                    logs_override: None,
                },
                &HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
                &format!("http://public@{}/42", server.address()),
                Instant::now() + UPLOAD_TIMEOUT,
            )
            .await;
        assert_eq!(
            result.unwrap_err().to_string(),
            "feedback report was accepted, but some attachments failed to upload"
        );
    }
    assert_eq!(
        fs::metadata(&oversized_path).unwrap().len(),
        MAX_DECODED_UPLOAD_BYTES as u64 + 1
    );
    fs::remove_file(oversized_path).unwrap();
}

#[tokio::test]
async fn feedback_upload_stops_diagnostics_for_sentry_rate_limits() {
    for (accepted_before_limit, rate_limit) in [
        (1, ResponseTemplate::new(StatusCode::TOO_MANY_REQUESTS)),
        (
            1,
            ResponseTemplate::new(StatusCode::TOO_MANY_REQUESTS)
                .insert_header("X-Sentry-Rate-Limits", "60:attachment:project")
                .insert_header("Retry-After", "0"),
        ),
        (
            0,
            ResponseTemplate::new(StatusCode::OK)
                .insert_header("X-Sentry-Rate-Limits", "60:attachment:project"),
        ),
        (
            1,
            ResponseTemplate::new(StatusCode::OK)
                .insert_header("X-Sentry-Rate-Limits", "60::project"),
        ),
        (
            0,
            ResponseTemplate::new(StatusCode::OK).insert_header("Retry-After", "60"),
        ),
        (
            1,
            ResponseTemplate::new(StatusCode::SERVICE_UNAVAILABLE)
                .insert_header("Retry-After", "300"),
        ),
        (
            1,
            ResponseTemplate::new(StatusCode::SERVICE_UNAVAILABLE)
                .insert_header("Retry-After", "invalid"),
        ),
    ] {
        let server = MockServer::start().await;
        let attempt = AtomicUsize::default();
        Mock::given(method("POST"))
            .and(path("/api/42/envelope/"))
            .respond_with(move |_: &wiremock::Request| {
                if attempt.fetch_add(/*val*/ 1, Ordering::SeqCst) < accepted_before_limit {
                    ResponseTemplate::new(StatusCode::OK)
                } else {
                    rate_limit.clone()
                }
            })
            .expect((accepted_before_limit + 1) as u64)
            .mount(&server)
            .await;

        let dsn = format!("http://public@{}/42", server.address());
        let later = FeedbackAttachment {
            filename: "later.txt".to_string(),
            content_type: None,
            buffer: b"later diagnostic".to_vec(),
        };
        upload_test_feedback(&CodexFeedback::new(), &dsn, vec![later])
            .await
            .expect_err("legacy uploads report rate-limited diagnostics");
    }
}

#[tokio::test]
async fn feedback_upload_reports_rejected_sentry_response_without_exposing_feedback() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/42/envelope/"))
        .respond_with(ResponseTemplate::new(503))
        .expect(1)
        .mount(&server)
        .await;

    let feedback = CodexFeedback::new();
    let dsn = format!("http://public@{}/42", server.address());

    let error = upload_test_feedback(&feedback, &dsn, Vec::new())
        .await
        .expect_err("rejected Sentry responses must fail feedback uploads");

    let error = format!("{error:#}");
    assert!(error.contains("503"));
    assert!(!error.contains("private feedback"));
}

#[tokio::test]
async fn feedback_upload_does_not_forward_private_data_to_redirect_target() {
    let sentry_server = MockServer::start().await;
    let redirect_target = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/42/envelope/"))
        .respond_with(
            ResponseTemplate::new(307)
                .insert_header("Location", format!("{}/capture", redirect_target.uri())),
        )
        .expect(1)
        .mount(&sentry_server)
        .await;
    let dsn = format!("http://public@{}/42", sentry_server.address());
    let error = upload_test_feedback(&CodexFeedback::new(), &dsn, Vec::new())
        .await
        .expect_err("redirected feedback uploads must be rejected");

    assert!(error.to_string().contains("307"));
    assert!(
        redirect_target
            .received_requests()
            .await
            .expect("redirect target should record requests")
            .is_empty()
    );
}

#[tokio::test]
async fn feedback_upload_reports_transport_failures() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("unused local address should be available");
    let address = listener
        .local_addr()
        .expect("listener should have an address");
    drop(listener);

    let dsn = format!("http://public@{address}/42");
    let error = upload_test_feedback(&CodexFeedback::new(), &dsn, Vec::new())
        .await
        .expect_err("transport failures must fail feedback uploads");

    assert!(
        error
            .downcast_ref::<codex_http_client::RouteAwareRequestError>()
            .is_some_and(codex_http_client::RouteAwareRequestError::is_connect)
    );
}

#[test]
fn feedback_attachments_gate_connectivity_diagnostics() {
    let extra_filename = format!("codex-feedback-extra-{}.jsonl", ThreadId::new());
    let extra_path = std::env::temp_dir().join(&extra_filename);
    let extra_attachment_path = FeedbackAttachmentPath {
        path: extra_path.clone(),
        attachment_filename_override: None,
    };
    fs::write(&extra_path, "rollout").expect("extra attachment should be written");

    let snapshot_with_diagnostics = CodexFeedback::new()
        .snapshot(/*session_id*/ None)
        .with_feedback_diagnostics(FeedbackDiagnostics::new(vec![FeedbackDiagnostic {
            headline: "Proxy environment variables are set and may affect connectivity."
                .to_string(),
            details: vec!["HTTPS_PROXY = https://example.com:443".to_string()],
        }]));

    let attachments_with_diagnostics = snapshot_with_diagnostics
        .feedback_attachments(
            &sentry::protocol::EnvelopeHeaders::default(),
            /*include_logs*/ true,
            vec![FeedbackAttachment {
                filename: DOCTOR_REPORT_ATTACHMENT_FILENAME.to_string(),
                content_type: Some("application/json".to_string()),
                buffer: b"{\"overallStatus\":\"ok\"}".to_vec(),
            }],
            &[extra_attachment_path],
            Some(vec![1]),
        )
        .collect::<Result<Vec<_>>>()
        .unwrap();

    assert_eq!(
        attachments_with_diagnostics
            .iter()
            .map(|attachment| attachment.filename.as_str())
            .collect::<Vec<_>>(),
        vec![
            "codex-logs.log",
            DOCTOR_REPORT_ATTACHMENT_FILENAME,
            FEEDBACK_DIAGNOSTICS_ATTACHMENT_FILENAME,
            extra_filename.as_str()
        ]
    );
    assert_eq!(attachments_with_diagnostics[0].buffer, vec![1]);
    assert_eq!(
        attachments_with_diagnostics[1].buffer,
        b"{\"overallStatus\":\"ok\"}".to_vec()
    );
    assert_eq!(
            attachments_with_diagnostics[2].buffer,
            b"Connectivity diagnostics\n\n- Proxy environment variables are set and may affect connectivity.\n  - HTTPS_PROXY = https://example.com:443".to_vec()
        );
    assert_eq!(attachments_with_diagnostics[3].buffer, b"rollout".to_vec());
    assert_eq!(
        attachments_with_diagnostics[3].content_type.as_deref(),
        Some("text/plain")
    );
    assert_eq!(
        OsStr::new(attachments_with_diagnostics[3].filename.as_str()),
        OsStr::new(extra_filename.as_str())
    );
    let attachments_without_diagnostics = CodexFeedback::new()
        .snapshot(/*session_id*/ None)
        .with_feedback_diagnostics(FeedbackDiagnostics::default())
        .feedback_attachments(
            &sentry::protocol::EnvelopeHeaders::default(),
            /*include_logs*/ true,
            Vec::new(),
            &[],
            Some(vec![1]),
        )
        .collect::<Result<Vec<_>>>()
        .unwrap();

    assert_eq!(
        attachments_without_diagnostics
            .iter()
            .map(|attachment| attachment.filename.as_str())
            .collect::<Vec<_>>(),
        vec!["codex-logs.log"]
    );
    assert_eq!(attachments_without_diagnostics[0].buffer, vec![1]);
    fs::remove_file(extra_path).expect("extra attachment should be removed");
}

#[test]
fn path_backed_attachments_use_binary_content_types() {
    let suffix = ThreadId::new();
    let gzip_filename = format!("codex-desktop-app-logs-{suffix}.tar.gz");
    let unknown_filename = format!("codex-feedback-extra-{suffix}.binunknown");
    let gzip_path = std::env::temp_dir().join(&gzip_filename);
    let unknown_path = std::env::temp_dir().join(&unknown_filename);
    let gzip_bytes = b"\x1f\x8b\x08\x00\xff";
    let unknown_bytes = b"\x00\x9f\x92\x96";
    fs::write(&gzip_path, gzip_bytes).expect("gzip attachment should be written");
    fs::write(&unknown_path, unknown_bytes).expect("unknown attachment should be written");

    let attachments = CodexFeedback::new()
        .snapshot(/*session_id*/ None)
        .feedback_attachments(
            &sentry::protocol::EnvelopeHeaders::default(),
            /*include_logs*/ false,
            Vec::new(),
            &[
                FeedbackAttachmentPath {
                    path: gzip_path.clone(),
                    attachment_filename_override: None,
                },
                FeedbackAttachmentPath {
                    path: unknown_path.clone(),
                    attachment_filename_override: None,
                },
            ],
            /*logs_override*/ None,
        )
        .collect::<Result<Vec<_>>>()
        .unwrap();

    fs::remove_file(gzip_path).expect("gzip attachment should be removed");
    fs::remove_file(unknown_path).expect("unknown attachment should be removed");
    assert_eq!(
        attachments
            .iter()
            .map(|attachment| (
                attachment.filename.as_str(),
                attachment.content_type.as_deref(),
                attachment.buffer.as_slice(),
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                gzip_filename.as_str(),
                Some("application/gzip"),
                gzip_bytes.as_slice(),
            ),
            (
                unknown_filename.as_str(),
                Some("application/octet-stream"),
                unknown_bytes.as_slice(),
            ),
        ]
    );
}

#[test]
fn upload_tags_include_client_tags_and_preserve_reserved_fields() {
    let mut tags = BTreeMap::new();
    tags.insert("thread_id".to_string(), "wrong-thread".to_string());
    tags.insert("turn_id".to_string(), "wrong-turn".to_string());
    tags.insert(
        "classification".to_string(),
        "wrong-classification".to_string(),
    );
    tags.insert("cli_version".to_string(), "wrong-version".to_string());
    tags.insert("session_source".to_string(), "wrong-source".to_string());
    tags.insert("reason".to_string(), "wrong-reason".to_string());
    tags.insert("account_id".to_string(), "actual-account".to_string());
    tags.insert("model".to_string(), "gpt-5".to_string());
    tags.insert("effort".to_string(), "Some(High)".to_string());
    let snapshot = FeedbackSnapshot {
        bytes: Vec::new(),
        tags,
        feedback_diagnostics: FeedbackDiagnostics::default(),
        thread_id: "thread-123".to_string(),
    };
    let mut client_tags = BTreeMap::new();
    client_tags.insert("thread_id".to_string(), "wrong-client-thread".to_string());
    client_tags.insert("turn_id".to_string(), "turn-456".to_string());
    client_tags.insert(
        "classification".to_string(),
        "wrong-client-classification".to_string(),
    );
    client_tags.insert(
        "cli_version".to_string(),
        "wrong-client-version".to_string(),
    );
    client_tags.insert(
        "session_source".to_string(),
        "wrong-client-source".to_string(),
    );
    client_tags.insert("reason".to_string(), "wrong-client-reason".to_string());
    client_tags.insert("client_tag".to_string(), "from-client".to_string());
    client_tags.insert("model".to_string(), "mewthree".to_string());
    client_tags.insert("effort".to_string(), "Some(Ultra)".to_string());

    let upload_tags = snapshot.upload_tags(
        "bug",
        Some("actual reason"),
        Some(&client_tags),
        Some(&SessionSource::Cli),
    );

    assert_eq!(
        upload_tags.get("thread_id").map(String::as_str),
        Some("thread-123")
    );
    assert_eq!(
        upload_tags.get("turn_id").map(String::as_str),
        Some("turn-456")
    );
    assert_eq!(
        upload_tags.get("classification").map(String::as_str),
        Some("bug")
    );
    assert_eq!(
        upload_tags.get("session_source").map(String::as_str),
        Some("cli")
    );
    assert_eq!(
        upload_tags.get("reason").map(String::as_str),
        Some("actual reason")
    );
    assert_eq!(
        upload_tags.get("account_id").map(String::as_str),
        Some("actual-account")
    );
    assert_eq!(
        upload_tags.get("model").map(String::as_str),
        Some("mewthree")
    );
    assert_eq!(
        upload_tags.get("effort").map(String::as_str),
        Some("Some(Ultra)")
    );
    assert_eq!(
        upload_tags.get("client_tag").map(String::as_str),
        Some("from-client")
    );
}

#[path = "feedback_event_tests.rs"]
mod feedback_event_tests;
#[path = "metadata_tests.rs"]
mod metadata_tests;
#[path = "rollout_attachment_tests.rs"]
mod rollout_attachment_tests;
#[path = "save_rollout_tests.rs"]
mod save_rollout_tests;
