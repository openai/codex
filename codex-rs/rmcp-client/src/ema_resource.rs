//! Bound resource discovery and its HTTP safety policy before enterprise credential use.
//! The returned client keeps one deadline across resource and authorization-server discovery.

use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use codex_exec_server::HttpClient;
use codex_network_proxy::is_non_public_ip;
use codex_protocol::mcp::ENTERPRISE_MANAGED_AUTHORIZATION_EXTENSION_ID;
use futures::StreamExt;
use http::Method;
use http::StatusCode;
use http::header::ACCEPT;
use http::header::CONTENT_TYPE;
use http::header::LOCATION;
use http::header::WWW_AUTHENTICATE;
use rmcp::model::ClientCapabilities;
use rmcp::model::ClientJsonRpcMessage;
use rmcp::model::ClientRequest;
use rmcp::model::DiscoverRequest;
use rmcp::model::DiscoverRequestParams;
use rmcp::model::ErrorCode;
use rmcp::model::Implementation;
use rmcp::model::InitializeRequest;
use rmcp::model::InitializeRequestParams;
use rmcp::model::JsonRpcMessage;
use rmcp::model::JsonRpcRequest;
use rmcp::model::ProtocolVersion;
use rmcp::model::RequestId;
use rmcp::model::RequestMetaObject;
use rmcp::model::ServerJsonRpcMessage;
use rmcp::transport::auth::OAuthHttpRedirectPolicy;
use rmcp::transport::auth::WWWAuthenticateParams;
use rmcp::transport::common::http_header::HEADER_MCP_METHOD;
use rmcp::transport::common::http_header::HEADER_MCP_PROTOCOL_VERSION;
use serde::Deserialize;
use sse_stream::SseStream;
use url::Host;
use url::Url;

use crate::ema_auth_policy::validate_ema_auth_resource;
use crate::ema_auth_policy::validate_ema_oauth_endpoint;
use crate::http_client_adapter::StreamableHttpRedirectMode;
use crate::http_client_adapter::legacy_discovery_fallback_response;
use crate::oauth_http_client::OAuthHttpClientAdapter;
use crate::utils::build_default_headers;

const MAX_EMA_AUTHORIZATION_SERVERS: usize = 16;
const MAX_EMA_DISCOVERY_REDIRECTS: usize = 10;
const EMA_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Deserialize)]
struct EmaResourceMetadata {
    resource: String,
    authorization_server: Option<String>,
    #[serde(default)]
    authorization_servers: Vec<String>,
}

/// EMA discovery retains its own resource/issuer context while sharing the HTTP safety policy.
pub(super) struct EmaDiscoveryClient {
    inner: OAuthHttpClientAdapter,
    deadline: Instant,
}

impl EmaDiscoveryClient {
    pub(super) async fn get(&self, url: Url) -> Result<oauth2::HttpResponse> {
        self.execute(
            oauth2::http::Request::builder()
                .uri(url.as_str())
                .body(Vec::new())?,
        )
        .await
    }

    async fn post(
        &self,
        url: &Url,
        request: &JsonRpcRequest<ClientRequest>,
        version: ProtocolVersion,
    ) -> Result<oauth2::HttpResponse> {
        self.execute(
            oauth2::http::Request::builder()
                .method(Method::POST)
                .uri(url.as_str())
                .header(ACCEPT, "application/json, text/event-stream")
                .header(CONTENT_TYPE, "application/json")
                .header(HEADER_MCP_PROTOCOL_VERSION, version.as_str())
                .header(HEADER_MCP_METHOD, request.request.method())
                .body(serde_json::to_vec(request)?)?,
        )
        .await
    }

    async fn execute(&self, request: oauth2::HttpRequest) -> Result<oauth2::HttpResponse> {
        let (parts, body) = request.into_parts();
        let mut url = Url::parse(&parts.uri.to_string())?;
        for _ in 0..MAX_EMA_DISCOVERY_REDIRECTS {
            validate_ema_oauth_endpoint(url.as_str(), "enterprise metadata URL")?;
            let remaining = self
                .deadline
                .checked_duration_since(Instant::now())
                .ok_or_else(|| anyhow!("enterprise metadata discovery timed out"))?;
            let mut request = oauth2::http::Request::builder()
                .method(parts.method.clone())
                .uri(url.as_str())
                .body(body.clone())?;
            *request.headers_mut() = parts.headers.clone();
            let response = self
                .inner
                .execute_request(request, OAuthHttpRedirectPolicy::Stop, Some(remaining))
                .await
                .map_err(|error| {
                    anyhow!("enterprise metadata discovery request failed: {error}")
                })?;
            if response.status().is_server_error()
                || matches!(
                    response.status(),
                    StatusCode::REQUEST_TIMEOUT
                        | StatusCode::TOO_EARLY
                        | StatusCode::TOO_MANY_REQUESTS
                )
            {
                bail!(
                    "enterprise metadata discovery returned HTTP {}",
                    response.status()
                );
            }
            if !response.status().is_redirection() || parts.method != Method::GET {
                return Ok(response);
            }
            let Some(location) = response.headers().get(LOCATION) else {
                return Ok(response);
            };
            let next = url.join(location.to_str()?)?;
            if next.origin() != url.origin() {
                bail!("enterprise metadata discovery refused a cross-origin redirect");
            }
            url = next;
        }
        bail!("enterprise metadata discovery exceeded its redirect limit")
    }
}

// Discovery responses use the same JSON or SSE framing as ordinary MCP requests.
// The HTTP adapter has already bounded this body; only a terminal RPC message matters.
async fn discovery_response_message(
    response: &oauth2::HttpResponse,
) -> Option<ServerJsonRpcMessage> {
    if !response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("text/event-stream"))
    {
        return serde_json::from_slice(response.body()).ok();
    }
    let mut events =
        SseStream::from_bytes_stream(futures::stream::iter([Ok::<_, std::convert::Infallible>(
            response.body().as_slice(),
        )]));
    while let Some(event) = events.next().await {
        let event = event.ok()?;
        if !matches!(event.event.as_deref(), None | Some("") | Some("message")) {
            continue;
        }
        let Some(data) = event.data.filter(|data| !data.trim().is_empty()) else {
            continue;
        };
        let message = serde_json::from_str(&data).ok()?;
        if matches!(
            message,
            JsonRpcMessage::Response(_) | JsonRpcMessage::Error(_)
        ) {
            return Some(message);
        }
    }
    None
}

/// Retain rmcp's denial of metadata endpoints on private addresses and cloud-metadata hosts.
pub(super) fn validate_ema_metadata_issuer(resource: &Url, issuer: &Url) -> Result<()> {
    validate_ema_oauth_endpoint(issuer.as_str(), "enterprise authorization server issuer")?;
    if issuer.query().is_some() {
        bail!("enterprise authorization server issuer must not contain a query");
    }
    let loopback = |url: &Url| match url.host() {
        Some(Host::Domain(host)) => {
            let host = host.trim_end_matches('.');
            host == "localhost" || host.ends_with(".localhost")
        }
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    };
    if loopback(resource) && loopback(issuer) {
        return Ok(());
    }
    let disallowed = match issuer.host() {
        Some(Host::Domain(host)) => {
            let host = host.trim_end_matches('.').to_ascii_lowercase();
            matches!(
                host.as_str(),
                "localhost" | "metadata" | "metadata.google.internal" | "metadata.azure.internal"
            ) || host.ends_with(".localhost")
        }
        Some(Host::Ipv4(ip)) => is_non_public_ip(IpAddr::V4(ip)),
        Some(Host::Ipv6(ip)) => is_non_public_ip(IpAddr::V6(ip)),
        None => true,
    };
    if disallowed {
        bail!("enterprise discovery refused a private authorization-server metadata destination");
    }
    Ok(())
}

pub(super) struct EmaDiscoveryRequest<'a> {
    pub(super) server_url: &'a str,
    pub(super) resource: Option<&'a str>,
    pub(super) expected_issuer: Option<&'a str>,
    pub(super) http_client: Arc<dyn HttpClient>,
}

/// A validated resource and candidate list, with the discovery deadline still in force.
pub(super) struct EmaResource {
    pub(super) server_url: Url,
    pub(super) resource: String,
    pub(super) authorization_servers: Vec<String>,
    pub(super) client: EmaDiscoveryClient,
}

pub(super) async fn discover_ema_resource(request: EmaDiscoveryRequest<'_>) -> Result<EmaResource> {
    let client = EmaDiscoveryClient {
        inner: OAuthHttpClientAdapter::new_with_redirect_mode(
            request.http_client,
            build_default_headers(/*http_headers*/ None, /*env_http_headers*/ None)?,
            request.server_url,
            /*has_configured_headers*/ false,
            StreamableHttpRedirectMode::Legacy,
        )?,
        deadline: Instant::now() + EMA_DISCOVERY_TIMEOUT,
    };
    let server = Url::parse(request.server_url)?;
    let resource = Url::parse(request.resource.unwrap_or(request.server_url))?;
    let path = resource.path().trim_matches('/');
    let mut resource_urls = vec![server.clone()];
    let metadata_url = |path: &str| {
        let mut url = resource.clone();
        url.set_path(path);
        url
    };
    if !path.is_empty() {
        resource_urls.push(metadata_url(&format!(
            "/.well-known/oauth-protected-resource/{path}"
        )));
        resource_urls.push(metadata_url(&format!(
            "/{path}/.well-known/oauth-protected-resource"
        )));
    }
    resource_urls.push(metadata_url("/.well-known/oauth-protected-resource"));
    let mut resource_metadata = None;
    for url in resource_urls {
        let mut response = if url == server {
            // Modern MCP servers need not serve GET. Probe a read-only RPC to receive
            // their authentication challenge before resolving enterprise credentials.
            let mut discover = DiscoverRequest::new(DiscoverRequestParams {});
            discover
                .extensions
                .insert(RequestMetaObject::with_client_context(
                    ProtocolVersion::V_2026_07_28,
                    Implementation::new("codex", env!("CARGO_PKG_VERSION")),
                    ClientCapabilities::builder()
                        .enable_extensions_with(
                            [(
                                ENTERPRISE_MANAGED_AUTHORIZATION_EXTENSION_ID.to_string(),
                                Default::default(),
                            )]
                            .into(),
                        )
                        .build(),
                ));
            let discover = JsonRpcRequest::new(
                RequestId::String("ema-auth-discover".into()),
                discover.into(),
            );
            let mut response = client
                .post(&server, &discover, ProtocolVersion::V_2026_07_28)
                .await?;
            let legacy = response.status() != StatusCode::UNAUTHORIZED
                && match discovery_response_message(&response).await {
                    Some(message) => matches!(
                        legacy_discovery_fallback_response(
                            &ClientJsonRpcMessage::Request(discover),
                            message,
                            response.status() == StatusCode::BAD_REQUEST,
                        ),
                        JsonRpcMessage::Error(error) if error.error.code == ErrorCode::METHOD_NOT_FOUND
                    ),
                    None => matches!(
                        response.status(),
                        StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED
                    ),
                };
            if legacy {
                // Preserve legacy custom metadata challenges without exchanging credentials
                // or retrying application operations. All probes share the same deadline.
                response = client
                    .execute(
                        oauth2::http::Request::builder()
                            .uri(server.as_str())
                            .header(ACCEPT, "text/event-stream")
                            .body(Vec::new())?,
                    )
                    .await?;
                if matches!(
                    response.status(),
                    StatusCode::BAD_REQUEST
                        | StatusCode::NOT_FOUND
                        | StatusCode::METHOD_NOT_ALLOWED
                        | StatusCode::NOT_ACCEPTABLE
                ) {
                    response = client
                        .post(
                            &server,
                            &JsonRpcRequest::new(
                                RequestId::String("ema-auth-initialize".into()),
                                InitializeRequest::new(
                                    InitializeRequestParams::new(
                                        ClientCapabilities::default(),
                                        Implementation::new("codex", env!("CARGO_PKG_VERSION")),
                                    )
                                    .with_protocol_version(ProtocolVersion::V_2025_06_18),
                                )
                                .into(),
                            ),
                            ProtocolVersion::V_2025_06_18,
                        )
                        .await?;
                }
            }
            response
        } else {
            client.get(url).await?
        };
        if response.status() == StatusCode::UNAUTHORIZED {
            let challenge_url = response
                .headers()
                .get_all(WWW_AUTHENTICATE)
                .iter()
                .filter_map(|value| value.to_str().ok())
                .find_map(|value| {
                    WWWAuthenticateParams::parse(value, &server).resource_metadata_url
                });
            if let Some(url) = challenge_url {
                response = client.get(url).await?;
            }
        }
        if response.status() == StatusCode::OK
            && let Ok(metadata) = serde_json::from_slice::<EmaResourceMetadata>(response.body())
        {
            resource_metadata = Some(metadata);
            break;
        }
    }
    let Some(EmaResourceMetadata {
        resource,
        authorization_server,
        authorization_servers,
    }) = resource_metadata
    else {
        bail!("enterprise MCP protected-resource metadata is missing or invalid");
    };
    if resource.trim().is_empty() {
        bail!("enterprise MCP protected-resource identifier is empty");
    }
    validate_ema_auth_resource(request.server_url, Some(&resource))?;
    if request
        .resource
        .is_some_and(|expected| expected != resource)
    {
        bail!(
            "configured enterprise MCP resource does not match the RFC 9728 protected-resource metadata"
        );
    }
    let mut issuers = authorization_server
        .into_iter()
        .chain(authorization_servers)
        .collect::<Vec<_>>();
    if issuers.len() > MAX_EMA_AUTHORIZATION_SERVERS {
        bail!(
            "enterprise MCP protected-resource metadata advertises too many authorization servers"
        );
    }
    if let Some(expected) = request.expected_issuer {
        issuers.retain(|issuer| issuer == expected);
        if issuers.is_empty() {
            bail!("enterprise MCP authorization server issuer does not match configured issuer");
        }
    }
    Ok(EmaResource {
        server_url: server,
        resource,
        authorization_servers: issuers,
        client,
    })
}
