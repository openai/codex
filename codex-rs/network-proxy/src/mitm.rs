use crate::config::MitmConfig;
use crate::config::NetworkMode;
use crate::policy::method_allowed;
use crate::policy::normalize_host;
use crate::responses::blocked_text_response;
use crate::state::AppState;
use crate::state::BlockedRequest;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use rama::Context as RamaContext;
use rama::Layer;
use rama::Service;
use rama::bytes::Bytes;
use rama::error::BoxError;
use rama::error::OpaqueError;
use rama::futures::stream::Stream;
use rama::http::Body;
use rama::http::HeaderValue;
use rama::http::Request;
use rama::http::Response;
use rama::http::StatusCode;
use rama::http::Uri;
use rama::http::dep::http::uri::PathAndQuery;
use rama::http::header::HOST;
use rama::http::layer::remove_header::RemoveRequestHeaderLayer;
use rama::http::layer::remove_header::RemoveResponseHeaderLayer;
use rama::http::layer::upgrade::Upgraded;
use rama::http::server::HttpServer;
use rama::net::proxy::ProxyTarget;
use rama::net::stream::SocketInfo;
use rama::service::service_fn;
use rama::tls::rustls::dep::pemfile;
use rama::tls::rustls::server::TlsAcceptorData;
use rama::tls::rustls::server::TlsAcceptorDataBuilder;
use rama::tls::rustls::server::TlsAcceptorLayer;
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::BufReader;
use std::io::Write;
use std::net::IpAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Context as TaskContext;
use std::task::Poll;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use tracing::info;
use tracing::warn;

use rcgen_rama::BasicConstraints;
use rcgen_rama::CertificateParams;
use rcgen_rama::DistinguishedName;
use rcgen_rama::DnType;
use rcgen_rama::ExtendedKeyUsagePurpose;
use rcgen_rama::IsCa;
use rcgen_rama::Issuer;
use rcgen_rama::KeyPair;
use rcgen_rama::KeyUsagePurpose;
use rcgen_rama::SanType;

pub struct MitmState {
    issuer: Issuer<'static, KeyPair>,
    upstream: rama::service::BoxService<Arc<AppState>, Request, Response, OpaqueError>,
    inspect: bool,
    max_body_bytes: usize,
}

impl MitmState {
    pub fn new(cfg: &MitmConfig) -> Result<Self> {
        // MITM exists to make limited-mode HTTPS enforceable: once CONNECT is established, plain
        // proxying would lose visibility into the inner HTTP request. We generate/load a local CA
        // and issue per-host leaf certs so we can terminate TLS and apply policy.
        let (ca_cert_pem, ca_key_pem) = load_or_create_ca(cfg)?;
        let ca_key = KeyPair::from_pem(&ca_key_pem).context("failed to parse CA key")?;
        let issuer: Issuer<'static, KeyPair> =
            Issuer::from_ca_cert_pem(&ca_cert_pem, ca_key).context("failed to parse CA cert")?;

        let tls_config = rama::tls::rustls::client::TlsConnectorData::new_http_auto()
            .context("create upstream TLS config")?;
        let upstream = rama::http::client::EasyHttpWebClient::builder()
            // Use a direct transport connector (no upstream proxy) to avoid proxy loops.
            .with_default_transport_connector()
            .without_tls_proxy_support()
            .without_proxy_support()
            .with_tls_support_using_rustls(Some(tls_config))
            .build()
            .boxed();

        Ok(Self {
            issuer,
            upstream,
            inspect: cfg.inspect,
            max_body_bytes: cfg.max_body_bytes,
        })
    }

    fn tls_acceptor_data_for_host(&self, host: &str) -> Result<TlsAcceptorData> {
        let (cert_pem, key_pem) = issue_host_certificate_pem(host, &self.issuer)?;
        let cert_chain = pemfile::certs(&mut BufReader::new(cert_pem.as_bytes()))
            .collect::<Result<Vec<_>, _>>()
            .context("failed to parse host cert PEM")?;
        if cert_chain.is_empty() {
            return Err(anyhow!("no certificates found"));
        }

        let key_der = pemfile::private_key(&mut BufReader::new(key_pem.as_bytes()))
            .context("failed to parse host key PEM")?
            .context("no private key found")?;

        Ok(TlsAcceptorDataBuilder::new(cert_chain, key_der)
            .context("failed to build rustls acceptor config")?
            .with_alpn_protocols_http_auto()
            .build())
    }

    pub fn inspect_enabled(&self) -> bool {
        self.inspect
    }

    pub fn max_body_bytes(&self) -> usize {
        self.max_body_bytes
    }
}

pub async fn mitm_tunnel(
    mut ctx: RamaContext<Arc<AppState>>,
    upgraded: Upgraded,
    host: &str,
    _port: u16,
    mode: NetworkMode,
    state: Arc<MitmState>,
) -> Result<()> {
    // Ensure the MITM state is available for the per-request handler.
    ctx.insert(state.clone());
    ctx.insert(mode);

    let acceptor_data = state.tls_acceptor_data_for_host(host)?;
    let http_service = HttpServer::auto(ctx.executor().clone()).service(
        (
            RemoveResponseHeaderLayer::hop_by_hop(),
            RemoveRequestHeaderLayer::hop_by_hop(),
        )
            .into_layer(service_fn(handle_mitm_request)),
    );

    let https_service = TlsAcceptorLayer::new(acceptor_data)
        .with_store_client_hello(true)
        .into_layer(http_service);

    https_service
        .serve(ctx, upgraded)
        .await
        .map_err(|err| anyhow!("MITM serve error: {err}"))?;
    Ok(())
}

async fn handle_mitm_request(
    ctx: RamaContext<Arc<AppState>>,
    req: Request,
) -> Result<Response, std::convert::Infallible> {
    let response = match forward_request(ctx, req).await {
        Ok(resp) => resp,
        Err(err) => {
            warn!("MITM upstream request failed: {err}");
            text_response(StatusCode::BAD_GATEWAY, "mitm upstream error")
        }
    };
    Ok(response)
}

async fn forward_request(ctx: RamaContext<Arc<AppState>>, req: Request) -> Result<Response> {
    let target = ctx
        .get::<ProxyTarget>()
        .context("missing proxy target")?
        .0
        .clone();

    let target_host = normalize_host(&target.host().to_string());
    let target_port = target.port();
    let mode = ctx
        .get::<NetworkMode>()
        .copied()
        .unwrap_or(NetworkMode::Full);
    let mitm = ctx
        .get::<Arc<MitmState>>()
        .cloned()
        .context("missing MITM state")?;

    if req.method().as_str() == "CONNECT" {
        return Ok(text_response(
            StatusCode::METHOD_NOT_ALLOWED,
            "CONNECT not supported inside MITM",
        ));
    }

    let method = req.method().as_str().to_string();
    let path = path_and_query(req.uri());
    let client = ctx
        .get::<SocketInfo>()
        .map(|info| info.peer_addr().to_string());

    if let Some(request_host) = extract_request_host(&req) {
        let normalized = normalize_host(&request_host);
        if !normalized.is_empty() && normalized != target_host {
            warn!("MITM host mismatch (target={target_host}, request_host={normalized})");
            return Ok(text_response(StatusCode::BAD_REQUEST, "host mismatch"));
        }
    }

    if !method_allowed(mode, method.as_str()) {
        let _ = ctx
            .state()
            .record_blocked(BlockedRequest::new(
                target_host.clone(),
                "method_not_allowed".to_string(),
                client.clone(),
                Some(method.clone()),
                Some(NetworkMode::Limited),
                "https".to_string(),
            ))
            .await;
        warn!(
            "MITM blocked by method policy (host={target_host}, method={method}, path={path}, mode={mode:?}, allowed_methods=GET, HEAD, OPTIONS)"
        );
        return Ok(blocked_text("method_not_allowed"));
    }

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
                path: path.clone(),
            },
        )
    } else {
        body
    };

    let upstream_req = Request::from_parts(parts, body);
    let upstream_resp = mitm.upstream.serve(ctx, upstream_req).await?;
    respond_with_inspection(
        upstream_resp,
        inspect,
        max_body_bytes,
        &method,
        &path,
        &authority,
    )
}

fn respond_with_inspection(
    resp: Response,
    inspect: bool,
    max_body_bytes: usize,
    method: &str,
    path: &str,
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
            path: path.to_string(),
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
    inner: Pin<Box<rama::http::BodyDataStream>>,
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
        .map(PathAndQuery::as_str)
        .unwrap_or("/")
        .to_string()
}

fn issue_host_certificate_pem(
    host: &str,
    issuer: &Issuer<'_, KeyPair>,
) -> Result<(String, String)> {
    let mut params = if let Ok(ip) = host.parse::<IpAddr>() {
        let mut params = CertificateParams::new(Vec::new())
            .map_err(|err| anyhow!("failed to create cert params: {err}"))?;
        params.subject_alt_names.push(SanType::IpAddress(ip));
        params
    } else {
        CertificateParams::new(vec![host.to_string()])
            .map_err(|err| anyhow!("failed to create cert params: {err}"))?
    };

    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    params.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyEncipherment,
    ];

    let key_pair = KeyPair::generate_for(&rcgen_rama::PKCS_ECDSA_P256_SHA256)
        .map_err(|err| anyhow!("failed to generate host key pair: {err}"))?;
    let cert = params
        .signed_by(&key_pair, issuer)
        .map_err(|err| anyhow!("failed to sign host cert: {err}"))?;

    Ok((cert.pem(), key_pair.serialize_pem()))
}

fn load_or_create_ca(cfg: &MitmConfig) -> Result<(String, String)> {
    let cert_path = &cfg.ca_cert_path;
    let key_path = &cfg.ca_key_path;

    if cert_path.exists() || key_path.exists() {
        if !cert_path.exists() || !key_path.exists() {
            return Err(anyhow!("both ca_cert_path and ca_key_path must exist"));
        }
        let cert_pem = fs::read_to_string(cert_path)
            .with_context(|| format!("failed to read CA cert {}", cert_path.display()))?;
        let key_pem = fs::read_to_string(key_path)
            .with_context(|| format!("failed to read CA key {}", key_path.display()))?;
        return Ok((cert_pem, key_pem));
    }

    if let Some(parent) = cert_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    if let Some(parent) = key_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let (cert_pem, key_pem) = generate_ca()?;
    // The CA key is a high-value secret. Create it atomically with restrictive permissions.
    // The cert can be world-readable, but we still write it atomically to avoid partial writes.
    //
    // We intentionally use create-new semantics: if a key already exists, we should not overwrite
    // it silently (that would invalidate previously-trusted cert chains).
    write_atomic_create_new(key_path, key_pem.as_bytes(), 0o600)
        .with_context(|| format!("failed to persist CA key {}", key_path.display()))?;
    if let Err(err) = write_atomic_create_new(cert_path, cert_pem.as_bytes(), 0o644)
        .with_context(|| format!("failed to persist CA cert {}", cert_path.display()))
    {
        // Avoid leaving a partially-created CA around (cert missing) if the second write fails.
        let _ = fs::remove_file(key_path);
        return Err(err);
    }
    let cert_path = cert_path.display();
    let key_path = key_path.display();
    info!("generated MITM CA (cert_path={cert_path}, key_path={key_path})");
    Ok((cert_pem, key_pem))
}

fn generate_ca() -> Result<(String, String)> {
    let mut params = CertificateParams::default();
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.key_usages = vec![
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyEncipherment,
    ];
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, "network_proxy MITM CA");
    params.distinguished_name = dn;

    let key_pair = KeyPair::generate_for(&rcgen_rama::PKCS_ECDSA_P256_SHA256)
        .map_err(|err| anyhow!("failed to generate CA key pair: {err}"))?;
    let cert = params
        .self_signed(&key_pair)
        .map_err(|err| anyhow!("failed to generate CA cert: {err}"))?;
    Ok((cert.pem(), key_pair.serialize_pem()))
}

fn write_atomic_create_new(path: &std::path::Path, contents: &[u8], mode: u32) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("missing parent directory"))?;

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id();
    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
    let tmp_path = parent.join(format!(".{file_name}.tmp.{pid}.{nanos}"));

    let mut file = open_create_new_with_mode(&tmp_path, mode)?;
    file.write_all(contents)
        .with_context(|| format!("failed to write {}", tmp_path.display()))?;
    file.sync_all()
        .with_context(|| format!("failed to fsync {}", tmp_path.display()))?;
    drop(file);

    if path.exists() {
        let _ = fs::remove_file(&tmp_path);
        return Err(anyhow!(
            "refusing to overwrite existing file {}",
            path.display()
        ));
    }

    fs::rename(&tmp_path, path).with_context(|| {
        format!(
            "failed to rename {} -> {}",
            tmp_path.display(),
            path.display()
        )
    })?;

    // Best-effort durability: ensure the directory entry is persisted too.
    let dir = File::open(parent).with_context(|| format!("failed to open {}", parent.display()))?;
    dir.sync_all()
        .with_context(|| format!("failed to fsync {}", parent.display()))?;

    Ok(())
}

#[cfg(unix)]
fn open_create_new_with_mode(path: &std::path::Path, mode: u32) -> Result<File> {
    use std::os::unix::fs::OpenOptionsExt;

    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(path)
        .with_context(|| format!("failed to create {}", path.display()))
}

#[cfg(not(unix))]
fn open_create_new_with_mode(path: &std::path::Path, _mode: u32) -> Result<File> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| format!("failed to create {}", path.display()))
}

fn blocked_text(reason: &str) -> Response {
    blocked_text_response(reason)
}

fn text_response(status: StatusCode, body: &str) -> Response {
    Response::builder()
        .status(status)
        .header("content-type", "text/plain")
        .body(Body::from(body.to_string()))
        .unwrap_or_else(|_| Response::new(Body::from(body.to_string())))
}
