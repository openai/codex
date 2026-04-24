use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use crate::AuthProvider;
use codex_client::build_reqwest_client_with_custom_ca;
use reqwest::StatusCode;
use reqwest::header::CONTENT_LENGTH;
use serde::Deserialize;
use tokio::fs::File;
use tokio::time::Instant;
use tokio_util::io::ReaderStream;
use url::Url;

pub const OPENAI_FILE_URI_PREFIX: &str = "sediment://";
pub const OPENAI_FILE_UPLOAD_LIMIT_BYTES: u64 = 512 * 1024 * 1024;

const OPENAI_FILE_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const OPENAI_FILE_FINALIZE_TIMEOUT: Duration = Duration::from_secs(30);
const OPENAI_FILE_FINALIZE_RETRY_DELAY: Duration = Duration::from_millis(250);
const OPENAI_FILE_USE_CASE: &str = "codex";
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OpenAiFileUploadOptions {
    pub store_in_library: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadedOpenAiFile {
    pub file_id: String,
    pub uri: String,
    pub download_url: Option<String>,
    pub file_name: String,
    pub file_size_bytes: u64,
    pub mime_type: Option<String>,
    pub path: PathBuf,
    pub library_file_id: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum OpenAiFileError {
    #[error("path `{path}` does not exist")]
    MissingPath { path: PathBuf },
    #[error("path `{path}` is not a file")]
    NotAFile { path: PathBuf },
    #[error("path `{path}` cannot be read: {source}")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(
        "file `{path}` is too large: {size_bytes} bytes exceeds the limit of {limit_bytes} bytes"
    )]
    FileTooLarge {
        path: PathBuf,
        size_bytes: u64,
        limit_bytes: u64,
    },
    #[error("failed to send OpenAI file request to {url}: {source}")]
    Request {
        url: String,
        #[source]
        source: reqwest::Error,
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
    #[error("failed to resolve OpenAI file URL `{url}`: {source}")]
    InvalidUrl {
        url: String,
        #[source]
        source: url::ParseError,
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

#[derive(Deserialize)]
struct ProcessUploadStreamStatus {
    event: Option<String>,
    message: Option<String>,
    #[serde(default)]
    extra: Option<ProcessUploadStreamExtra>,
}

#[derive(Debug, Default, Deserialize)]
struct ProcessUploadStreamExtra {
    #[serde(alias = "metadata_object_id", alias = "library_file_id")]
    library_file_id: Option<String>,
    #[serde(alias = "library_file_name")]
    file_name: Option<String>,
    mime_type: Option<String>,
}

pub fn openai_file_uri(file_id: &str) -> String {
    format!("{OPENAI_FILE_URI_PREFIX}{file_id}")
}

pub async fn download_openai_file(
    base_url: &str,
    auth: &impl AuthProvider,
    download_url: &str,
) -> Result<Vec<u8>, OpenAiFileError> {
    let resolved_url = resolve_openai_file_download_url(base_url, download_url)?;
    let request_builder = if should_attach_auth_to_openai_file_url(&resolved_url, base_url) {
        authorized_request(auth, reqwest::Method::GET, resolved_url.as_str())
    } else {
        build_reqwest_client()
            .request(reqwest::Method::GET, resolved_url.as_str())
            .timeout(OPENAI_FILE_REQUEST_TIMEOUT)
    };
    let response = request_builder
        .send()
        .await
        .map_err(|source| OpenAiFileError::Request {
            url: resolved_url.to_string(),
            source,
        })?;
    response_bytes(resolved_url.as_str(), response).await
}

pub async fn upload_local_file(
    base_url: &str,
    auth: &dyn AuthProvider,
    path: &Path,
    options: &OpenAiFileUploadOptions,
) -> Result<UploadedOpenAiFile, OpenAiFileError> {
    let metadata = tokio::fs::metadata(path)
        .await
        .map_err(|source| match source.kind() {
            std::io::ErrorKind::NotFound => OpenAiFileError::MissingPath {
                path: path.to_path_buf(),
            },
            _ => OpenAiFileError::ReadFile {
                path: path.to_path_buf(),
                source,
            },
        })?;
    if !metadata.is_file() {
        return Err(OpenAiFileError::NotAFile {
            path: path.to_path_buf(),
        });
    }
    if metadata.len() > OPENAI_FILE_UPLOAD_LIMIT_BYTES {
        return Err(OpenAiFileError::FileTooLarge {
            path: path.to_path_buf(),
            size_bytes: metadata.len(),
            limit_bytes: OPENAI_FILE_UPLOAD_LIMIT_BYTES,
        });
    }

    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("file")
        .to_string();
    let base_url = base_url.trim_end_matches('/');
    let mut create_request = serde_json::json!({
        "file_name": file_name,
        "file_size": metadata.len(),
        "use_case": OPENAI_FILE_USE_CASE,
    });
    if options.store_in_library {
        create_request["store_in_library"] = serde_json::json!(true);
    }
    let create_url = format!("{base_url}/files");
    let create_response = authorized_request(auth, reqwest::Method::POST, &create_url)
        .json(&create_request)
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

    let upload_file = File::open(path)
        .await
        .map_err(|source| OpenAiFileError::ReadFile {
            path: path.to_path_buf(),
            source,
        })?;
    let upload_response = build_reqwest_client()
        .put(&create_payload.upload_url)
        .timeout(OPENAI_FILE_REQUEST_TIMEOUT)
        .header("x-ms-blob-type", "BlockBlob")
        .header(CONTENT_LENGTH, metadata.len())
        .body(reqwest::Body::wrap_stream(ReaderStream::new(upload_file)))
        .send()
        .await
        .map_err(|source| OpenAiFileError::Request {
            url: create_payload.upload_url.clone(),
            source,
        })?;
    let upload_status = upload_response.status();
    let upload_body = upload_response.text().await.unwrap_or_default();
    if !upload_status.is_success() {
        return Err(OpenAiFileError::UnexpectedStatus {
            url: create_payload.upload_url.clone(),
            status: upload_status,
            body: upload_body,
        });
    }

    if options.store_in_library {
        let processed =
            process_upload_stream(auth, base_url, &create_payload.file_id, &file_name).await?;
        let library_file_id =
            processed
                .library_file_id
                .ok_or_else(|| OpenAiFileError::UploadFailed {
                    file_id: create_payload.file_id.clone(),
                    message: "upload completed without creating a library_file_id".to_string(),
                })?;
        return Ok(UploadedOpenAiFile {
            file_id: create_payload.file_id.clone(),
            uri: openai_file_uri(&create_payload.file_id),
            download_url: None,
            file_name: processed.file_name.unwrap_or(file_name),
            file_size_bytes: metadata.len(),
            mime_type: processed.mime_type,
            path: path.to_path_buf(),
            library_file_id: Some(library_file_id),
        });
    }

    let finalize_url = format!("{base_url}/files/{}/uploaded", create_payload.file_id);
    let finalize_started_at = Instant::now();
    loop {
        let finalize_response = authorized_request(auth, reqwest::Method::POST, &finalize_url)
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
                    download_url: Some(finalize_payload.download_url.ok_or_else(|| {
                        OpenAiFileError::UploadFailed {
                            file_id: create_payload.file_id.clone(),
                            message: "missing download_url".to_string(),
                        }
                    })?),
                    file_name: finalize_payload.file_name.unwrap_or(file_name),
                    file_size_bytes: metadata.len(),
                    mime_type: finalize_payload.mime_type,
                    path: path.to_path_buf(),
                    library_file_id: None,
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

async fn process_upload_stream(
    auth: &dyn AuthProvider,
    base_url: &str,
    file_id: &str,
    file_name: &str,
) -> Result<ProcessUploadStreamExtra, OpenAiFileError> {
    let process_url = format!("{base_url}/files/process_upload_stream");
    let process_response = authorized_request(auth, reqwest::Method::POST, &process_url)
        .json(&serde_json::json!({
            "file_id": file_id,
            "file_name": file_name,
            "use_case": OPENAI_FILE_USE_CASE,
            "index_for_retrieval": false,
            "entry_surface": OPENAI_FILE_USE_CASE,
            "metadata": {
                "store_in_library": true,
            },
        }))
        .send()
        .await
        .map_err(|source| OpenAiFileError::Request {
            url: process_url.clone(),
            source,
        })?;
    let process_body = response_text(&process_url, process_response).await?;

    let mut result = ProcessUploadStreamExtra::default();
    for line in process_body.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let ProcessUploadStreamStatus {
            event,
            message,
            extra,
        } = serde_json::from_str(line).map_err(|source| OpenAiFileError::Decode {
            url: process_url.clone(),
            source,
        })?;
        let extra = extra.unwrap_or_default();

        if let Some(event) = event.as_deref()
            && is_process_upload_stream_error_event(event)
        {
            return Err(OpenAiFileError::UploadFailed {
                file_id: file_id.to_string(),
                message: message
                    .filter(|message| !message.is_empty())
                    .unwrap_or_else(|| format!("process_upload_stream returned {event}")),
            });
        }

        if result.library_file_id.is_none() {
            result.library_file_id = non_empty_string(extra.library_file_id);
        }
        if result.file_name.is_none() {
            result.file_name = non_empty_string(extra.file_name);
        }
        if result.mime_type.is_none() {
            result.mime_type = non_empty_string(extra.mime_type);
        }
    }

    Ok(result)
}

fn non_empty_string(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.is_empty())
}

fn is_process_upload_stream_error_event(event: &str) -> bool {
    let event_tail = event.rsplit(['.', '_']).next().unwrap_or(event);
    matches!(event_tail, "error" | "cancelled" | "unknown")
}

fn authorized_request(
    auth: &dyn AuthProvider,
    method: reqwest::Method,
    url: &str,
) -> reqwest::RequestBuilder {
    let mut headers = http::HeaderMap::new();
    auth.add_auth_headers(&mut headers);

    let client = build_reqwest_client();
    client
        .request(method, url)
        .timeout(OPENAI_FILE_REQUEST_TIMEOUT)
        .headers(headers)
}

async fn response_text(url: &str, response: reqwest::Response) -> Result<String, OpenAiFileError> {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(OpenAiFileError::UnexpectedStatus {
            url: url.to_string(),
            status,
            body,
        });
    }
    Ok(body)
}

async fn response_bytes(
    url: &str,
    response: reqwest::Response,
) -> Result<Vec<u8>, OpenAiFileError> {
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(OpenAiFileError::UnexpectedStatus {
            url: url.to_string(),
            status,
            body,
        });
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|source| OpenAiFileError::Request {
            url: url.to_string(),
            source,
        })?;
    Ok(bytes.to_vec())
}

fn resolve_openai_file_download_url(
    base_url: &str,
    download_url: &str,
) -> Result<Url, OpenAiFileError> {
    match Url::parse(download_url) {
        Ok(url) => Ok(url),
        Err(url::ParseError::RelativeUrlWithoutBase) => {
            let normalized_base_url = if base_url.ends_with('/') {
                base_url.to_string()
            } else {
                format!("{base_url}/")
            };
            let base =
                Url::parse(&normalized_base_url).map_err(|source| OpenAiFileError::InvalidUrl {
                    url: normalized_base_url.clone(),
                    source,
                })?;
            base.join(download_url)
                .map_err(|source| OpenAiFileError::InvalidUrl {
                    url: download_url.to_string(),
                    source,
                })
        }
        Err(source) => Err(OpenAiFileError::InvalidUrl {
            url: download_url.to_string(),
            source,
        }),
    }
}

fn should_attach_auth_to_openai_file_url(download_url: &Url, base_url: &str) -> bool {
    let Ok(base_url) = Url::parse(base_url) else {
        return false;
    };
    match (download_url.host_str(), base_url.host_str()) {
        (Some(download_host), Some(base_host)) => download_host.eq_ignore_ascii_case(base_host),
        _ => false,
    }
}

fn build_reqwest_client() -> reqwest::Client {
    build_reqwest_client_with_custom_ca(reqwest::Client::builder()).unwrap_or_else(|error| {
        tracing::warn!(error = %error, "failed to build OpenAI file upload client");
        reqwest::Client::new()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use reqwest::header::HeaderValue;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use tempfile::TempDir;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::Request;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::body_json;
    use wiremock::matchers::header;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    #[derive(Clone, Copy)]
    struct ChatGptTestAuth;

    impl AuthProvider for ChatGptTestAuth {
        fn add_auth_headers(&self, headers: &mut reqwest::header::HeaderMap) {
            headers.insert(
                reqwest::header::AUTHORIZATION,
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
    async fn upload_local_file_returns_canonical_uri() {
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
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("hello.txt");
        tokio::fs::write(&path, b"hello").await.expect("write file");

        let uploaded = upload_local_file(
            &base_url,
            &chatgpt_auth(),
            &path,
            &OpenAiFileUploadOptions::default(),
        )
        .await
        .expect("upload succeeds");

        assert_eq!(uploaded.file_id, "file_123");
        assert_eq!(uploaded.uri, "sediment://file_123");
        assert_eq!(
            uploaded.download_url,
            Some(format!("{}/download/file_123", server.uri()))
        );
        assert_eq!(uploaded.file_name, "hello.txt");
        assert_eq!(uploaded.mime_type, Some("text/plain".to_string()));
        assert_eq!(finalize_attempts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn upload_local_file_stores_library_file_with_process_upload_stream() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/backend-api/files"))
            .and(header("chatgpt-account-id", "account_id"))
            .and(body_json(serde_json::json!({
                "file_name": "hello.txt",
                "file_size": 5,
                "use_case": "codex",
                "store_in_library": true,
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
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/backend-api/files/process_upload_stream"))
            .and(body_json(serde_json::json!({
                "file_id": "file_123",
                "file_name": "hello.txt",
                "use_case": "codex",
                "index_for_retrieval": false,
                "entry_surface": "codex",
                "metadata": {
                    "store_in_library": true,
                },
            })))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(
                concat!(
                    "{\"file_id\":\"file_123\",\"event\":\"indexing.completed\",\"message\":\"\",",
                    "\"extra\":{\"metadata_object_id\":\"library_123\",",
                    "\"library_file_name\":\"hello.txt\",\"mime_type\":\"text/plain\"}}\n",
                    "{\"file_id\":\"file_123\",\"event\":\"completed\",",
                    "\"message\":\"Succeeded processing file file_123\",",
                    "\"progress\":100,\"extra\":null}\n",
                )
                .as_bytes()
                .to_vec(),
            ))
            .mount(&server)
            .await;

        let base_url = base_url_for(&server);
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("hello.txt");
        tokio::fs::write(&path, b"hello").await.expect("write file");

        let uploaded = upload_local_file(
            &base_url,
            &chatgpt_auth(),
            &path,
            &OpenAiFileUploadOptions {
                store_in_library: true,
            },
        )
        .await
        .expect("upload succeeds");

        assert_eq!(uploaded.file_id, "file_123");
        assert_eq!(uploaded.uri, "sediment://file_123");
        assert_eq!(uploaded.download_url, None);
        assert_eq!(uploaded.file_name, "hello.txt");
        assert_eq!(uploaded.mime_type, Some("text/plain".to_string()));
        assert_eq!(uploaded.library_file_id, Some("library_123".to_string()));
    }
}
