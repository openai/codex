//! Feedback event contracts for titles, report grouping, and lossless comment bodies.

use crate::CodexFeedback;
use crate::FeedbackUploadOptions;
use crate::UPLOAD_TIMEOUT;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use flate2::read::GzDecoder;
use http::StatusCode;
use pretty_assertions::assert_eq;
use sentry::protocol::Event;
use sentry::protocol::Exception;
use sentry::protocol::Level;
use sentry::protocol::Value;
use sentry::protocol::Values;
use sentry::protocol::value::to_value;
use std::collections::BTreeMap;
use std::io::Read;
use std::time::Instant;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[test]
fn custom_titles_preserve_comments_and_keep_submissions_separate() {
    let snapshot = CodexFeedback::new().snapshot(/*session_id*/ None);
    let tags = BTreeMap::from([
        (
            "feedback_title".to_string(),
            " \t\u{2003}Workflow  feedback\nwith 🌍 context\r\n ".to_string(),
        ),
        ("entrypoint".to_string(), "task-picker".to_string()),
    ]);
    let title = "Workflow  feedback\nwith 🌍 context";
    for (reason, reason_tag) in [
        (
            Some("  The\n\nworkflow\twas\u{2003}useful 🌍.  "),
            Some("  The  workflow\twas\u{2003}useful 🌍.  "),
        ),
        (Some(""), Some("")),
        (None, None),
    ] {
        let event =
            snapshot.feedback_event("other", reason, Some(&tags), /*session_source*/ None);
        let mut expected_tags = tags.clone();
        expected_tags.extend([
            ("thread_id".to_string(), snapshot.thread_id.clone()),
            ("classification".to_string(), "other".to_string()),
            (
                "cli_version".to_string(),
                env!("CARGO_PKG_VERSION").to_string(),
            ),
        ]);
        if let Some(reason_tag) = reason_tag {
            expected_tags.insert("reason".to_string(), reason_tag.to_string());
        }
        assert_eq!(
            event,
            Event {
                event_id: event.event_id,
                timestamp: event.timestamp,
                level: Level::Info,
                message: Some(title.to_string()),
                exception: reason
                    .map(|reason| Exception {
                        ty: title.to_string(),
                        value: Some(reason.to_string()),
                        ..Default::default()
                    })
                    .into_iter()
                    .collect::<Vec<_>>()
                    .into(),
                tags: expected_tags,
                fingerprint: vec![event.event_id.to_string().into()].into(),
                ..Default::default()
            }
        );
        let next_event =
            snapshot.feedback_event("other", reason, Some(&tags), /*session_source*/ None);
        assert_ne!(event.fingerprint, next_event.fingerprint);
    }
}

#[test]
fn missing_or_blank_titles_preserve_session_titles_and_default_grouping() {
    let snapshot = CodexFeedback::new().snapshot(/*session_id*/ None);
    let reason = "  Feedback\nwith 🌍 context.  ";
    for custom_title in [None, Some(""), Some(" \t\n\u{2003} ")] {
        let tags = custom_title
            .map(|title| BTreeMap::from([("feedback_title".to_string(), title.to_string())]));
        let event = snapshot.feedback_event(
            "bug",
            Some(reason),
            tags.as_ref(),
            /*session_source*/ None,
        );
        let title = format!("[Bug]: Codex session {}", snapshot.thread_id);
        let mut expected_tags = tags.unwrap_or_default();
        expected_tags.extend([
            ("thread_id".to_string(), snapshot.thread_id.clone()),
            ("classification".to_string(), "bug".to_string()),
            (
                "cli_version".to_string(),
                env!("CARGO_PKG_VERSION").to_string(),
            ),
            (
                "reason".to_string(),
                "  Feedback with 🌍 context.  ".to_string(),
            ),
        ]);
        assert_eq!(
            event,
            Event {
                event_id: event.event_id,
                timestamp: event.timestamp,
                level: Level::Error,
                message: Some(title.clone()),
                exception: vec![Exception {
                    ty: title,
                    value: Some(reason.to_string()),
                    ..Default::default()
                }]
                .into(),
                tags: expected_tags,
                ..Default::default()
            }
        );
    }
}

#[test]
fn reason_tag_previews_preserve_full_comments_in_the_exception() {
    let snapshot = CodexFeedback::new().snapshot(/*session_id*/ None);
    let title = format!("[Bug]: Codex session {}", snapshot.thread_id);
    for (reason, preview) in [
        ("A short comment".to_string(), "A short comment".to_string()),
        (
            "First\r\n\r\nSecond\rThird\n".to_string(),
            "First    Second Third ".to_string(),
        ),
        ("a".repeat(200), "a".repeat(200)),
        ("a".repeat(201), "a".repeat(200)),
        (format!("{}extra", "界🌍".repeat(100)), "界🌍".repeat(100)),
    ] {
        let event = snapshot.feedback_event(
            "bug",
            Some(&reason),
            /*tags*/ None,
            /*session_source*/ None,
        );
        assert_eq!(
            event,
            Event {
                event_id: event.event_id,
                timestamp: event.timestamp,
                level: Level::Error,
                message: Some(title.clone()),
                exception: vec![Exception {
                    ty: title.clone(),
                    value: Some(reason),
                    ..Default::default()
                }]
                .into(),
                tags: BTreeMap::from([
                    ("thread_id".to_string(), snapshot.thread_id.clone()),
                    ("classification".to_string(), "bug".to_string()),
                    (
                        "cli_version".to_string(),
                        env!("CARGO_PKG_VERSION").to_string()
                    ),
                    ("reason".to_string(), preview),
                ]),
                ..Default::default()
            }
        );
    }
}

#[tokio::test]
async fn feedback_upload_sends_safe_reason_tag_and_full_comment() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/42/envelope/"))
        .and(header("Content-Encoding", "gzip"))
        .respond_with(ResponseTemplate::new(StatusCode::OK))
        .expect(/*r*/ 1)
        .mount(&server)
        .await;

    let snapshot = CodexFeedback::new().snapshot(/*session_id*/ None);
    let reason = format!("first\r\nsecond\n{}tail", "界🌍".repeat(120));
    let tags = BTreeMap::from([("reason".to_string(), "caller override\n".to_string())]);
    snapshot
        .upload_feedback_with_dsn(
            FeedbackUploadOptions {
                classification: "bug",
                reason: Some(&reason),
                tags: Some(&tags),
                include_logs: false,
                extra_attachments: &[],
                extra_attachment_paths: &[],
                session_source: None,
                logs_override: None,
            },
            &HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
            &format!("http://public@{}/42", server.address()),
            Instant::now() + UPLOAD_TIMEOUT,
        )
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let mut decoded = Vec::new();
    GzDecoder::new(requests[0].body.as_slice())
        .read_to_end(&mut decoded)
        .unwrap();
    // The first two envelope lines are its header and the event item header.
    let payload = std::str::from_utf8(&decoded)
        .unwrap()
        .splitn(/*n*/ 3, '\n')
        .nth(/*n*/ 2)
        .unwrap();
    let event: Value = payload.parse().unwrap();
    assert_eq!(
        event["tags"]["reason"].as_str().unwrap().chars().count(),
        200
    );
    let title = format!("[Bug]: Codex session {}", snapshot.thread_id);
    let expected_tags = to_value(BTreeMap::from([
        ("thread_id".to_string(), snapshot.thread_id),
        ("classification".to_string(), "bug".to_string()),
        (
            "cli_version".to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
        ),
        (
            "reason".to_string(),
            format!("first  second {}", "界🌍".repeat(93)),
        ),
    ]))
    .unwrap();
    let expected_exception = to_value(Values::from(vec![Exception {
        ty: title,
        value: Some(reason),
        ..Default::default()
    }]))
    .unwrap();
    assert_eq!(
        (&event["tags"], &event["exception"]),
        (&expected_tags, &expected_exception)
    );
}
