use std::net::IpAddr;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use codex_app_server_protocol::JSONRPCErrorError;
use codex_client::build_reqwest_client_with_custom_ca;
use futures::stream;
use reqwest::Method;
use reqwest::Url;
use reqwest::header::CONTENT_LENGTH;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;
use reqwest::redirect::Policy;
use tokio_util::sync::CancellationToken;
use zeroize::Zeroizing;

use crate::protocol::FileTransferHeader;
use crate::protocol::FileTransferUploadDescriptor;
use crate::rpc::invalid_params;

const MAX_DESCRIPTOR_URL_BYTES: usize = 8 * 1024;
const MAX_DESCRIPTOR_HEADERS: usize = 16;
const MAX_DESCRIPTOR_HEADER_BYTES: usize = 16 * 1024;
const MIN_DESCRIPTOR_LIFETIME: Duration = Duration::from_secs(30);
const MAX_DESCRIPTOR_LIFETIME: Duration = Duration::from_secs(10 * 60);
const DNS_TIMEOUT: Duration = Duration::from_secs(5);
const TRANSFER_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(5 * 60);

pub(super) enum UploadOutcome {
    Succeeded,
    Failed(String),
    CompletionUnknown(String),
}

pub(super) struct ValidatedUploadDescriptor {
    url: Url,
    addresses: Vec<std::net::SocketAddr>,
    headers: HeaderMap,
}

impl std::fmt::Debug for ValidatedUploadDescriptor {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ValidatedUploadDescriptor")
            .field("url", &"[REDACTED]")
            .field("address_count", &self.addresses.len())
            .field("header_count", &self.headers.len())
            .finish()
    }
}

pub(super) async fn validate_upload_descriptor(
    descriptor: FileTransferUploadDescriptor,
) -> Result<ValidatedUploadDescriptor, JSONRPCErrorError> {
    let FileTransferUploadDescriptor::HttpsPut {
        url,
        headers,
        expires_at_unix_seconds,
    } = descriptor;
    validate_expiry(expires_at_unix_seconds)?;
    if url.len() > MAX_DESCRIPTOR_URL_BYTES {
        return Err(invalid_params(
            "file transfer URL exceeds the size limit".to_string(),
        ));
    }
    let url =
        Url::parse(&url).map_err(|_| invalid_params("file transfer URL is invalid".to_string()))?;
    let local_development_url = is_local_development_url(&url);
    if url.scheme() != "https" && !local_development_url {
        return Err(invalid_params(
            "file transfer URL must use HTTPS".to_string(),
        ));
    }
    if !url.username().is_empty() || url.password().is_some() || url.fragment().is_some() {
        return Err(invalid_params(
            "file transfer URL must not contain credentials or a fragment".to_string(),
        ));
    }
    let host = url
        .host_str()
        .ok_or_else(|| invalid_params("file transfer URL must contain a host".to_string()))?;
    if !local_development_url && !is_trusted_transfer_host(host) {
        return Err(invalid_params(
            "file transfer URL host is not trusted".to_string(),
        ));
    }
    let port = url
        .port_or_known_default()
        .ok_or_else(|| invalid_params("file transfer URL has no valid port".to_string()))?;
    if !local_development_url && port != 443 {
        return Err(invalid_params(
            "file transfer URL must use port 443".to_string(),
        ));
    }
    let addresses = tokio::time::timeout(DNS_TIMEOUT, tokio::net::lookup_host((host, port)))
        .await
        .map_err(|_| invalid_params("file transfer host resolution timed out".to_string()))?
        .map_err(|_| invalid_params("file transfer host resolution failed".to_string()))?
        .collect::<Vec<_>>();
    if addresses.is_empty()
        || addresses
            .iter()
            .any(|address| is_disallowed_transfer_address(address.ip()) && !local_development_url)
    {
        return Err(invalid_params(
            "file transfer URL resolved to a disallowed address".to_string(),
        ));
    }
    let azure_blob = host
        .trim_end_matches('.')
        .to_ascii_lowercase()
        .ends_with(".blob.core.windows.net");
    let headers = validate_headers(headers, azure_blob)?;
    Ok(ValidatedUploadDescriptor {
        url,
        addresses,
        headers,
    })
}

pub(super) async fn upload_bytes(
    bytes: Zeroizing<Vec<u8>>,
    descriptor: ValidatedUploadDescriptor,
    cancellation: CancellationToken,
) -> UploadOutcome {
    let Some(host) = descriptor.url.host_str() else {
        return UploadOutcome::Failed("transfer URL omitted host".to_string());
    };
    let mut builder = reqwest::Client::builder()
        .connect_timeout(TRANSFER_CONNECT_TIMEOUT)
        .timeout(TRANSFER_TIMEOUT)
        .no_proxy()
        .redirect(Policy::none());
    builder = builder.resolve_to_addrs(host, &descriptor.addresses);
    let client = match build_reqwest_client_with_custom_ca(builder) {
        Ok(client) => client,
        Err(_) => return UploadOutcome::Failed("failed to build transfer client".to_string()),
    };
    let size = bytes.len() as u64;
    let body_stream = stream::unfold((bytes, 0usize), |(bytes, offset)| async move {
        if offset >= bytes.len() {
            return None;
        }
        let end = (offset + 64 * 1024).min(bytes.len());
        let chunk = bytes::Bytes::copy_from_slice(&bytes[offset..end]);
        Some((Ok::<_, std::convert::Infallible>(chunk), (bytes, end)))
    });
    let request = match client
        .request(Method::PUT, descriptor.url)
        .headers(descriptor.headers)
        .header(CONTENT_LENGTH, size)
        .body(reqwest::Body::wrap_stream(body_stream))
        .build()
    {
        Ok(request) => request,
        Err(_) => return UploadOutcome::Failed("failed to build transfer request".to_string()),
    };
    let response = tokio::select! {
        _ = cancellation.cancelled() => {
            return UploadOutcome::CompletionUnknown(
                "upload cancellation was requested after dispatch".to_string(),
            );
        }
        response = client.execute(request) => response,
    };
    let response = match response {
        Ok(response) => response,
        Err(_) => {
            return UploadOutcome::CompletionUnknown(
                "upload completion could not be confirmed".to_string(),
            );
        }
    };
    if response.status().is_success() {
        UploadOutcome::Succeeded
    } else {
        UploadOutcome::CompletionUnknown(format!(
            "transfer returned HTTP {}",
            response.status().as_u16()
        ))
    }
}

fn validate_expiry(expires_at_unix_seconds: i64) -> Result<(), JSONRPCErrorError> {
    let now = unix_seconds(SystemTime::now());
    let minimum = now.saturating_add(MIN_DESCRIPTOR_LIFETIME.as_secs() as i64);
    let maximum = now.saturating_add(MAX_DESCRIPTOR_LIFETIME.as_secs() as i64);
    if expires_at_unix_seconds < minimum || expires_at_unix_seconds > maximum {
        return Err(invalid_params(
            "file transfer descriptor expiry must be between 30 seconds and 10 minutes".to_string(),
        ));
    }
    Ok(())
}

fn validate_headers(
    headers: Vec<FileTransferHeader>,
    azure_blob: bool,
) -> Result<HeaderMap, JSONRPCErrorError> {
    if headers.len() > MAX_DESCRIPTOR_HEADERS {
        return Err(invalid_params(
            "file transfer descriptor has too many headers".to_string(),
        ));
    }
    let mut header_bytes = 0usize;
    let mut result = HeaderMap::new();
    for header in headers {
        header_bytes = header_bytes.saturating_add(header.name.len() + header.value.len());
        if header_bytes > MAX_DESCRIPTOR_HEADER_BYTES {
            return Err(invalid_params(
                "file transfer descriptor headers exceed the size limit".to_string(),
            ));
        }
        let name = HeaderName::from_bytes(header.name.as_bytes())
            .map_err(|_| invalid_params("file transfer header name is invalid".to_string()))?;
        if !is_allowed_header(&name) {
            return Err(invalid_params(format!(
                "file transfer header `{name}` is not permitted"
            )));
        }
        if result.contains_key(&name) {
            return Err(invalid_params(format!(
                "file transfer header `{name}` must not be repeated"
            )));
        }
        let value = HeaderValue::from_str(&header.value)
            .map_err(|_| invalid_params("file transfer header value is invalid".to_string()))?;
        if name == "x-ms-blob-type" && value != "BlockBlob" {
            return Err(invalid_params(
                "file transfer x-ms-blob-type must be BlockBlob".to_string(),
            ));
        }
        result.insert(name, value);
    }
    if azure_blob && !result.contains_key("x-ms-blob-type") {
        result.insert("x-ms-blob-type", HeaderValue::from_static("BlockBlob"));
    }
    Ok(result)
}

fn is_allowed_header(name: &HeaderName) -> bool {
    matches!(
        name.as_str(),
        "content-type" | "content-disposition" | "x-ms-blob-type" | "x-ms-date" | "x-ms-version"
    ) || name.as_str().starts_with("x-ms-meta-")
}

fn is_trusted_transfer_host(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    host.ends_with(".blob.core.windows.net") || host.ends_with(".oaiusercontent.com")
}

fn is_local_development_url(url: &Url) -> bool {
    cfg!(debug_assertions)
        && url.scheme() == "http"
        && url
            .host_str()
            .is_some_and(|host| matches!(host, "localhost" | "127.0.0.1" | "::1"))
}

fn is_disallowed_transfer_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => {
            let octets = address.octets();
            address.is_private()
                || address.is_link_local()
                || address.is_loopback()
                || address.is_unspecified()
                || address.is_multicast()
                || address.is_broadcast()
                || address.is_documentation()
                || octets[0] == 0
                || (octets[0] == 100 && (64..=127).contains(&octets[1]))
                || (octets[0] == 192 && octets[1] == 0 && octets[2] == 0)
                || (octets[0] == 198 && (18..=19).contains(&octets[1]))
                || octets[0] >= 240
        }
        IpAddr::V6(address) => {
            let segments = address.segments();
            address.is_loopback()
                || address.is_unspecified()
                || address.is_multicast()
                || (segments[0] & 0xfe00) == 0xfc00
                || (segments[0] & 0xffc0) == 0xfe80
                || (segments[0] == 0x2001 && segments[1] == 0x0db8)
                || address
                    .to_ipv4_mapped()
                    .is_some_and(|address| is_disallowed_transfer_address(IpAddr::V4(address)))
        }
    }
}

fn unix_seconds(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}
