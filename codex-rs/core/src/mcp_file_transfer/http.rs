use std::net::IpAddr;
use std::time::Duration;

use codex_network_proxy::NetworkProxy;
use futures::StreamExt;
use reqwest::Method;
use reqwest::header::ACCEPT;
use reqwest::header::CONTENT_LENGTH;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderValue;
use reqwest::header::USER_AGENT;
use reqwest::redirect::Policy;
use tokio::io::AsyncWriteExt;
use url::Url;

use crate::session::session::Session;

const TRANSFER_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(5 * 60);

pub(super) async fn download_transfer_file(
    sess: &Session,
    transfer: &codex_mcp::FileTransferDescriptor,
    output_path: &std::path::Path,
    max_size: u64,
) -> Result<u64, String> {
    let url = validated_transfer_descriptor(transfer, "GET")?;
    let response = transfer_client(sess, &url)
        .await?
        .get(url)
        .send()
        .await
        .map_err(|_| "MCP download transfer request failed".to_string())?;
    let status = response.status();
    let response = response
        .error_for_status()
        .map_err(|_| format!("MCP download transfer returned HTTP {status}"))?;
    if response
        .content_length()
        .is_some_and(|size| size > max_size)
    {
        return Err(format!("MCP download exceeds the {max_size}-byte limit"));
    }
    let temporary_path = output_path.with_extension("part");
    let result = async {
        let mut output = tokio::fs::File::create(&temporary_path)
            .await
            .map_err(|error| format!("failed to create MCP download: {error}"))?;
        let mut size = 0_u64;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|error| format!("failed to read MCP download: {error}"))?;
            size = size.saturating_add(chunk.len() as u64);
            if size > max_size {
                return Err(format!("MCP download exceeds the {max_size}-byte limit"));
            }
            output
                .write_all(&chunk)
                .await
                .map_err(|error| format!("failed to write MCP download: {error}"))?;
        }
        output
            .flush()
            .await
            .map_err(|error| format!("failed to flush MCP download: {error}"))?;
        drop(output);
        tokio::fs::rename(&temporary_path, output_path)
            .await
            .map_err(|error| format!("failed to finalize MCP download: {error}"))?;
        Ok(size)
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(&temporary_path).await;
    }
    result
}

pub(super) async fn put_transfer_file(
    sess: &Session,
    transfer: &codex_mcp::FileTransferDescriptor,
    bytes: Vec<u8>,
    max_size: u64,
) -> Result<(), String> {
    let url = validated_upload_transfer_descriptor(transfer)?;
    let method = Method::from_bytes(transfer.method.as_bytes())
        .map_err(|error| format!("invalid MCP transfer method: {error}"))?;
    let size = bytes.len() as u64;
    if size > max_size {
        return Err(format!("MCP upload exceeds the {max_size}-byte limit"));
    }
    let stream = futures::stream::once(async move { Ok::<_, std::io::Error>(bytes) });
    let azure_blob_upload = url.host_str().is_some_and(|host| {
        host.ends_with(".blob.core.windows.net") || host.ends_with(".oaiusercontent.com")
    });
    let mut request = transfer_client(sess, &url)
        .await?
        .request(method, url)
        .header(CONTENT_LENGTH, size)
        .body(reqwest::Body::wrap_stream(stream));
    if azure_blob_upload {
        request = request.header("x-ms-blob-type", "BlockBlob");
    }
    let response = request
        .send()
        .await
        .map_err(|_| "MCP upload transfer request failed".to_string())?;
    let status = response.status();
    response
        .error_for_status()
        .map_err(|_| format!("MCP upload transfer returned HTTP {status}"))?;
    Ok(())
}

pub(super) fn validated_transfer_descriptor(
    transfer: &codex_mcp::FileTransferDescriptor,
    expected_method: &str,
) -> Result<Url, String> {
    if transfer
        .transport
        .as_deref()
        .is_some_and(|value| value != "https")
    {
        return Err("MCP transfer transport must be HTTPS".to_string());
    }
    if transfer.method != expected_method {
        return Err(format!("MCP transfer method must be {expected_method}"));
    }
    if let Some(expires_at) = transfer.expires_at.as_deref() {
        let expires_at = chrono::DateTime::parse_from_rfc3339(expires_at)
            .map_err(|error| format!("invalid MCP transfer expiry: {error}"))?;
        if expires_at <= chrono::Utc::now() {
            return Err("MCP transfer descriptor has expired".to_string());
        }
    }
    validated_transfer_url(&transfer.url)
}

fn validated_upload_transfer_descriptor(
    transfer: &codex_mcp::FileTransferDescriptor,
) -> Result<Url, String> {
    if !matches!(transfer.method.as_str(), "PUT" | "POST") {
        return Err("MCP upload transfer method must be PUT or POST".to_string());
    }
    validated_transfer_descriptor(transfer, &transfer.method)
}

fn validated_transfer_url(url: &str) -> Result<Url, String> {
    let url = Url::parse(url).map_err(|error| format!("invalid MCP transfer URL: {error}"))?;
    let local_http = is_local_test_url(&url);
    if url.scheme() != "https" && !local_http {
        return Err("MCP transfer URL must use HTTPS".to_string());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("MCP transfer URL must not contain credentials".to_string());
    }
    if url
        .host()
        .and_then(|host| match host {
            url::Host::Ipv4(address) => Some(IpAddr::V4(address)),
            url::Host::Ipv6(address) => Some(IpAddr::V6(address)),
            url::Host::Domain(_) => None,
        })
        .is_some_and(is_disallowed_transfer_address)
        && !local_http
    {
        return Err("MCP transfer URL must not target a private address".to_string());
    }
    Ok(url)
}

async fn transfer_client(sess: &Session, url: &Url) -> Result<reqwest::Client, String> {
    let network = sess
        .services
        .network_proxy
        .load_full()
        .map_or(TransferNetwork::Direct, |started| {
            TransferNetwork::Managed(started.proxy())
        });
    build_transfer_client(url, network).await
}

enum TransferNetwork {
    Managed(NetworkProxy),
    Direct,
}

async fn build_transfer_client(
    url: &Url,
    network: TransferNetwork,
) -> Result<reqwest::Client, String> {
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("*/*"));
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static("Mozilla/5.0 Codex MCP File Transfer"),
    );
    let mut builder = reqwest::Client::builder()
        .default_headers(headers)
        .connect_timeout(TRANSFER_CONNECT_TIMEOUT)
        .timeout(TRANSFER_TIMEOUT)
        .no_proxy()
        .redirect(Policy::none());
    match network {
        TransferNetwork::Managed(proxy) => {
            let proxy_url = format!("http://{}", proxy.http_addr());
            let proxy = reqwest::Proxy::all(&proxy_url)
                .map_err(|error| format!("failed to configure MCP transfer proxy: {error}"))?;
            builder = builder.proxy(proxy);
        }
        TransferNetwork::Direct => {
            builder = pin_public_transfer_addresses(builder, url).await?;
        }
    }
    builder
        .build()
        .map_err(|error| format!("failed to build MCP transfer client: {error}"))
}

async fn pin_public_transfer_addresses(
    mut builder: reqwest::ClientBuilder,
    url: &Url,
) -> Result<reqwest::ClientBuilder, String> {
    let Some(host) = url.host_str() else {
        return Err("MCP transfer URL must contain a host".to_string());
    };
    if is_local_test_url(url) {
        return Ok(builder);
    }
    if !is_trusted_direct_transfer_host(host) {
        return Err(
            "MCP transfer URL requires the managed network proxy or a trusted transfer host"
                .to_string(),
        );
    }
    let port = url
        .port_or_known_default()
        .ok_or_else(|| "MCP transfer URL must contain a valid port".to_string())?;
    let addresses = tokio::net::lookup_host((host, port))
        .await
        .map_err(|_| "failed to resolve MCP transfer host".to_string())?
        .collect::<Vec<_>>();
    if addresses.is_empty() {
        return Err("MCP transfer host did not resolve".to_string());
    }
    if addresses
        .iter()
        .any(|address| is_disallowed_transfer_address(address.ip()))
    {
        return Err("MCP transfer URL must not target a private address".to_string());
    }
    if matches!(url.host(), Some(url::Host::Domain(_))) {
        builder = builder.resolve_to_addrs(host, &addresses);
    }
    Ok(builder)
}

pub(super) fn is_trusted_direct_transfer_host(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    host.ends_with(".blob.core.windows.net") || host.ends_with(".oaiusercontent.com")
}

fn is_local_test_url(url: &Url) -> bool {
    cfg!(test)
        && url.scheme() == "http"
        && url
            .host_str()
            .is_some_and(|host| host == "localhost" || host == "127.0.0.1" || host == "::1")
}

pub(super) fn is_disallowed_transfer_address(address: IpAddr) -> bool {
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
