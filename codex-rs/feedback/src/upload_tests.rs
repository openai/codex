use anyhow::Context;
use anyhow::Result;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_http_client::cache_system_proxy_route_for_test;
use pretty_assertions::assert_eq;
use sentry::ClientOptions;
use sentry::types::Auth;
use sentry::types::Dsn;
use serde_json::Value;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

use crate::CodexFeedback;
use crate::FeedbackAttachment;
use crate::FeedbackDiagnostics;
use crate::FeedbackUploadOptions;

#[tokio::test]
async fn sends_authenticated_envelope_with_existing_event_and_attachments() -> Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/42/envelope/"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;

    let dsn = format!("http://public@{}/42", server.address()).parse::<Dsn>()?;
    let snapshot = CodexFeedback::new()
        .snapshot(/*session_id*/ None)
        .with_feedback_diagnostics(FeedbackDiagnostics::default());
    let extra_attachments = [FeedbackAttachment {
        filename: "doctor.json".to_string(),
        content_type: Some("application/json".to_string()),
        buffer: br#"{"ok":true}"#.to_vec(),
    }];
    let upload = snapshot.prepare_feedback_upload_with_dsn(
        FeedbackUploadOptions {
            classification: "bug",
            reason: Some("proxy regression"),
            tags: None,
            include_logs: true,
            extra_attachments: &extra_attachments,
            extra_attachment_paths: &[],
            session_source: None,
            logs_override: Some(b"captured logs".to_vec()),
        },
        &dsn,
    )?;

    upload
        .send(&HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault))
        .await?;

    let requests = server.received_requests().await.unwrap_or_default();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    let user_agent = ClientOptions::default().user_agent;
    let authorization = request
        .headers
        .get("x-sentry-auth")
        .context("feedback request should include Sentry authentication")?
        .to_str()?
        .parse::<Auth>()?;
    assert_eq!(
        (
            authorization.public_key(),
            authorization.version(),
            authorization.client_agent(),
            authorization.timestamp().is_some(),
        ),
        ("public", 7, Some(user_agent.as_ref()), true)
    );

    let body = String::from_utf8(request.body.clone())?;
    let lines = body.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 7);
    let event = serde_json::from_str::<Value>(lines[2])?;
    assert_eq!(
        (
            event["message"].as_str(),
            event["tags"]["classification"].as_str(),
            event["exception"]["values"][0]["value"].as_str(),
        ),
        (
            Some(format!("[Bug]: Codex session {}", snapshot.thread_id).as_str()),
            Some("bug"),
            Some("proxy regression"),
        )
    );

    let logs_header = serde_json::from_str::<Value>(lines[3])?;
    let doctor_header = serde_json::from_str::<Value>(lines[5])?;
    assert_eq!(
        vec![
            (
                logs_header["filename"].as_str(),
                logs_header["content_type"].as_str(),
                lines[4],
            ),
            (
                doctor_header["filename"].as_str(),
                doctor_header["content_type"].as_str(),
                lines[6],
            ),
        ],
        vec![
            (Some("codex-logs.log"), Some("text/plain"), "captured logs"),
            (
                Some("doctor.json"),
                Some("application/json"),
                r#"{"ok":true}"#,
            ),
        ]
    );

    Ok(())
}

#[tokio::test]
async fn respects_system_proxy_for_sentry_envelope_destination() -> Result<()> {
    let proxy = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/42/envelope/"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&proxy)
        .await;

    let dsn = "http://public@feedback-proxy.invalid/42".parse::<Dsn>()?;
    let endpoint = dsn.envelope_api_url().to_string();
    cache_system_proxy_route_for_test(&endpoint, proxy.uri());
    let upload = CodexFeedback::new()
        .snapshot(/*session_id*/ None)
        .prepare_feedback_upload_with_dsn(
            FeedbackUploadOptions {
                classification: "bug",
                reason: None,
                tags: None,
                include_logs: false,
                extra_attachments: &[],
                extra_attachment_paths: &[],
                session_source: None,
                logs_override: None,
            },
            &dsn,
        )?;

    upload
        .send(&HttpClientFactory::new(
            OutboundProxyPolicy::RespectSystemProxy,
        ))
        .await?;

    Ok(())
}

#[tokio::test]
async fn returns_sentry_http_failures_to_the_caller() -> Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/42/envelope/"))
        .respond_with(ResponseTemplate::new(503))
        .expect(1)
        .mount(&server)
        .await;

    let dsn = format!("http://public@{}/42", server.address()).parse::<Dsn>()?;
    let upload = CodexFeedback::new()
        .snapshot(/*session_id*/ None)
        .prepare_feedback_upload_with_dsn(
            FeedbackUploadOptions {
                classification: "bug",
                reason: None,
                tags: None,
                include_logs: false,
                extra_attachments: &[],
                extra_attachment_paths: &[],
                session_source: None,
                logs_override: None,
            },
            &dsn,
        )?;

    let error = upload
        .send(&HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault))
        .await
        .expect_err("Sentry HTTP failures should fail the feedback upload");
    assert!(format!("{error:#}").contains("503"));

    Ok(())
}

#[tokio::test]
async fn returns_invalid_proxy_configuration_to_the_caller() -> Result<()> {
    let dsn = "http://public@invalid-feedback-proxy.invalid/42".parse::<Dsn>()?;
    let endpoint = dsn.envelope_api_url().to_string();
    cache_system_proxy_route_for_test(&endpoint, "not a valid proxy".to_string());
    let upload = CodexFeedback::new()
        .snapshot(/*session_id*/ None)
        .prepare_feedback_upload_with_dsn(
            FeedbackUploadOptions {
                classification: "bug",
                reason: None,
                tags: None,
                include_logs: false,
                extra_attachments: &[],
                extra_attachment_paths: &[],
                session_source: None,
                logs_override: None,
            },
            &dsn,
        )?;

    let error = upload
        .send(&HttpClientFactory::new(
            OutboundProxyPolicy::RespectSystemProxy,
        ))
        .await
        .expect_err("invalid proxy configuration should fail the feedback upload");
    assert!(
        error
            .to_string()
            .contains("failed to build Sentry feedback HTTP client")
    );

    Ok(())
}
