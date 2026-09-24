//! Upload retries replay bytes without allocating or finalizing another file record.

use super::tests::base_url_for;
use super::tests::chatgpt_auth;
use super::tests::default_http_client_pool;
use super::*;
use pretty_assertions::assert_eq;
use std::collections::HashSet;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::Request;
use wiremock::Respond;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_bytes;
use wiremock::matchers::method;
use wiremock::matchers::path;

async fn mock_upload(
    server: &MockServer,
    response: impl Respond + 'static,
    puts: u64,
    finalizes: u64,
) {
    Mock::given(method("POST"))
        .and(path("/backend-api/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "file_id": "file_123",
            "upload_url": format!("{}/upload?sig=secret", server.uri()),
        })))
        .expect(1)
        .mount(server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/upload"))
        .and(body_bytes(b"hello"))
        .respond_with(response)
        .expect(puts)
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/backend-api/files/file_123/uploaded"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "success", "download_url": "https://example.com/file"
        })))
        .expect(finalizes)
        .mount(server)
        .await;
}

#[tokio::test]
async fn retries_transient_upload_statuses_within_budget() {
    for (statuses, azure_code, retry_after) in [
        (vec![502, 200], None, None),
        (vec![503, 200], Some("ServerBusy"), None),
        (vec![504, 200], None, None),
        (vec![502, 504, 503, 200], None, None),
        (vec![502; 5], None, None),
        (vec![503; 5], Some("ServerBusy"), None),
        (vec![504; 5], None, None),
        (vec![403], Some("AuthenticationFailed"), None),
        (vec![409], None, None),
        (vec![500], None, None),
        (vec![502], None, Some("301")),
        (vec![503], Some("ServerBusy"), Some("301")),
        (vec![504], None, Some("301")),
    ] {
        let server = MockServer::start().await;
        let attempts = statuses.len();
        let succeeds = statuses.last() == Some(&200);
        let put_count = AtomicUsize::new(0);
        mock_upload(
            &server,
            move |_request: &Request| {
                let index = put_count.fetch_add(1, Ordering::SeqCst);
                let response = ResponseTemplate::new(statuses[index])
                    .insert_header("x-ms-request-id", format!("request-{index}"));
                let response = match retry_after {
                    Some(delay) => response.insert_header("retry-after", delay),
                    None => response.insert_header("x-ms-retry-after-ms", "0"),
                };
                match azure_code {
                    Some(code) => response.insert_header("x-ms-error-code", code),
                    None => response,
                }
            },
            attempts as u64,
            u64::from(succeeds),
        )
        .await;
        let mut opens = 0;

        let result = upload_openai_file(
            &base_url_for(&server),
            &chatgpt_auth(),
            &default_http_client_pool(),
            "hello.txt".into(),
            /*file_size_bytes*/ 5,
            || {
                opens += 1;
                async { Ok(futures::stream::iter([Ok(Bytes::from_static(b"hello"))])) }
            },
            /*hosted_upload*/ None,
        )
        .await;

        assert_eq!((result.is_ok(), opens), (succeeds, attempts));
        if let Err(error) = result {
            let message = error.to_string();
            assert!(message.contains(&format!("azure_request_id=request-{}", attempts - 1)));
            assert!(message.contains(azure_code.unwrap_or("missing")));
            assert!(!format!("{error:?}").contains("sig=secret"));
        }
        let requests = server.received_requests().await.expect("requests");
        let mut request_ids = HashSet::new();
        for request in requests.iter().filter(|r| r.method == Method::PUT) {
            let id = request.headers["x-ms-client-request-id"]
                .to_str()
                .expect("id");
            assert!(request_ids.insert(id.to_owned()));
        }
        assert_eq!(request_ids.len(), attempts);
        server.verify().await;
    }
}

#[tokio::test]
async fn reopens_after_a_stream_closure_and_sends_the_complete_body() {
    let server = MockServer::start().await;
    mock_upload(
        &server,
        ResponseTemplate::new(200),
        /*puts*/ 1,
        /*finalizes*/ 1,
    )
    .await;
    let mut opens = 0;

    let result = upload_openai_file(
        &base_url_for(&server),
        &chatgpt_auth(),
        &default_http_client_pool(),
        "hello.txt".into(),
        /*file_size_bytes*/ 5,
        || {
            opens += 1;
            let chunks = if opens == 1 {
                vec![
                    Ok(Bytes::from_static(b"he")),
                    Err(std::io::Error::new(
                        std::io::ErrorKind::ConnectionReset,
                        "stream closed",
                    )),
                ]
            } else {
                vec![Ok(Bytes::from_static(b"hello"))]
            };
            async move { Ok(futures::stream::iter(chunks)) }
        },
        /*hosted_upload*/ None,
    )
    .await
    .expect("replayed upload");

    assert_eq!((result.file_id.as_str(), opens), ("file_123", 2));
    server.verify().await;
}

#[tokio::test]
async fn reopening_cannot_extend_the_shared_upload_deadline() {
    let server = MockServer::start().await;
    mock_upload(
        &server,
        ResponseTemplate::new(503).insert_header("x-ms-retry-after-ms", "0"),
        /*puts*/ 1,
        /*finalizes*/ 0,
    )
    .await;
    let mut opens = 0;
    let result = upload_openai_file(
        &base_url_for(&server),
        &chatgpt_auth(),
        &default_http_client_pool(),
        "hello.txt".into(),
        /*file_size_bytes*/ 5,
        || {
            opens += 1;
            let attempt = opens;
            async move {
                if attempt == 2 {
                    tokio::time::pause();
                    tokio::time::sleep(OPENAI_FILE_BLOB_UPLOAD_TIMEOUT).await;
                }
                Ok(futures::stream::iter([Ok(Bytes::from_static(b"hello"))]))
            }
        },
        /*hosted_upload*/ None,
    )
    .await;

    assert!(matches!(
        result,
        Err(OpenAiFileError::BlobUploadRequest {
            error_kind: "timeout",
            ..
        })
    ));
    assert_eq!(opens, 2);
    server.verify().await;
}

#[test]
fn server_retry_delays_accept_milliseconds_seconds_and_http_dates() {
    for (milliseconds, retry_after, expected) in [
        (Some("250"), Some("5"), Some(Duration::from_millis(250))),
        (Some("invalid"), Some("5"), Some(Duration::from_secs(5))),
        (
            None,
            Some("Wed, 21 Oct 2015 07:28:00 GMT"),
            Some(Duration::ZERO),
        ),
        (None, Some("invalid"), None),
    ] {
        let mut headers = http::HeaderMap::new();
        if let Some(value) = milliseconds {
            headers.insert("x-ms-retry-after-ms", value.parse().expect("header"));
        }
        if let Some(value) = retry_after {
            headers.insert("retry-after", value.parse().expect("header"));
        }
        assert_eq!(blob_retry_after(&headers), expected);
    }
}
