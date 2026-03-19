use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use reqwest::StatusCode;
use serde::Deserialize;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;
use uuid::Uuid;

use crate::auth::CodexAuth;
use crate::config::Config;
use crate::default_client::build_reqwest_client;

pub(crate) const OPENAI_FILE_URI_PREFIX: &str = "sediment://";
pub(crate) const META_OPENAI_FILE_OUTPUTS: &str = "openai/fileOutputs";
pub(crate) const META_OPENAI_FILE_PARAMS: &str = "openai/fileParams";
pub(crate) const OPENAI_FILE_UPLOAD_LIMIT_BYTES: u64 = 512 * 1024 * 1024;
pub(crate) const OPENAI_FILE_DOWNLOAD_LIMIT_BYTES: u64 = 512 * 1024 * 1024;
pub(crate) const OPENAI_FILE_AUTO_DOWNLOAD_LIMIT_BYTES: u64 = 128 * 1024 * 1024;
pub(crate) const OPENAI_FILE_AUTO_DOWNLOAD_BUDGET_BYTES: u64 = 512 * 1024 * 1024;

const OPENAI_FILE_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const OPENAI_FILE_USE_CASE: &str = "codex";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedOpenAiFile {
    pub(crate) file_id: String,
    pub(crate) uri: String,
    pub(crate) download_url: String,
    pub(crate) file_name: Option<String>,
    pub(crate) mime_type: Option<String>,
    pub(crate) file_size_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UploadedOpenAiFile {
    pub(crate) file_id: String,
    pub(crate) uri: String,
    pub(crate) file_name: String,
    pub(crate) file_size_bytes: u64,
    pub(crate) mime_type: Option<String>,
    pub(crate) path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DownloadedOpenAiFile {
    pub(crate) file_id: String,
    pub(crate) uri: String,
    pub(crate) file_name: String,
    pub(crate) mime_type: Option<String>,
    pub(crate) destination_path: PathBuf,
    pub(crate) bytes_written: u64,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum OpenAiFileError {
    #[error("chatgpt authentication is required to use OpenAI file storage")]
    AuthRequired,
    #[error(
        "chatgpt authentication is required to use OpenAI file storage; api key auth is not supported"
    )]
    UnsupportedAuthMode,
    #[error("failed to read chatgpt auth token for OpenAI file storage: {0}")]
    AuthToken(#[source] std::io::Error),
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
    #[error(
        "file `{file_id}` is too large to download automatically: {size_bytes} bytes exceeds the limit of {limit_bytes} bytes"
    )]
    RemoteFileTooLarge {
        file_id: String,
        size_bytes: u64,
        limit_bytes: u64,
    },
    #[error("invalid OpenAI file reference `{reference}`")]
    InvalidFileReference { reference: String },
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
    #[error("failed to create temp directory `{path}`: {source}")]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write downloaded file to `{path}`: {source}")]
    WriteFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
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
    file_size_bytes: Option<u64>,
    mime_type: Option<String>,
    error_message: Option<String>,
}

pub(crate) fn is_openai_file_uri(value: &str) -> bool {
    value.starts_with(OPENAI_FILE_URI_PREFIX)
}

pub(crate) fn openai_file_uri(file_id: &str) -> String {
    format!("{OPENAI_FILE_URI_PREFIX}{file_id}")
}

pub(crate) fn parse_openai_file_id(reference: &str) -> Option<&str> {
    if let Some(file_id) = reference.strip_prefix(OPENAI_FILE_URI_PREFIX) {
        return (!file_id.is_empty()).then_some(file_id);
    }

    reference
        .strip_prefix("file_")
        .or_else(|| reference.strip_prefix("file-"))
        .map(|_| reference)
}

pub(crate) async fn resolve_openai_file(
    config: &Config,
    auth: Option<&CodexAuth>,
    reference: &str,
) -> Result<ResolvedOpenAiFile, OpenAiFileError> {
    let auth = ensure_chatgpt_auth(auth)?;
    let file_id =
        parse_openai_file_id(reference).ok_or_else(|| OpenAiFileError::InvalidFileReference {
            reference: reference.to_string(),
        })?;

    let url = format!(
        "{}/files/download/{}",
        config.chatgpt_base_url.trim_end_matches('/'),
        file_id,
    );
    let response = authorized_request(auth, reqwest::Method::GET, &url)?
        .send()
        .await
        .map_err(|source| OpenAiFileError::Request {
            url: url.clone(),
            source,
        })?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(OpenAiFileError::UnexpectedStatus { url, status, body });
    }

    let payload: DownloadLinkResponse =
        serde_json::from_str(&body).map_err(|source| OpenAiFileError::Decode {
            url: url.clone(),
            source,
        })?;

    match payload.status.as_str() {
        "success" => {
            let download_url =
                payload
                    .download_url
                    .ok_or_else(|| OpenAiFileError::UploadFailed {
                        file_id: file_id.to_string(),
                        message: "missing download_url".to_string(),
                    })?;
            Ok(ResolvedOpenAiFile {
                file_id: file_id.to_string(),
                uri: openai_file_uri(file_id),
                download_url,
                file_name: payload.file_name,
                mime_type: payload.mime_type,
                file_size_bytes: payload.file_size_bytes,
            })
        }
        "retry" => Err(OpenAiFileError::UploadNotReady {
            file_id: file_id.to_string(),
        }),
        _ => Err(OpenAiFileError::UploadFailed {
            file_id: file_id.to_string(),
            message: payload
                .error_message
                .unwrap_or_else(|| "download link request returned an error".to_string()),
        }),
    }
}

pub(crate) async fn upload_local_file(
    config: &Config,
    auth: Option<&CodexAuth>,
    path: &Path,
) -> Result<UploadedOpenAiFile, OpenAiFileError> {
    let auth = ensure_chatgpt_auth(auth)?;
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
    let create_url = format!("{}/files", config.chatgpt_base_url.trim_end_matches('/'));
    let create_response = authorized_request(auth, reqwest::Method::POST, &create_url)?
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
        config.chatgpt_base_url.trim_end_matches('/'),
        create_payload.file_id,
    );
    let finalize_response = authorized_request(auth, reqwest::Method::POST, &finalize_url)?
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

pub(crate) async fn download_file_to_managed_temp(
    config: &Config,
    auth: Option<&CodexAuth>,
    cwd: &Path,
    reference: &str,
    scope: &str,
    max_bytes: u64,
) -> Result<DownloadedOpenAiFile, OpenAiFileError> {
    let resolved = resolve_openai_file(config, auth, reference).await?;
    if let Some(file_size_bytes) = resolved.file_size_bytes
        && file_size_bytes > max_bytes
    {
        return Err(OpenAiFileError::RemoteFileTooLarge {
            file_id: resolved.file_id.clone(),
            size_bytes: file_size_bytes,
            limit_bytes: max_bytes,
        });
    }

    let download_dir = managed_download_dir(cwd, scope);
    tokio::fs::create_dir_all(&download_dir)
        .await
        .map_err(|source| OpenAiFileError::CreateDirectory {
            path: download_dir.clone(),
            source,
        })?;

    let file_name = sanitize_download_file_name(
        resolved
            .file_name
            .as_deref()
            .unwrap_or(resolved.file_id.as_str()),
    );
    let destination_path = download_dir.join(&file_name);
    let response = build_reqwest_client()
        .get(&resolved.download_url)
        .timeout(OPENAI_FILE_REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(|source| OpenAiFileError::Request {
            url: resolved.download_url.clone(),
            source,
        })?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(OpenAiFileError::UnexpectedStatus {
            url: resolved.download_url.clone(),
            status,
            body,
        });
    }

    let mut file =
        File::create(&destination_path)
            .await
            .map_err(|source| OpenAiFileError::WriteFile {
                path: destination_path.clone(),
                source,
            })?;
    let mut bytes_written = 0_u64;
    let mut stream = response.bytes_stream();
    while let Some(chunk) =
        futures::TryStreamExt::try_next(&mut stream)
            .await
            .map_err(|source| OpenAiFileError::Request {
                url: resolved.download_url.clone(),
                source,
            })?
    {
        bytes_written = bytes_written.saturating_add(chunk.len() as u64);
        if bytes_written > max_bytes {
            let _ = tokio::fs::remove_file(&destination_path).await;
            return Err(OpenAiFileError::RemoteFileTooLarge {
                file_id: resolved.file_id,
                size_bytes: bytes_written,
                limit_bytes: max_bytes,
            });
        }
        file.write_all(&chunk)
            .await
            .map_err(|source| OpenAiFileError::WriteFile {
                path: destination_path.clone(),
                source,
            })?;
    }
    file.flush()
        .await
        .map_err(|source| OpenAiFileError::WriteFile {
            path: destination_path.clone(),
            source,
        })?;

    Ok(DownloadedOpenAiFile {
        file_id: resolved.file_id,
        uri: resolved.uri,
        file_name,
        mime_type: resolved.mime_type,
        destination_path,
        bytes_written,
    })
}

pub(crate) fn managed_download_dir(cwd: &Path, scope: &str) -> PathBuf {
    cwd.join(".codex")
        .join("tmp")
        .join("openai-files")
        .join(scope)
}

pub(crate) fn unique_manual_download_scope() -> String {
    format!("manual/{}", Uuid::new_v4())
}

fn ensure_chatgpt_auth(auth: Option<&CodexAuth>) -> Result<&CodexAuth, OpenAiFileError> {
    let Some(auth) = auth else {
        return Err(OpenAiFileError::AuthRequired);
    };
    if !auth.is_chatgpt_auth() {
        return Err(OpenAiFileError::UnsupportedAuthMode);
    }
    Ok(auth)
}

fn authorized_request(
    auth: &CodexAuth,
    method: reqwest::Method,
    url: &str,
) -> Result<reqwest::RequestBuilder, OpenAiFileError> {
    let client = build_reqwest_client();
    let token = auth.get_token().map_err(OpenAiFileError::AuthToken)?;
    let mut request = client
        .request(method, url)
        .timeout(OPENAI_FILE_REQUEST_TIMEOUT)
        .bearer_auth(token);
    if let Some(account_id) = auth.get_account_id() {
        request = request.header("chatgpt-account-id", account_id);
    }
    Ok(request)
}

fn sanitize_download_file_name(file_name: &str) -> String {
    let sanitized: String = file_name
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | '\0'..='\u{1f}' | '\u{7f}' => '_',
            _ => ch,
        })
        .collect();
    let trimmed = sanitized.trim_matches('.');
    if trimmed.is_empty() {
        "downloaded-file".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::test_config;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::body_json;
    use wiremock::matchers::header;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    fn chatgpt_auth() -> CodexAuth {
        crate::CodexAuth::create_dummy_chatgpt_auth_for_testing()
    }

    fn test_config_for(server: &MockServer) -> Config {
        let mut config = test_config();
        config.chatgpt_base_url = format!("{}/backend-api", server.uri());
        config
    }

    #[test]
    fn parse_openai_file_id_accepts_uri_and_bare_ids() {
        assert_eq!(
            parse_openai_file_id("sediment://file_123"),
            Some("file_123")
        );
        assert_eq!(parse_openai_file_id("file_123"), Some("file_123"));
        assert_eq!(parse_openai_file_id("file-123"), Some("file-123"));
        assert_eq!(parse_openai_file_id("nope"), None);
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

        let config = test_config_for(&server);
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("hello.txt");
        tokio::fs::write(&path, b"hello").await.expect("write file");

        let uploaded = upload_local_file(&config, Some(&chatgpt_auth()), &path)
            .await
            .expect("upload succeeds");

        assert_eq!(uploaded.file_id, "file_123");
        assert_eq!(uploaded.uri, "sediment://file_123");
        assert_eq!(uploaded.file_name, "hello.txt");
        assert_eq!(uploaded.mime_type, Some("text/plain".to_string()));
    }

    #[tokio::test]
    async fn download_file_to_managed_temp_writes_under_workspace_tmp() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/backend-api/files/download/file_123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "download_url": format!("{}/blob/file_123", server.uri()),
                "file_name": "report.txt",
                "mime_type": "text/plain",
                "file_size_bytes": 4
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/blob/file_123"))
            .respond_with(ResponseTemplate::new(200).set_body_raw("test", "text/plain"))
            .mount(&server)
            .await;

        let workspace = TempDir::new().expect("workspace");
        let downloaded = download_file_to_managed_temp(
            &test_config_for(&server),
            Some(&chatgpt_auth()),
            workspace.path(),
            "sediment://file_123",
            "call-1",
            OPENAI_FILE_DOWNLOAD_LIMIT_BYTES,
        )
        .await
        .expect("download succeeds");

        assert_eq!(downloaded.file_id, "file_123");
        assert_eq!(downloaded.file_name, "report.txt");
        assert_eq!(downloaded.bytes_written, 4);
        assert!(
            downloaded
                .destination_path
                .starts_with(workspace.path().join(".codex/tmp/openai-files/call-1"))
        );
        assert_eq!(
            tokio::fs::read_to_string(&downloaded.destination_path)
                .await
                .expect("read downloaded file"),
            "test"
        );
    }

    #[tokio::test]
    async fn download_file_to_managed_temp_enforces_max_bytes() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/backend-api/files/download/file_123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "download_url": format!("{}/blob/file_123", server.uri()),
                "file_name": "big.txt",
                "mime_type": "text/plain",
                "file_size_bytes": 200
            })))
            .mount(&server)
            .await;

        let workspace = TempDir::new().expect("workspace");
        let error = download_file_to_managed_temp(
            &test_config_for(&server),
            Some(&chatgpt_auth()),
            workspace.path(),
            "file_123",
            "call-2",
            128,
        )
        .await
        .expect_err("download should fail");

        assert_eq!(
            error.to_string(),
            "file `file_123` is too large to download automatically: 200 bytes exceeds the limit of 128 bytes"
        );
    }
}
