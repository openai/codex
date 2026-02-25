use crate::certs::ManagedMitmCa;
use crate::config::NetworkMode;
use crate::policy::normalize_host;
use crate::reasons::REASON_METHOD_NOT_ALLOWED;
use crate::responses::blocked_text_response;
use crate::responses::text_response;
use crate::runtime::HostBlockDecision;
use crate::runtime::HostBlockReason;
use crate::state::BlockedRequest;
use crate::state::BlockedRequestArgs;
use crate::state::NetworkProxyState;
use crate::upstream::UpstreamClient;
use anyhow::Context as _;
use anyhow::Result;
use anyhow::anyhow;
use rama_core::Layer;
use rama_core::Service;
use rama_core::bytes::Bytes;
use rama_core::error::BoxError;
use rama_core::extensions::ExtensionsRef;
use rama_core::futures::stream::Stream;
use rama_core::rt::Executor;
use rama_core::service::service_fn;
use rama_http::Body;
use rama_http::BodyDataStream;
use rama_http::HeaderValue;
use rama_http::Request;
use rama_http::Response;
use rama_http::StatusCode;
use rama_http::Uri;
use rama_http::header::HOST;
use rama_http::layer::remove_header::RemoveRequestHeaderLayer;
use rama_http::layer::remove_header::RemoveResponseHeaderLayer;
use rama_http_backend::server::HttpServer;
use rama_http_backend::server::layer::upgrade::Upgraded;
use rama_net::proxy::ProxyTarget;
use rama_net::stream::SocketInfo;
use rama_tls_rustls::server::TlsAcceptorData;
use rama_tls_rustls::server::TlsAcceptorLayer;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Context as TaskContext;
use std::task::Poll;
use tracing::info;
use tracing::warn;

/// State needed to terminate a CONNECT tunnel and enforce policy on inner HTTPS requests.
pub struct MitmState {
    ca: ManagedMitmCa,
    upstream: UpstreamClient,
    inspect: bool,
    max_body_bytes: usize,
}

#[derive(Clone)]
struct MitmPolicyContext {
    target_host: String,
    target_port: u16,
    mode: NetworkMode,
    app_state: Arc<NetworkProxyState>,
}

#[derive(Clone)]
struct MitmRequestContext {
    policy: MitmPolicyContext,
    mitm: Arc<MitmState>,
}

const MITM_INSPECT_BODIES: bool = false;
const MITM_MAX_BODY_BYTES: usize = 4096;

impl std::fmt::Debug for MitmState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Avoid dumping internal state (CA material, connectors, etc.) to logs.
        f.debug_struct("MitmState")
            .field("inspect", &self.inspect)
            .field("max_body_bytes", &self.max_body_bytes)
            .finish_non_exhaustive()
    }
}

impl MitmState {
    pub(crate) fn new(allow_upstream_proxy: bool) -> Result<Self> {
        // MITM exists to make limited-mode HTTPS enforceable: once CONNECT is established, plain
        // proxying would lose visibility into the inner HTTP request. We generate/load a local CA
        // and issue per-host leaf certs so we can terminate TLS and apply policy.
        let ca = ManagedMitmCa::load_or_create()?;

        let upstream = if allow_upstream_proxy {
            UpstreamClient::from_env_proxy()
        } else {
            UpstreamClient::direct()
        };

        Ok(Self {
            ca,
            upstream,
            inspect: MITM_INSPECT_BODIES,
            max_body_bytes: MITM_MAX_BODY_BYTES,
        })
    }

    fn tls_acceptor_data_for_host(&self, host: &str) -> Result<TlsAcceptorData> {
        self.ca.tls_acceptor_data_for_host(host)
    }

    pub(crate) fn inspect_enabled(&self) -> bool {
        self.inspect
    }

    pub(crate) fn max_body_bytes(&self) -> usize {
        self.max_body_bytes
    }
}

/// Terminate the upgraded CONNECT stream with a generated leaf cert and proxy inner HTTPS traffic.
pub(crate) async fn mitm_tunnel(upgraded: Upgraded) -> Result<()> {
    let mitm = upgraded
        .extensions()
        .get::<Arc<MitmState>>()
        .cloned()
        .context("missing MITM state")?;
    let app_state = upgraded
        .extensions()
        .get::<Arc<NetworkProxyState>>()
        .cloned()
        .context("missing app state")?;
    let target = upgraded
        .extensions()
        .get::<ProxyTarget>()
        .context("missing proxy target")?
        .0
        .clone();
    let target_host = normalize_host(&target.host.to_string());
    let target_port = target.port;
    let acceptor_data = mitm.tls_acceptor_data_for_host(&target_host)?;
    let mode = upgraded
        .extensions()
        .get::<NetworkMode>()
        .copied()
        .unwrap_or(NetworkMode::Full);
    let request_ctx = Arc::new(MitmRequestContext {
        policy: MitmPolicyContext {
            target_host,
            target_port,
            mode,
            app_state,
        },
        mitm,
    });

    let executor = upgraded
        .extensions()
        .get::<Executor>()
        .cloned()
        .unwrap_or_default();

    let http_service = HttpServer::auto(executor).service(
        (
            RemoveResponseHeaderLayer::hop_by_hop(),
            RemoveRequestHeaderLayer::hop_by_hop(),
        )
            .into_layer(service_fn({
                let request_ctx = request_ctx.clone();
                move |req| {
                    let request_ctx = request_ctx.clone();
                    async move { handle_mitm_request(req, request_ctx).await }
                }
            })),
    );

    let https_service = TlsAcceptorLayer::new(acceptor_data)
        .with_store_client_hello(true)
        .into_layer(http_service);

    https_service
        .serve(upgraded)
        .await
        .map_err(|err| anyhow!("MITM serve error: {err}"))?;
    Ok(())
}

async fn handle_mitm_request(
    req: Request,
    request_ctx: Arc<MitmRequestContext>,
) -> Result<Response, std::convert::Infallible> {
    let response = match forward_request(req, &request_ctx).await {
        Ok(resp) => resp,
        Err(err) => {
            warn!("MITM request handling failed: {err}");
            text_response(StatusCode::BAD_GATEWAY, "mitm upstream error")
        }
    };
    Ok(response)
}

async fn forward_request(req: Request, request_ctx: &MitmRequestContext) -> Result<Response> {
    if let Some(response) = mitm_blocking_response(&req, &request_ctx.policy).await? {
        return Ok(response);
    }

    let target_host = request_ctx.policy.target_host.clone();
    let target_port = request_ctx.policy.target_port;
    let mitm = request_ctx.mitm.clone();

    let method = req.method().as_str().to_string();
    let path = path_and_query(req.uri());
    let log_path = path_for_log(req.uri());

    let (mut parts, body) = req.into_parts();
    let authority = authority_header_value(&target_host, target_port);
    parts.uri = build_https_uri(&authority, &path)?;
    parts
        .headers
        .insert(HOST, HeaderValue::from_str(&authority)?);

    let inspect = mitm.inspect_enabled();
    let max_body_bytes = mitm.max_body_bytes();
    let body = if inspect {
        inspect_body(
            body,
            max_body_bytes,
            RequestLogContext {
                host: authority.clone(),
                method: method.clone(),
                path: log_path.clone(),
            },
        )
    } else {
        body
    };

    let upstream_req = Request::from_parts(parts, body);
    let upstream_resp = mitm.upstream.serve(upstream_req).await?;
    respond_with_inspection(
        upstream_resp,
        inspect,
        max_body_bytes,
        &method,
        &log_path,
        &authority,
    )
}

async fn mitm_blocking_response(
    req: &Request,
    policy: &MitmPolicyContext,
) -> Result<Option<Response>> {
    if req.method().as_str() == "CONNECT" {
        return Ok(Some(text_response(
            StatusCode::METHOD_NOT_ALLOWED,
            "CONNECT not supported inside MITM",
        )));
    }

    let method = req.method().as_str().to_string();
    let log_path = path_for_log(req.uri());
    let client = req
        .extensions()
        .get::<SocketInfo>()
        .map(|info| info.peer_addr().to_string());

    if let Some(request_host) = extract_request_host(req) {
        let normalized = normalize_host(&request_host);
        if !normalized.is_empty() && normalized != policy.target_host {
            warn!(
                "MITM host mismatch (target={}, request_host={normalized})",
                policy.target_host
            );
            return Ok(Some(text_response(
                StatusCode::BAD_REQUEST,
                "host mismatch",
            )));
        }
    }

    // CONNECT already handled allowlist/denylist + decider policy. Re-check local/private
    // resolution here to defend against DNS rebinding between CONNECT and inner HTTPS requests.
    if matches!(
        policy
            .app_state
            .host_blocked(&policy.target_host, policy.target_port)
            .await?,
        HostBlockDecision::Blocked(HostBlockReason::NotAllowedLocal)
    ) {
        let reason = HostBlockReason::NotAllowedLocal.as_str();
        let _ = policy
            .app_state
            .record_blocked(BlockedRequest::new(BlockedRequestArgs {
                host: policy.target_host.clone(),
                reason: reason.to_string(),
                client: client.clone(),
                method: Some(method.clone()),
                mode: Some(policy.mode),
                protocol: "https".to_string(),
                decision: None,
                source: None,
                port: Some(policy.target_port),
            }))
            .await;
        warn!(
            "MITM blocked local/private target after CONNECT (host={}, port={}, method={method}, path={log_path})",
            policy.target_host, policy.target_port
        );
        return Ok(Some(blocked_text_response(reason)));
    }

    if !policy.mode.allows_method(&method) {
        let _ = policy
            .app_state
            .record_blocked(BlockedRequest::new(BlockedRequestArgs {
                host: policy.target_host.clone(),
                reason: REASON_METHOD_NOT_ALLOWED.to_string(),
                client: client.clone(),
                method: Some(method.clone()),
                mode: Some(policy.mode),
                protocol: "https".to_string(),
                decision: None,
                source: None,
                port: Some(policy.target_port),
            }))
            .await;
        warn!(
            "MITM blocked by method policy (host={}, method={method}, path={log_path}, mode={:?}, allowed_methods=GET, HEAD, OPTIONS)",
            policy.target_host, policy.mode
        );
        return Ok(Some(blocked_text_response(REASON_METHOD_NOT_ALLOWED)));
    }

    Ok(None)
}

fn respond_with_inspection(
    resp: Response,
    inspect: bool,
    max_body_bytes: usize,
    method: &str,
    log_path: &str,
    authority: &str,
) -> Result<Response> {
    if !inspect {
        return Ok(resp);
    }

    let (parts, body) = resp.into_parts();
    let body = inspect_body(
        body,
        max_body_bytes,
        ResponseLogContext {
            host: authority.to_string(),
            method: method.to_string(),
            path: log_path.to_string(),
            status: parts.status,
        },
    );
    Ok(Response::from_parts(parts, body))
}

fn inspect_body<T: BodyLoggable + Send + 'static>(
    body: Body,
    max_body_bytes: usize,
    ctx: T,
) -> Body {
    Body::from_stream(InspectStream {
        inner: Box::pin(body.into_data_stream()),
        ctx: Some(Box::new(ctx)),
        len: 0,
        max_body_bytes,
    })
}

struct InspectStream<T> {
    inner: Pin<Box<BodyDataStream>>,
    ctx: Option<Box<T>>,
    len: usize,
    max_body_bytes: usize,
}

impl<T: BodyLoggable> Stream for InspectStream<T> {
    type Item = Result<Bytes, BoxError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        match this.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => {
                this.len = this.len.saturating_add(bytes.len());
                Poll::Ready(Some(Ok(bytes)))
            }
            Poll::Ready(Some(Err(err))) => Poll::Ready(Some(Err(err))),
            Poll::Ready(None) => {
                if let Some(ctx) = this.ctx.take() {
                    ctx.log(this.len, this.len > this.max_body_bytes);
                }
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

struct RequestLogContext {
    host: String,
    method: String,
    path: String,
}

struct ResponseLogContext {
    host: String,
    method: String,
    path: String,
    status: StatusCode,
}

trait BodyLoggable {
    fn log(self, len: usize, truncated: bool);
}

impl BodyLoggable for RequestLogContext {
    fn log(self, len: usize, truncated: bool) {
        let host = self.host;
        let method = self.method;
        let path = self.path;
        info!(
            "MITM inspected request body (host={host}, method={method}, path={path}, body_len={len}, truncated={truncated})"
        );
    }
}

impl BodyLoggable for ResponseLogContext {
    fn log(self, len: usize, truncated: bool) {
        let host = self.host;
        let method = self.method;
        let path = self.path;
        let status = self.status;
        info!(
            "MITM inspected response body (host={host}, method={method}, path={path}, status={status}, body_len={len}, truncated={truncated})"
        );
    }
}

fn extract_request_host(req: &Request) -> Option<String> {
    req.headers()
        .get(HOST)
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string)
        .or_else(|| req.uri().authority().map(|a| a.as_str().to_string()))
}

fn authority_header_value(host: &str, port: u16) -> String {
    // Host header / URI authority formatting.
    if host.contains(':') {
        if port == 443 {
            format!("[{host}]")
        } else {
            format!("[{host}]:{port}")
        }
    } else if port == 443 {
        host.to_string()
    } else {
        format!("{host}:{port}")
    }
}

fn build_https_uri(authority: &str, path: &str) -> Result<Uri> {
    let target = format!("https://{authority}{path}");
    Ok(target.parse()?)
}

fn path_and_query(uri: &Uri) -> String {
    uri.path_and_query()
        .map(rama_http::uri::PathAndQuery::as_str)
        .unwrap_or("/")
        .to_string()
}

fn path_for_log(uri: &Uri) -> String {
    uri.path().to_string()
}

#[cfg(test)]
#[path = "mitm_tests.rs"]
mod tests;
