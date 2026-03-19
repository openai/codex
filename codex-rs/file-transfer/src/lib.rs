use anyhow::Context;
use anyhow::Result;
use codex_client::build_reqwest_client_with_custom_ca;
use mime_guess::MimeGuess;
use reqwest::header::AUTHORIZATION;
use reqwest::header::CONTENT_DISPOSITION;
use reqwest::header::CONTENT_TYPE;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;
use reqwest::header::USER_AGENT;
use serde::Deserialize;
use serde::Serialize;
use std::future::Future;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use tokio::time::Duration;
use tokio_util::io::ReaderStream;

pub const CODEX_CORE_FILE_TRANSFER_ARG1: &str = "--codex-run-as-file-transfer";
pub const FILE_TRANSFER_BASE_URL_ENV: &str = "CODEX_FILE_TRANSFER_BASE_URL";
pub const FILE_TRANSFER_BEARER_TOKEN_ENV: &str = "CODEX_FILE_TRANSFER_BEARER_TOKEN";
pub const FILE_TRANSFER_ACCOUNT_ID_ENV: &str = "CODEX_FILE_TRANSFER_ACCOUNT_ID";
pub const FILE_TRANSFER_USER_AGENT_ENV: &str = "CODEX_FILE_TRANSFER_USER_AGENT";
const NETWORK_RETRY_ATTEMPTS: u32 = 3;
const NETWORK_RETRY_DELAYS_MS: [u64; 2] = [250, 750];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum FileTransferRequest {
    Upload {
        path: PathBuf,
    },
    Download {
        file_id: String,
        path: PathBuf,
        path_is_directory: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UploadFileToolResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retryable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_status_code: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DownloadFileToolResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_written: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retryable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_status_code: Option<u16>,
}

#[derive(Debug, Clone)]
pub struct FileTransferConfig {
    pub base_url: String,
    pub bearer_token: String,
    pub chatgpt_account_id: Option<String>,
    pub user_agent: String,
}

#[derive(Debug)]
pub enum FileTransferOutput {
    Upload(UploadFileToolResult),
    Download(DownloadFileToolResult),
}

#[derive(Debug, Serialize)]
struct CreateFileRequestBody {
    file_name: String,
    file_size: u64,
    use_case: &'static str,
}

#[derive(Debug, Deserialize)]
struct CreateFileResponseBody {
    file_id: String,
    upload_url: String,
}

#[derive(Debug, Deserialize)]
struct FinalizeUploadResponseBody {
    status: String,
    download_url: String,
    file_name: String,
    file_size_bytes: u64,
    mime_type: String,
}

#[derive(Debug, Deserialize)]
struct DownloadLinkResponseBody {
    status: String,
    download_url: Option<String>,
    #[serde(default)]
    file_name: Option<String>,
    #[serde(default)]
    mime_type: Option<String>,
}

impl FileTransferOutput {
    pub fn to_json(&self) -> Result<String> {
        match self {
            Self::Upload(output) => serde_json::to_string(output).map_err(anyhow::Error::from),
            Self::Download(output) => serde_json::to_string(output).map_err(anyhow::Error::from),
        }
    }
}

impl FileTransferConfig {
    pub fn from_env() -> Result<Self> {
        let base_url = std::env::var(FILE_TRANSFER_BASE_URL_ENV)
            .with_context(|| format!("missing {FILE_TRANSFER_BASE_URL_ENV}"))?;
        let bearer_token = std::env::var(FILE_TRANSFER_BEARER_TOKEN_ENV)
            .with_context(|| format!("missing {FILE_TRANSFER_BEARER_TOKEN_ENV}"))?;
        let user_agent = std::env::var(FILE_TRANSFER_USER_AGENT_ENV)
            .with_context(|| format!("missing {FILE_TRANSFER_USER_AGENT_ENV}"))?;
        let chatgpt_account_id = std::env::var(FILE_TRANSFER_ACCOUNT_ID_ENV)
            .ok()
            .filter(|value| !value.is_empty());
        Ok(Self {
            base_url,
            bearer_token,
            chatgpt_account_id,
            user_agent,
        })
    }

    fn headers(&self) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.bearer_token))
                .context("invalid bearer token header")?,
        );
        headers.insert(
            USER_AGENT,
            HeaderValue::from_str(&self.user_agent).context("invalid user agent header")?,
        );
        if let Some(account_id) = &self.chatgpt_account_id {
            headers.insert(
                HeaderName::from_static("chatgpt-account-id"),
                HeaderValue::from_str(account_id).context("invalid account id header")?,
            );
        }
        Ok(headers)
    }

    fn download_headers(&self) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_str(&self.user_agent).context("invalid user agent header")?,
        );
        Ok(headers)
    }

    fn files_api_url(&self) -> String {
        let base_url = normalize_base_url(&self.base_url);
        if base_url.contains("/backend-api") {
            format!("{base_url}/files")
        } else {
            format!("{base_url}/api/files")
        }
    }
}

pub async fn run_from_arg(request_arg: &str) -> Result<FileTransferOutput> {
    let request: FileTransferRequest =
        serde_json::from_str(request_arg).context("failed to decode file transfer request")?;
    let config = FileTransferConfig::from_env()?;
    run_request(&config, &request).await
}

pub async fn run_request(
    config: &FileTransferConfig,
    request: &FileTransferRequest,
) -> Result<FileTransferOutput> {
    match request {
        FileTransferRequest::Upload { path } => {
            Ok(FileTransferOutput::Upload(upload_file(config, path).await?))
        }
        FileTransferRequest::Download {
            file_id,
            path,
            path_is_directory,
        } => Ok(FileTransferOutput::Download(
            download_file(config, file_id, path, *path_is_directory).await?,
        )),
    }
}

async fn upload_file(config: &FileTransferConfig, path: &Path) -> Result<UploadFileToolResult> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .context("upload path has no valid UTF-8 filename")?;
    let metadata = match tokio::fs::metadata(path).await {
        Ok(metadata) => metadata,
        Err(err) => return Ok(upload_path_error(path, err)),
    };
    let file_size = metadata.len();

    let client = build_reqwest_client_with_custom_ca(reqwest::Client::builder())?;
    let create_url = config.files_api_url();
    let create_request = CreateFileRequestBody {
        file_name: file_name.clone(),
        file_size,
        use_case: "codex",
    };
    let create_headers = config.headers()?;
    let create_response = match send_with_retries(|| {
        client
            .post(&create_url)
            .headers(create_headers.clone())
            .json(&create_request)
            .send()
    })
    .await
    {
        Ok(response) => response,
        Err(err) => {
            return Ok(UploadFileToolResult {
                ok: false,
                file_id: None,
                uri: None,
                file_name: Some(file_name.clone()),
                file_size_bytes: Some(file_size),
                mime_type: None,
                error_code: Some("backend_request_failed".to_string()),
                message: Some(format!("file create request failed after retries: {err}")),
                retryable: Some(is_retryable_request_error(&err)),
                http_status_code: None,
                path: Some(path.display().to_string()),
            });
        }
    };
    if !create_response.status().is_success() {
        let status = create_response.status().as_u16();
        let body = create_response.text().await.unwrap_or_default();
        return Ok(UploadFileToolResult {
            ok: false,
            file_id: None,
            uri: None,
            file_name: Some(file_name),
            file_size_bytes: Some(file_size),
            mime_type: None,
            error_code: Some("backend_request_failed".to_string()),
            message: Some(format!(
                "file create request failed with status {status}: {body}"
            )),
            retryable: Some(is_retryable_http_status(status)),
            http_status_code: Some(status),
            path: Some(path.display().to_string()),
        });
    }
    let create_payload: CreateFileResponseBody = create_response
        .json()
        .await
        .context("failed to decode file create response")?;

    let mime_type = MimeGuess::from_path(path)
        .first_or_octet_stream()
        .to_string();
    let upload_url = create_payload.upload_url.clone();
    let upload_response = {
        let mut attempt = 1;
        loop {
            let upload_file = match tokio::fs::File::open(path).await {
                Ok(file) => file,
                Err(err) => return Ok(upload_path_error(path, err)),
            };
            match client
                .put(&upload_url)
                .header("x-ms-blob-type", "BlockBlob")
                .header(CONTENT_TYPE, mime_type.as_str())
                .body(reqwest::Body::wrap_stream(ReaderStream::new(upload_file)))
                .send()
                .await
            {
                Ok(response)
                    if attempt < NETWORK_RETRY_ATTEMPTS
                        && is_retryable_http_status(response.status().as_u16()) =>
                {
                    sleep_before_retry(attempt).await;
                    attempt += 1;
                }
                Ok(response) => break response,
                Err(err)
                    if attempt < NETWORK_RETRY_ATTEMPTS && is_retryable_request_error(&err) =>
                {
                    sleep_before_retry(attempt).await;
                    attempt += 1;
                }
                Err(err) => {
                    return Ok(UploadFileToolResult {
                        ok: false,
                        file_id: Some(create_payload.file_id.clone()),
                        uri: Some(openai_file_uri(&create_payload.file_id)),
                        file_name: Some(file_name),
                        file_size_bytes: Some(file_size),
                        mime_type: Some(mime_type),
                        error_code: Some("upload_failed".to_string()),
                        message: Some(format!("byte upload failed after retries: {err}")),
                        retryable: Some(is_retryable_request_error(&err)),
                        http_status_code: None,
                        path: Some(path.display().to_string()),
                    });
                }
            }
        }
    };
    if !upload_response.status().is_success() {
        let status = upload_response.status().as_u16();
        let body = upload_response.text().await.unwrap_or_default();
        return Ok(UploadFileToolResult {
            ok: false,
            file_id: Some(create_payload.file_id.clone()),
            uri: Some(openai_file_uri(&create_payload.file_id)),
            file_name: Some(file_name),
            file_size_bytes: Some(file_size),
            mime_type: Some(mime_type),
            error_code: Some("upload_failed".to_string()),
            message: Some(format!("byte upload failed with status {status}: {body}")),
            retryable: Some(is_retryable_http_status(status)),
            http_status_code: Some(status),
            path: Some(path.display().to_string()),
        });
    }

    let finalize_url = format!("{}/{}/uploaded", create_url, create_payload.file_id);
    let finalize_headers = config.headers()?;
    let finalize_response = match send_with_retries(|| {
        client
            .post(&finalize_url)
            .headers(finalize_headers.clone())
            .send()
    })
    .await
    {
        Ok(response) => response,
        Err(err) => {
            return Ok(UploadFileToolResult {
                ok: false,
                file_id: Some(create_payload.file_id.clone()),
                uri: Some(openai_file_uri(&create_payload.file_id)),
                file_name: Some(file_name),
                file_size_bytes: Some(file_size),
                mime_type: Some(mime_type),
                error_code: Some("finalize_failed".to_string()),
                message: Some(format!("finalize request failed after retries: {err}")),
                retryable: Some(is_retryable_request_error(&err)),
                http_status_code: None,
                path: Some(path.display().to_string()),
            });
        }
    };
    if !finalize_response.status().is_success() {
        let status = finalize_response.status().as_u16();
        let body = finalize_response.text().await.unwrap_or_default();
        return Ok(UploadFileToolResult {
            ok: false,
            file_id: Some(create_payload.file_id.clone()),
            uri: Some(openai_file_uri(&create_payload.file_id)),
            file_name: Some(file_name),
            file_size_bytes: Some(file_size),
            mime_type: Some(mime_type),
            error_code: Some("finalize_failed".to_string()),
            message: Some(format!(
                "finalize request failed with status {status}: {body}"
            )),
            retryable: Some(is_retryable_http_status(status)),
            http_status_code: Some(status),
            path: Some(path.display().to_string()),
        });
    }
    let finalize_payload: FinalizeUploadResponseBody = finalize_response
        .json()
        .await
        .context("failed to decode finalize response")?;
    let _ = &finalize_payload.status;
    let _ = &finalize_payload.download_url;

    Ok(UploadFileToolResult {
        ok: true,
        file_id: Some(create_payload.file_id.clone()),
        uri: Some(openai_file_uri(&create_payload.file_id)),
        file_name: Some(finalize_payload.file_name),
        file_size_bytes: Some(finalize_payload.file_size_bytes),
        mime_type: Some(finalize_payload.mime_type),
        error_code: None,
        message: None,
        retryable: None,
        http_status_code: None,
        path: Some(path.display().to_string()),
    })
}

async fn download_file(
    config: &FileTransferConfig,
    file_id: &str,
    path: &Path,
    path_is_directory: bool,
) -> Result<DownloadFileToolResult> {
    let client = build_reqwest_client_with_custom_ca(reqwest::Client::builder())?;
    let url = format!("{}/download/{file_id}", config.files_api_url());
    let link_headers = config.headers()?;
    let mut link_payload = None;
    for attempt in 1..=NETWORK_RETRY_ATTEMPTS {
        let link_response =
            match send_with_retries(|| client.get(&url).headers(link_headers.clone()).send()).await
            {
                Ok(response) => response,
                Err(err) => {
                    return Ok(DownloadFileToolResult {
                        ok: false,
                        file_id: Some(file_id.to_string()),
                        uri: Some(openai_file_uri(file_id)),
                        file_name: None,
                        mime_type: None,
                        destination_path: Some(path.display().to_string()),
                        bytes_written: None,
                        error_code: Some("download_failed".to_string()),
                        message: Some(format!("download request failed after retries: {err}")),
                        retryable: Some(is_retryable_request_error(&err)),
                        http_status_code: None,
                    });
                }
            };

        if !link_response.status().is_success() {
            let status = link_response.status().as_u16();
            let body = link_response.text().await.unwrap_or_default();
            return Ok(DownloadFileToolResult {
                ok: false,
                file_id: Some(file_id.to_string()),
                uri: Some(openai_file_uri(file_id)),
                file_name: None,
                mime_type: None,
                destination_path: Some(path.display().to_string()),
                bytes_written: None,
                error_code: Some("download_failed".to_string()),
                message: Some(format!(
                    "download request failed with status {status}: {body}"
                )),
                retryable: Some(is_retryable_http_status(status)),
                http_status_code: Some(status),
            });
        }

        let current_payload: DownloadLinkResponseBody = link_response
            .json()
            .await
            .context("failed to decode download link response")?;
        if current_payload.status == "success" {
            link_payload = Some(current_payload);
            break;
        }
        if current_payload.status == "retry" && attempt < NETWORK_RETRY_ATTEMPTS {
            sleep_before_retry(attempt).await;
            continue;
        }
        return Ok(DownloadFileToolResult {
            ok: false,
            file_id: Some(file_id.to_string()),
            uri: Some(openai_file_uri(file_id)),
            file_name: current_payload.file_name,
            mime_type: current_payload.mime_type,
            destination_path: Some(path.display().to_string()),
            bytes_written: None,
            error_code: Some("download_failed".to_string()),
            message: Some(format!(
                "download link response reported status `{}`",
                current_payload.status
            )),
            retryable: Some(current_payload.status == "retry"),
            http_status_code: None,
        });
    }
    let link_payload =
        link_payload.context("download link retry loop did not produce a payload")?;
    let Some(download_url) = link_payload.download_url else {
        return Ok(DownloadFileToolResult {
            ok: false,
            file_id: Some(file_id.to_string()),
            uri: Some(openai_file_uri(file_id)),
            file_name: link_payload.file_name,
            mime_type: link_payload.mime_type,
            destination_path: Some(path.display().to_string()),
            bytes_written: None,
            error_code: Some("download_failed".to_string()),
            message: Some("download link response did not include a download_url".to_string()),
            retryable: Some(false),
            http_status_code: None,
        });
    };
    let download_headers = config.download_headers()?;
    let response = match send_with_retries(|| {
        client
            .get(download_url.as_str())
            .headers(download_headers.clone())
            .send()
    })
    .await
    {
        Ok(response) => response,
        Err(err) => {
            return Ok(DownloadFileToolResult {
                ok: false,
                file_id: Some(file_id.to_string()),
                uri: Some(openai_file_uri(file_id)),
                file_name: link_payload.file_name,
                mime_type: link_payload.mime_type,
                destination_path: Some(path.display().to_string()),
                bytes_written: None,
                error_code: Some("download_failed".to_string()),
                message: Some(format!(
                    "download bytes request failed after retries: {err}"
                )),
                retryable: Some(is_retryable_request_error(&err)),
                http_status_code: None,
            });
        }
    };
    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        return Ok(DownloadFileToolResult {
            ok: false,
            file_id: Some(file_id.to_string()),
            uri: Some(openai_file_uri(file_id)),
            file_name: link_payload.file_name,
            mime_type: link_payload.mime_type,
            destination_path: Some(path.display().to_string()),
            bytes_written: None,
            error_code: Some("download_failed".to_string()),
            message: Some(format!(
                "download bytes request failed with status {status}: {body}"
            )),
            retryable: Some(is_retryable_http_status(status)),
            http_status_code: Some(status),
        });
    }

    let file_name = response
        .headers()
        .get(CONTENT_DISPOSITION)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_content_disposition_filename)
        .map(|file_name| sanitize_download_file_name(&file_name))
        .or_else(|| {
            link_payload
                .file_name
                .as_deref()
                .map(sanitize_download_file_name)
        })
        .unwrap_or_else(|| file_id.to_string());
    let mime_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
        .or(link_payload.mime_type)
        .unwrap_or_else(|| "application/octet-stream".to_string());
    let destination_path = if path_is_directory {
        path.join(&file_name)
    } else {
        path.to_path_buf()
    };
    let mut destination_file = match tokio::fs::File::create(&destination_path).await {
        Ok(file) => file,
        Err(err) => return Ok(download_path_error(file_id, path, &destination_path, err)),
    };
    let mut bytes_written = 0u64;
    let mut response = response;
    while let Some(chunk) = response
        .chunk()
        .await
        .context("failed to read download bytes")?
    {
        if let Err(err) = destination_file.write_all(&chunk).await {
            return Ok(download_path_error(file_id, path, &destination_path, err));
        }
        bytes_written += chunk.len() as u64;
    }
    if let Err(err) = destination_file.flush().await {
        return Ok(download_path_error(file_id, path, &destination_path, err));
    }

    Ok(DownloadFileToolResult {
        ok: true,
        file_id: Some(file_id.to_string()),
        uri: Some(openai_file_uri(file_id)),
        file_name: Some(file_name),
        mime_type: Some(mime_type),
        destination_path: Some(destination_path.display().to_string()),
        bytes_written: Some(bytes_written),
        error_code: None,
        message: None,
        retryable: None,
        http_status_code: None,
    })
}

fn normalize_base_url(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if (trimmed.starts_with("https://chatgpt.com")
        || trimmed.starts_with("https://chat.openai.com"))
        && !trimmed.contains("/backend-api")
    {
        format!("{trimmed}/backend-api")
    } else {
        trimmed.to_string()
    }
}

async fn send_with_retries<F, Fut>(mut send: F) -> Result<reqwest::Response, reqwest::Error>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<reqwest::Response, reqwest::Error>>,
{
    let mut attempt = 1;
    loop {
        match send().await {
            Ok(response) => {
                if attempt < NETWORK_RETRY_ATTEMPTS
                    && is_retryable_http_status(response.status().as_u16())
                {
                    sleep_before_retry(attempt).await;
                    attempt += 1;
                    continue;
                }
                return Ok(response);
            }
            Err(err) => {
                if attempt < NETWORK_RETRY_ATTEMPTS && is_retryable_request_error(&err) {
                    sleep_before_retry(attempt).await;
                    attempt += 1;
                    continue;
                }
                return Err(err);
            }
        }
    }
}

fn is_retryable_http_status(status: u16) -> bool {
    matches!(status, 408 | 409 | 425 | 429) || status >= 500
}

fn is_retryable_request_error(err: &reqwest::Error) -> bool {
    err.is_timeout() || err.is_connect() || err.is_request()
}

async fn sleep_before_retry(attempt: u32) {
    let delay_ms = NETWORK_RETRY_DELAYS_MS
        .get((attempt - 1) as usize)
        .copied()
        .unwrap_or(*NETWORK_RETRY_DELAYS_MS.last().unwrap_or(&750));
    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
}

fn openai_file_uri(file_id: &str) -> String {
    format!("openai-file://v1/{file_id}")
}

fn parse_content_disposition_filename(content_disposition: &str) -> Option<String> {
    for part in content_disposition.split(';').map(str::trim) {
        if let Some(value) = part.strip_prefix("filename*=") {
            let value = value.strip_prefix("UTF-8''").unwrap_or(value);
            return Some(percent_decode(value));
        }
        if let Some(value) = part.strip_prefix("filename=") {
            return Some(value.trim_matches('"').to_string());
        }
    }
    None
}

fn sanitize_download_file_name(file_name: &str) -> String {
    let sanitized = Path::new(file_name)
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or("");
    if sanitized.is_empty() {
        "download".to_string()
    } else {
        sanitized.to_string()
    }
}

fn upload_path_error(path: &Path, err: std::io::Error) -> UploadFileToolResult {
    let error_code = match err.kind() {
        ErrorKind::NotFound => "path_not_found",
        ErrorKind::PermissionDenied => "sandbox_path_denied",
        _ => "upload_failed",
    };
    UploadFileToolResult {
        ok: false,
        file_id: None,
        uri: None,
        file_name: path
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string),
        file_size_bytes: None,
        mime_type: None,
        error_code: Some(error_code.to_string()),
        message: Some(format!("failed to access `{}`: {err}", path.display())),
        retryable: Some(matches!(err.kind(), ErrorKind::Interrupted)),
        http_status_code: None,
        path: Some(path.display().to_string()),
    }
}

fn download_path_error(
    file_id: &str,
    requested_path: &Path,
    destination_path: &Path,
    err: std::io::Error,
) -> DownloadFileToolResult {
    let error_code = match err.kind() {
        ErrorKind::NotFound => "destination_parent_missing",
        ErrorKind::PermissionDenied => "sandbox_path_denied",
        _ => "download_failed",
    };
    DownloadFileToolResult {
        ok: false,
        file_id: Some(file_id.to_string()),
        uri: Some(openai_file_uri(file_id)),
        file_name: None,
        mime_type: None,
        destination_path: Some(destination_path.display().to_string()),
        bytes_written: None,
        error_code: Some(error_code.to_string()),
        message: Some(format!(
            "failed to write download for `{}` to `{}`: {err}",
            requested_path.display(),
            destination_path.display()
        )),
        retryable: Some(matches!(err.kind(), ErrorKind::Interrupted)),
        http_status_code: None,
    }
}

fn percent_decode(value: &str) -> String {
    let mut decoded = String::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
        {
            decoded.push((high * 16 + low) as char);
            index += 3;
            continue;
        }
        decoded.push(bytes[index] as char);
        index += 1;
    }
    decoded
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::body_json;
    use wiremock::matchers::header;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    fn test_config(server: &MockServer) -> FileTransferConfig {
        FileTransferConfig {
            base_url: format!("{}/backend-api", server.uri()),
            bearer_token: "token".to_string(),
            chatgpt_account_id: Some("acct".to_string()),
            user_agent: "codex-test".to_string(),
        }
    }

    #[tokio::test]
    async fn upload_request_uses_expected_flow() -> Result<()> {
        let server = MockServer::start().await;
        let file_id = "file-123";
        let upload_url = format!("{}/upload-target", server.uri());
        Mock::given(method("POST"))
            .and(path("/backend-api/files"))
            .and(header(AUTHORIZATION.as_str(), "Bearer token"))
            .and(header("chatgpt-account-id", "acct"))
            .and(body_json(serde_json::json!({
                "file_name": "hello.txt",
                "file_size": 5,
                "use_case": "codex"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "file_id": file_id,
                "upload_url": upload_url
            })))
            .mount(&server)
            .await;
        Mock::given(method("PUT"))
            .and(path("/upload-target"))
            .respond_with(ResponseTemplate::new(201))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path(format!("/backend-api/files/{file_id}/uploaded")))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "download_url": format!("{}/blob/file-123", server.uri()),
                "file_name": "hello.txt",
                "file_size_bytes": 5,
                "mime_type": "text/plain"
            })))
            .mount(&server)
            .await;

        let dir = tempdir()?;
        let path = dir.path().join("hello.txt");
        tokio::fs::write(&path, b"hello").await?;

        let output = upload_file(&test_config(&server), &path).await?;
        assert_eq!(
            output,
            UploadFileToolResult {
                ok: true,
                file_id: Some(file_id.to_string()),
                uri: Some(openai_file_uri(file_id)),
                file_name: Some("hello.txt".to_string()),
                file_size_bytes: Some(5),
                mime_type: Some("text/plain".to_string()),
                error_code: None,
                message: None,
                retryable: None,
                http_status_code: None,
                path: Some(path.display().to_string()),
            }
        );
        Ok(())
    }

    #[tokio::test]
    async fn upload_retries_create_request_after_transient_server_error() -> Result<()> {
        let server = MockServer::start().await;
        let file_id = "file-retry";
        let upload_url = format!("{}/upload-target", server.uri());
        Mock::given(method("POST"))
            .and(path("/backend-api/files"))
            .respond_with(ResponseTemplate::new(500).set_body_string("temporary failure"))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/backend-api/files"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "file_id": file_id,
                "upload_url": upload_url
            })))
            .mount(&server)
            .await;
        Mock::given(method("PUT"))
            .and(path("/upload-target"))
            .respond_with(ResponseTemplate::new(201))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path(format!("/backend-api/files/{file_id}/uploaded")))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "download_url": format!("{}/blob/{file_id}", server.uri()),
                "file_name": "retry.txt",
                "file_size_bytes": 5,
                "mime_type": "text/plain"
            })))
            .mount(&server)
            .await;

        let dir = tempdir()?;
        let path = dir.path().join("retry.txt");
        tokio::fs::write(&path, b"hello").await?;

        let output = upload_file(&test_config(&server), &path).await?;
        assert_eq!(output.ok, true);
        let requests = server.received_requests().await.unwrap_or_default();
        let create_requests = requests
            .iter()
            .filter(|request| request.url.path() == "/backend-api/files")
            .count();
        assert_eq!(create_requests, 2);
        Ok(())
    }

    #[tokio::test]
    async fn upload_zero_byte_file_uses_size_zero() -> Result<()> {
        let server = MockServer::start().await;
        let file_id = "file-empty";
        let upload_url = format!("{}/upload-target", server.uri());
        Mock::given(method("POST"))
            .and(path("/backend-api/files"))
            .and(body_json(serde_json::json!({
                "file_name": "empty.txt",
                "file_size": 0,
                "use_case": "codex"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "file_id": file_id,
                "upload_url": upload_url
            })))
            .mount(&server)
            .await;
        Mock::given(method("PUT"))
            .and(path("/upload-target"))
            .respond_with(ResponseTemplate::new(201))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path(format!("/backend-api/files/{file_id}/uploaded")))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "download_url": format!("{}/blob/{file_id}", server.uri()),
                "file_name": "empty.txt",
                "file_size_bytes": 0,
                "mime_type": "text/plain"
            })))
            .mount(&server)
            .await;

        let dir = tempdir()?;
        let path = dir.path().join("empty.txt");
        tokio::fs::write(&path, b"").await?;

        let output = upload_file(&test_config(&server), &path).await?;
        assert_eq!(output.file_size_bytes, Some(0));
        assert_eq!(output.file_id, Some(file_id.to_string()));
        assert_eq!(output.uri, Some(openai_file_uri(file_id)));
        Ok(())
    }

    #[tokio::test]
    async fn download_directory_uses_remote_file_name() -> Result<()> {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/backend-api/files/download/file-456"))
            .and(header(AUTHORIZATION.as_str(), "Bearer token"))
            .and(header("chatgpt-account-id", "acct"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "download_url": format!("{}/content/file-456", server.uri()),
                "file_name": "ignored.txt",
                "mime_type": "text/plain"
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/content/file-456"))
            .and(header(USER_AGENT.as_str(), "codex-test"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header(CONTENT_TYPE.as_str(), "text/plain")
                    .insert_header(
                        CONTENT_DISPOSITION.as_str(),
                        "attachment; filename*=UTF-8''report%20final.txt",
                    )
                    .set_body_bytes(b"downloaded".to_vec()),
            )
            .mount(&server)
            .await;

        let dir = tempdir()?;
        let output = download_file(&test_config(&server), "file-456", dir.path(), true).await?;
        let expected_path = dir.path().join("report final.txt");
        assert_eq!(tokio::fs::read(&expected_path).await?, b"downloaded");
        assert_eq!(
            output,
            DownloadFileToolResult {
                ok: true,
                file_id: Some("file-456".to_string()),
                uri: Some(openai_file_uri("file-456")),
                file_name: Some("report final.txt".to_string()),
                mime_type: Some("text/plain".to_string()),
                destination_path: Some(expected_path.display().to_string()),
                bytes_written: Some(10),
                error_code: None,
                message: None,
                retryable: None,
                http_status_code: None,
            }
        );
        let requests = server.received_requests().await.unwrap_or_default();
        let content_request = requests
            .iter()
            .find(|request| request.url.path() == "/content/file-456")
            .context("expected blob download request")?;
        assert_eq!(
            content_request
                .headers
                .get(USER_AGENT.as_str())
                .and_then(|value| value.to_str().ok()),
            Some("codex-test")
        );
        assert_eq!(content_request.headers.get(AUTHORIZATION.as_str()), None);
        assert_eq!(content_request.headers.get("chatgpt-account-id"), None);
        Ok(())
    }

    #[tokio::test]
    async fn download_directory_sanitizes_remote_file_name() -> Result<()> {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/backend-api/files/download/file-456"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "download_url": format!("{}/content/file-456", server.uri()),
                "file_name": "../report.txt",
                "mime_type": "text/plain"
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/content/file-456"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header(CONTENT_TYPE.as_str(), "text/plain")
                    .insert_header(
                        CONTENT_DISPOSITION.as_str(),
                        "attachment; filename=../report.txt",
                    )
                    .set_body_bytes(b"downloaded".to_vec()),
            )
            .mount(&server)
            .await;

        let dir = tempdir()?;
        let output = download_file(&test_config(&server), "file-456", dir.path(), true).await?;
        let expected_path = dir.path().join("report.txt");
        assert_eq!(
            output,
            DownloadFileToolResult {
                ok: true,
                file_id: Some("file-456".to_string()),
                uri: Some(openai_file_uri("file-456")),
                file_name: Some("report.txt".to_string()),
                mime_type: Some("text/plain".to_string()),
                destination_path: Some(expected_path.display().to_string()),
                bytes_written: Some(10),
                error_code: None,
                message: None,
                retryable: None,
                http_status_code: None,
            }
        );
        assert_eq!(
            tokio::fs::read_to_string(expected_path).await?,
            "downloaded"
        );
        Ok(())
    }

    #[tokio::test]
    async fn download_uses_json_link_then_fetches_bytes() -> Result<()> {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/backend-api/files/download/file-789"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "download_url": format!("{}/content/file-789", server.uri()),
                "file_name": "fallback.txt",
                "mime_type": "text/plain"
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/content/file-789"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"hello bytes".to_vec()))
            .mount(&server)
            .await;

        let dir = tempdir()?;
        let path = dir.path().join("out.txt");
        let output = download_file(&test_config(&server), "file-789", &path, false).await?;
        assert_eq!(tokio::fs::read_to_string(path).await?, "hello bytes");
        assert_eq!(output.file_name, Some("fallback.txt".to_string()));
        assert_eq!(output.mime_type, Some("text/plain".to_string()));
        Ok(())
    }

    #[tokio::test]
    async fn download_directory_uses_utf8_file_name_from_link_payload() -> Result<()> {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/backend-api/files/download/file-unicode"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "download_url": format!("{}/content/file-unicode", server.uri()),
                "file_name": "café.txt",
                "mime_type": "text/plain"
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/content/file-unicode"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"bonjour".to_vec()))
            .mount(&server)
            .await;

        let dir = tempdir()?;
        let output = download_file(&test_config(&server), "file-unicode", dir.path(), true).await?;
        let expected_path = dir.path().join("café.txt");
        assert_eq!(tokio::fs::read_to_string(expected_path).await?, "bonjour");
        assert_eq!(output.file_name, Some("café.txt".to_string()));
        Ok(())
    }

    #[tokio::test]
    async fn download_retries_when_link_payload_requests_retry() -> Result<()> {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/backend-api/files/download/file-retry"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "retry"
            })))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/backend-api/files/download/file-retry"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "download_url": format!("{}/content/file-retry", server.uri()),
                "file_name": "retried.txt",
                "mime_type": "text/plain"
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/content/file-retry"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"retried bytes".to_vec()))
            .mount(&server)
            .await;

        let dir = tempdir()?;
        let output = download_file(&test_config(&server), "file-retry", dir.path(), true).await?;
        assert_eq!(output.ok, true);
        assert_eq!(output.file_name, Some("retried.txt".to_string()));
        let requests = server.received_requests().await.unwrap_or_default();
        let link_requests = requests
            .iter()
            .filter(|request| request.url.path() == "/backend-api/files/download/file-retry")
            .count();
        assert_eq!(link_requests, 2);
        Ok(())
    }

    #[tokio::test]
    async fn upload_missing_path_returns_structured_error() -> Result<()> {
        let server = MockServer::start().await;
        let dir = tempdir()?;
        let path = dir.path().join("missing.txt");

        let output = upload_file(&test_config(&server), &path).await?;
        assert_eq!(output.ok, false);
        assert_eq!(output.file_id, None);
        assert_eq!(output.uri, None);
        assert_eq!(output.file_name, Some("missing.txt".to_string()));
        assert_eq!(output.file_size_bytes, None);
        assert_eq!(output.mime_type, None);
        assert_eq!(output.error_code, Some("path_not_found".to_string()));
        assert_eq!(output.retryable, Some(false));
        assert_eq!(output.http_status_code, None);
        assert_eq!(output.path, Some(path.display().to_string()));
        assert!(
            output
                .message
                .as_deref()
                .is_some_and(|message| message.contains(&path.display().to_string())),
            "message should mention missing path"
        );
        Ok(())
    }

    #[test]
    fn files_api_url_uses_chatgpt_backend_style() {
        let config = FileTransferConfig {
            base_url: "https://chatgpt.com/backend-api/".to_string(),
            bearer_token: "token".to_string(),
            chatgpt_account_id: None,
            user_agent: "codex-test".to_string(),
        };

        assert_eq!(
            config.files_api_url(),
            "https://chatgpt.com/backend-api/files"
        );
    }

    #[test]
    fn files_api_url_uses_non_chatgpt_api_style() {
        let config = FileTransferConfig {
            base_url: "https://example.test".to_string(),
            bearer_token: "token".to_string(),
            chatgpt_account_id: None,
            user_agent: "codex-test".to_string(),
        };

        assert_eq!(config.files_api_url(), "https://example.test/api/files");
    }
}
