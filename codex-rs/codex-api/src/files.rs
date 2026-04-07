use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use codex_client::build_reqwest_client_with_custom_ca;
use reqwest::StatusCode;
use serde::Deserialize;
use tokio::fs::File;
use tokio_util::io::ReaderStream;

pub const OPENAI_FILE_URI_PREFIX: &str = "sediment://";
pub const OPENAI_FILE_UPLOAD_LIMIT_BYTES: u64 = 512 * 1024 * 1024;

const OPENAI_FILE_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const OPENAI_FILE_USE_CASE: &str = "codex";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadedOpenAiFile {
    pub file_id: String,
    pub uri: String,
    pub download_url: String,
    pub file_name: String,
    pub file_size_bytes: u64,
    pub mime_type: Option<String>,
    pub path: PathBuf,
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
    #[error("OpenAI file upload for `{file_id}` is not ready yet")]
    UploadNotReady { file_id: String },
    #[error("OpenAI file upload for `{file_id}` failed: {message}")]
    UploadFailed { file_id: String, message: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenAiFileUploadAuth {
    pub access_token: String,
    pub account_id: Option<String>,
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

pub async fn upload_local_file(
    base_url: &str,
    auth: &OpenAiFileUploadAuth,
    path: &Path,
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
    let create_url = format!("{}/files", base_url.trim_end_matches('/'));
    let create_response = authorized_request(auth, reqwest::Method::POST, &create_url)
        .json(&serde_json::json!({
            "file_name": file_name,
            "file_size": metadata.len(),
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

    let finalize_url = format!(
        "{}/files/{}/uploaded",
        base_url.trim_end_matches('/'),
        create_payload.file_id,
    );
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
            url: finalize_url,
            source,
        })?;

    match finalize_payload.status.as_str() {
        "success" => Ok(UploadedOpenAiFile {
            file_id: create_payload.file_id.clone(),
            uri: openai_file_uri(&create_payload.file_id),
            download_url: finalize_payload.download_url.ok_or_else(|| {
                OpenAiFileError::UploadFailed {
                    file_id: create_payload.file_id.clone(),
                    message: "missing download_url".to_string(),
                }
            })?,
            file_name: finalize_payload.file_name.unwrap_or(file_name),
            file_size_bytes: metadata.len(),
            mime_type: finalize_payload.mime_type,
            path: path.to_path_buf(),
        }),
        "retry" => Err(OpenAiFileError::UploadNotReady {
            file_id: create_payload.file_id,
        }),
        _ => Err(OpenAiFileError::UploadFailed {
            file_id: create_payload.file_id,
            message: finalize_payload
                .error_message
                .unwrap_or_else(|| "upload finalization returned an error".to_string()),
        }),
    }
}

fn authorized_request(
    auth: &OpenAiFileUploadAuth,
    method: reqwest::Method,
    url: &str,
) -> reqwest::RequestBuilder {
    let client = build_reqwest_client();
    let mut request = client
        .request(method, url)
        .timeout(OPENAI_FILE_REQUEST_TIMEOUT)
        .bearer_auth(&auth.access_token);
    if let Some(account_id) = &auth.account_id {
        request = request.header("chatgpt-account-id", account_id);
    }
    request
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
    use tempfile::TempDir;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::body_json;
    use wiremock::matchers::header;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    fn chatgpt_auth() -> OpenAiFileUploadAuth {
        OpenAiFileUploadAuth {
            access_token: "token".to_string(),
            account_id: Some("account_id".to_string()),
        }
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
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/backend-api/files/file_123/uploaded"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "download_url": format!("{}/download/file_123", server.uri()),
                "file_name": "hello.txt",
                "mime_type": "text/plain",
                "file_size_bytes": 5
            })))
            .mount(&server)
            .await;

        let base_url = base_url_for(&server);
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("hello.txt");
        tokio::fs::write(&path, b"hello").await.expect("write file");

        let uploaded = upload_local_file(&base_url, &chatgpt_auth(), &path)
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
    }
}
