use std::time::Duration;

use crate::AuthProvider;
use bytes::Bytes;
use codex_http_client::HttpResponse;
use codex_http_client::RouteAwareClientPool;
use codex_http_client::RouteAwareRequestBuilder;
use codex_http_client::RouteAwareRequestError;
use futures::Stream;
use http::Method;
use http::StatusCode;
use http::header::CONTENT_LENGTH;
use serde::Deserialize;
use tokio::time::Instant;
use uuid::Uuid;

pub const OPENAI_FILE_URI_PREFIX: &str = "sediment://";
pub const OPENAI_FILE_UPLOAD_LIMIT_BYTES: u64 = 512 * 1024 * 1024;

const OPENAI_FILE_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const OPENAI_FILE_FINALIZE_TIMEOUT: Duration = Duration::from_secs(30);
const OPENAI_FILE_FINALIZE_RETRY_DELAY: Duration = Duration::from_millis(250);
const OPENAI_FILE_USE_CASE: &str = "codex";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadedOpenAiFile {
    pub file_id: String,
    pub uri: String,
    pub download_url: String,
    pub file_name: String,
    pub file_size_bytes: u64,
    pub mime_type: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum OpenAiFileError {
    #[error(
        "file `{file_name}` is too large: {size_bytes} bytes exceeds the limit of {limit_bytes} bytes"
    )]
    FileTooLarge {
        file_name: String,
        size_bytes: u64,
        limit_bytes: u64,
    },
    #[error("failed to send OpenAI file request to {url}: {source}")]
    Request {
        url: String,
        #[source]
        source: RouteAwareRequestError,
    },
    #[error(
        "OpenAI file blob upload to {host} failed after {elapsed_ms} ms ({error_kind}, azure_client_request_id={azure_client_request_id}): {source}"
    )]
    BlobUploadRequest {
        host: String,
        elapsed_ms: u128,
        error_kind: &'static str,
        azure_client_request_id: String,
        #[source]
        source: RouteAwareRequestError,
    },
    #[error(
        "OpenAI file blob upload to {host} failed with status {status} (azure_client_request_id={azure_client_request_id}, azure_request_id={azure_request_id}, azure_error_code={azure_error_code})"
    )]
    BlobUploadStatus {
        host: String,
        status: StatusCode,
        azure_client_request_id: String,
        azure_request_id: String,
        azure_error_code: String,
    },
    #[error("OpenAI file request to {url} failed with status {status}: {body}")]
    UnexpectedStatus {
        url: String,
        status: StatusCode,
        body: String,
    },
    #[error("failed to parse OpenAI file response from {url}: {source}")]
    Decode {
        url: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("OpenAI file upload for `{file_id}` is not ready yet")]
    UploadNotReady { file_id: String },
    #[error("OpenAI file upload for `{file_id}` failed: {message}")]
    UploadFailed { file_id: String, message: String },
}

#[derive(Deserialize)]
struct CreateFileResponse {
    file_id: String,
    upload_url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
struct DownloadLinkResponse {
    status: String,
    download_url: Option<String>,
    file_name: Option<String>,
    mime_type: Option<String>,
    error_message: Option<String>,
}

pub fn openai_file_uri(file_id: &str) -> String {
    format!("{OPENAI_FILE_URI_PREFIX}{file_id}")
}

pub async fn upload_openai_file(
    base_url: &str,
    auth: &dyn AuthProvider,
    client_pool: &RouteAwareClientPool,
    file_name: String,
    file_size_bytes: u64,
    contents: impl Stream<Item = std::io::Result<Bytes>> + Send + 'static,
) -> Result<UploadedOpenAiFile, OpenAiFileError> {
    if file_size_bytes > OPENAI_FILE_UPLOAD_LIMIT_BYTES {
        return Err(OpenAiFileError::FileTooLarge {
            file_name,
            size_bytes: file_size_bytes,
            limit_bytes: OPENAI_FILE_UPLOAD_LIMIT_BYTES,
        });
    }

    let create_url = format!("{}/files", base_url.trim_end_matches('/'));
    let create_response = authorized_request(client_pool, auth, Method::POST, &create_url)
        .json(&serde_json::json!({
            "file_name": file_name.as_str(),
            "file_size": file_size_bytes,
            "use_case": OPENAI_FILE_USE_CASE,
        }))
        .send()
        .await
        .map_err(|source| OpenAiFileError::Request {
            url: create_url.clone(),
            source,
        })?;
    let create_status = create_response.status();
    let create_body = create_response.text().await.unwrap_or_default();
    if !create_status.is_success() {
        return Err(OpenAiFileError::UnexpectedStatus {
            url: create_url,
            status: create_status,
            body: create_body,
        });
    }
    let create_payload: CreateFileResponse =
        serde_json::from_str(&create_body).map_err(|source| OpenAiFileError::Decode {
            url: create_url.clone(),
            source,
        })?;

    let upload_host = url::Url::parse(&create_payload.upload_url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .unwrap_or_else(|| "unknown-host".to_string());
    let azure_client_request_id = Uuid::new_v4().to_string();
    let upload_started_at = Instant::now();
    let upload_response = client_pool
        .put(&create_payload.upload_url)
        .timeout(OPENAI_FILE_REQUEST_TIMEOUT)
        .header("x-ms-blob-type", "BlockBlob")
        .header("x-ms-client-request-id", &azure_client_request_id)
        .header(CONTENT_LENGTH, file_size_bytes)
        .body_stream(contents)
        .send()
        .await
        .map_err(|source| {
            let elapsed_ms = upload_started_at.elapsed().as_millis();
            let error_kind = if source.is_timeout() {
                "timeout"
            } else if source.is_connect() {
                "connect"
            } else if source.is_body() {
                "body"
            } else if source.is_request() {
                "request"
            } else {
                "other"
            };
            tracing::event!(
                target: "codex_otel.log_only",
                tracing::Level::WARN,
                event.name = "codex.openai_file_blob_upload_failed",
                file_id = %create_payload.file_id,
                host = %upload_host,
                file_size_bytes,
                elapsed_ms,
                error_kind,
                azure_client_request_id,
                "OpenAI file blob upload transport failed"
            );
            OpenAiFileError::BlobUploadRequest {
                host: upload_host.clone(),
                elapsed_ms,
                error_kind,
                azure_client_request_id: azure_client_request_id.clone(),
                source: source.without_url(),
            }
        })?;
    let upload_status = upload_response.status();
    let cloudflare_ray_id = upload_response_header(&upload_response, "cf-ray");
    let azure_request_id = upload_response_header(&upload_response, "x-ms-request-id");
    let azure_error_code = upload_response_header(&upload_response, "x-ms-error-code");
    if !upload_status.is_success() {
        tracing::event!(
            target: "codex_otel.log_only",
            tracing::Level::WARN,
            event.name = "codex.openai_file_blob_upload_failed",
            file_id = %create_payload.file_id,
            host = %upload_host,
            file_size_bytes,
            elapsed_ms = upload_started_at.elapsed().as_millis(),
            status = %upload_status,
            cloudflare_ray_id,
            azure_client_request_id,
            azure_request_id,
            azure_error_code,
            "OpenAI file blob upload failed"
        );
        return Err(OpenAiFileError::BlobUploadStatus {
            host: upload_host,
            status: upload_status,
            azure_client_request_id,
            azure_request_id,
            azure_error_code,
        });
    }

    let finalize_url = format!(
        "{}/files/{}/uploaded",
        base_url.trim_end_matches('/'),
        create_payload.file_id,
    );
    let finalize_started_at = Instant::now();
    loop {
        let finalize_response = authorized_request(client_pool, auth, Method::POST, &finalize_url)
            .json(&serde_json::json!({}))
            .send()
            .await
            .map_err(|source| OpenAiFileError::Request {
                url: finalize_url.clone(),
                source,
            })?;
        let finalize_status = finalize_response.status();
        let finalize_body = finalize_response.text().await.unwrap_or_default();
        if !finalize_status.is_success() {
            return Err(OpenAiFileError::UnexpectedStatus {
                url: finalize_url.clone(),
                status: finalize_status,
                body: finalize_body,
            });
        }
        let finalize_payload: DownloadLinkResponse =
            serde_json::from_str(&finalize_body).map_err(|source| OpenAiFileError::Decode {
                url: finalize_url.clone(),
                source,
            })?;

        match finalize_payload.status.as_str() {
            "success" => {
                return Ok(UploadedOpenAiFile {
                    file_id: create_payload.file_id.clone(),
                    uri: openai_file_uri(&create_payload.file_id),
                    download_url: finalize_payload.download_url.ok_or_else(|| {
                        OpenAiFileError::UploadFailed {
                            file_id: create_payload.file_id.clone(),
                            message: "missing download_url".to_string(),
                        }
                    })?,
                    file_name: finalize_payload.file_name.unwrap_or(file_name),
                    file_size_bytes,
                    mime_type: finalize_payload.mime_type,
                });
            }
            "retry" => {
                if finalize_started_at.elapsed() >= OPENAI_FILE_FINALIZE_TIMEOUT {
                    return Err(OpenAiFileError::UploadNotReady {
                        file_id: create_payload.file_id,
                    });
                }
                tokio::time::sleep(OPENAI_FILE_FINALIZE_RETRY_DELAY).await;
            }
            _ => {
                return Err(OpenAiFileError::UploadFailed {
                    file_id: create_payload.file_id,
                    message: finalize_payload
                        .error_message
                        .unwrap_or_else(|| "upload finalization returned an error".to_string()),
                });
            }
        }
    }
}

fn authorized_request(
    client_pool: &RouteAwareClientPool,
    auth: &dyn AuthProvider,
    method: Method,
    url: &str,
) -> RouteAwareRequestBuilder {
    let mut headers = http::HeaderMap::new();
    auth.add_auth_headers(&mut headers);

    client_pool
        .request(method, url)
        .timeout(OPENAI_FILE_REQUEST_TIMEOUT)
        .headers(headers)
}

fn upload_response_header(response: &HttpResponse, header: &str) -> String {
    response
        .headers()
        .get(header)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("missing")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_http_client::ClientRouteClass;
    use codex_http_client::HttpClientFactory;
    use codex_http_client::OutboundProxyPolicy;
    use http::header::HeaderValue;
    use pretty_assertions::assert_eq;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::Request;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::body_json;
    use wiremock::matchers::header;
    use wiremock::matchers::header_regex;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    #[derive(Clone, Copy)]
    struct ChatGptTestAuth;

    fn default_http_client_pool() -> RouteAwareClientPool {
        RouteAwareClientPool::new_without_request_logging(
            HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
            ClientRouteClass::Api,
        )
        .with_legacy_custom_ca_fallback()
    }

    impl AuthProvider for ChatGptTestAuth {
        fn add_auth_headers(&self, headers: &mut http::HeaderMap) {
            headers.insert(
                http::header::AUTHORIZATION,
                HeaderValue::from_static("Bearer token"),
            );
            headers.insert("ChatGPT-Account-ID", HeaderValue::from_static("account_id"));
        }
    }

    fn chatgpt_auth() -> ChatGptTestAuth {
        ChatGptTestAuth
    }

    fn base_url_for(server: &MockServer) -> String {
        format!("{}/backend-api", server.uri())
    }

    #[tokio::test]
    async fn upload_openai_file_returns_canonical_uri() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/backend-api/files"))
            .and(header("chatgpt-account-id", "account_id"))
            .and(body_json(serde_json::json!({
                "file_name": "hello.txt",
                "file_size": 5,
                "use_case": "codex",
            })))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"file_id": "file_123", "upload_url": format!("{}/upload/file_123", server.uri())})),
            )
            .mount(&server)
            .await;
        Mock::given(method("PUT"))
            .and(path("/upload/file_123"))
            .and(header("content-length", "5"))
            .and(header_regex("x-ms-client-request-id", "^[0-9a-f-]{36}$"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let finalize_attempts = Arc::new(AtomicUsize::new(0));
        let finalize_attempts_responder = Arc::clone(&finalize_attempts);
        let download_url = format!("{}/download/file_123", server.uri());
        Mock::given(method("POST"))
            .and(path("/backend-api/files/file_123/uploaded"))
            .respond_with(move |_request: &Request| {
                if finalize_attempts_responder.fetch_add(1, Ordering::SeqCst) == 0 {
                    return ResponseTemplate::new(200).set_body_json(serde_json::json!({
                        "status": "retry"
                    }));
                }

                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "status": "success",
                    "download_url": download_url,
                    "file_name": "hello.txt",
                    "mime_type": "text/plain",
                    "file_size_bytes": 5
                }))
            })
            .mount(&server)
            .await;

        let base_url = base_url_for(&server);
        let contents =
            futures::stream::iter([Ok::<_, std::io::Error>(Bytes::from_static(b"hello"))]);
        let uploaded = upload_openai_file(
            &base_url,
            &chatgpt_auth(),
            &default_http_client_pool(),
            "hello.txt".to_string(),
            /*file_size_bytes*/ 5,
            contents,
        )
        .await
        .expect("upload succeeds");

        assert_eq!(uploaded.file_id, "file_123");
        assert_eq!(uploaded.uri, "sediment://file_123");
        assert_eq!(
            uploaded.download_url,
            format!("{}/download/file_123", server.uri())
        );
        assert_eq!(uploaded.file_name, "hello.txt");
        assert_eq!(uploaded.mime_type, Some("text/plain".to_string()));
        assert_eq!(finalize_attempts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn upload_openai_file_reuses_client_pool_across_uploads() {
        let server = MockServer::start().await;
        let files = [
            ("first.txt", "file_1", &b"first"[..]),
            ("second.txt", "file_2", &b"second"[..]),
        ];

        for (file_name, file_id, contents) in files {
            Mock::given(method("POST"))
                .and(path("/backend-api/files"))
                .and(header("chatgpt-account-id", "account_id"))
                .and(body_json(serde_json::json!({
                    "file_name": file_name,
                    "file_size": contents.len(),
                    "use_case": "codex",
                })))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "file_id": file_id,
                    "upload_url": format!("{}/upload/{file_id}", server.uri()),
                })))
                .expect(1)
                .mount(&server)
                .await;
            Mock::given(method("PUT"))
                .and(path(format!("/upload/{file_id}")))
                .and(header("content-length", contents.len().to_string()))
                .respond_with(ResponseTemplate::new(200))
                .expect(1)
                .mount(&server)
                .await;
            Mock::given(method("POST"))
                .and(path(format!("/backend-api/files/{file_id}/uploaded")))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "status": "success",
                    "download_url": format!("{}/download/{file_id}", server.uri()),
                    "file_name": file_name,
                    "mime_type": "text/plain",
                    "file_size_bytes": contents.len(),
                })))
                .expect(1)
                .mount(&server)
                .await;
        }

        let client_pool = default_http_client_pool();
        let mut uploaded_files = Vec::new();
        for (file_name, _, contents) in files {
            let contents_stream =
                futures::stream::iter([Ok::<_, std::io::Error>(Bytes::copy_from_slice(contents))]);
            let uploaded = upload_openai_file(
                &base_url_for(&server),
                &chatgpt_auth(),
                &client_pool,
                file_name.to_string(),
                u64::try_from(contents.len()).expect("file size should fit in a u64"),
                contents_stream,
            )
            .await
            .expect("upload succeeds with the shared client pool");
            uploaded_files.push(uploaded);
        }

        assert_eq!(
            uploaded_files,
            vec![
                UploadedOpenAiFile {
                    file_id: "file_1".to_string(),
                    uri: "sediment://file_1".to_string(),
                    download_url: format!("{}/download/file_1", server.uri()),
                    file_name: "first.txt".to_string(),
                    file_size_bytes: 5,
                    mime_type: Some("text/plain".to_string()),
                },
                UploadedOpenAiFile {
                    file_id: "file_2".to_string(),
                    uri: "sediment://file_2".to_string(),
                    download_url: format!("{}/download/file_2", server.uri()),
                    file_name: "second.txt".to_string(),
                    file_size_bytes: 6,
                    mime_type: Some("text/plain".to_string()),
                },
            ]
        );
        server.verify().await;
    }

    #[tokio::test]
    async fn upload_openai_file_reports_blob_response_diagnostics_without_sas() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/backend-api/files"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "file_id": "file_123",
                "upload_url": format!("{}/upload/file_123?sig=secret", server.uri()),
            })))
            .mount(&server)
            .await;
        Mock::given(method("PUT"))
            .and(path("/upload/file_123"))
            .respond_with(
                ResponseTemplate::new(500)
                    .insert_header("x-ms-request-id", "azure-request")
                    .insert_header("x-ms-error-code", "ServerBusy")
                    .set_body_string("try again"),
            )
            .mount(&server)
            .await;

        let error = upload_openai_file(
            &base_url_for(&server),
            &chatgpt_auth(),
            &default_http_client_pool(),
            "hello.txt".to_string(),
            /*file_size_bytes*/ 5,
            futures::stream::iter([Ok::<_, std::io::Error>(Bytes::from_static(b"hello"))]),
        )
        .await
        .expect_err("blob response failure should be returned");

        let message = error.to_string();
        assert!(message.contains("failed with status 500"));
        assert!(message.contains("azure_client_request_id="));
        assert!(message.contains("azure_request_id=azure-request"));
        assert!(message.contains("azure_error_code=ServerBusy"));
        assert!(!message.contains("try again"));
        assert!(!message.contains("sig=secret"));
    }

    #[tokio::test]
    async fn upload_openai_file_reports_blob_transport_diagnostics_without_sas() {
        let upload_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind upload address");
        let upload_address = upload_listener.local_addr().expect("upload address");
        drop(upload_listener);
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/backend-api/files"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "file_id": "file_123",
                "upload_url": format!("http://{upload_address}/upload?sig=secret"),
            })))
            .mount(&server)
            .await;

        let error = upload_openai_file(
            &base_url_for(&server),
            &chatgpt_auth(),
            &default_http_client_pool(),
            "hello.txt".to_string(),
            /*file_size_bytes*/ 5,
            futures::stream::iter([Ok::<_, std::io::Error>(Bytes::from_static(b"hello"))]),
        )
        .await
        .expect_err("blob transport failure should be returned");

        let message = error.to_string();
        assert!(message.contains("failed after"));
        assert!(message.contains("(connect,"), "{message}");
        assert!(message.contains("azure_client_request_id="));
        assert!(!message.contains("sig=secret"));
    }
}
