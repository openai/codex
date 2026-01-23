use crate::config::MitmConfig;
use crate::config::NetworkMode;
use crate::policy::normalize_host;
use crate::reasons::REASON_METHOD_NOT_ALLOWED;
use crate::responses::blocked_text_response;
use crate::responses::text_response;
use crate::state::BlockedRequest;
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
use rama_net::tls::ApplicationProtocol;
use rama_net::tls::DataEncoding;
use rama_net::tls::server::ServerAuth;
use rama_net::tls::server::ServerAuthData;
use rama_net::tls::server::ServerConfig;
use rama_tls_boring::server::TlsAcceptorData;
use rama_tls_boring::server::TlsAcceptorLayer;
use rama_utils::str::NonEmptyStr;
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
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
    upstream: UpstreamClient,
    inspect: bool,
    max_body_bytes: usize,
}

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
    pub fn new(cfg: &MitmConfig, allow_upstream_proxy: bool) -> Result<Self> {
        // MITM exists to make limited-mode HTTPS enforceable: once CONNECT is established, plain
        // proxying would lose visibility into the inner HTTP request. We generate/load a local CA
        // and issue per-host leaf certs so we can terminate TLS and apply policy.
        let (ca_cert_pem, ca_key_pem) = load_or_create_ca(cfg)?;
        let ca_key = KeyPair::from_pem(&ca_key_pem).context("failed to parse CA key")?;
        let issuer: Issuer<'static, KeyPair> =
            Issuer::from_ca_cert_pem(&ca_cert_pem, ca_key).context("failed to parse CA cert")?;

        let upstream = if allow_upstream_proxy {
            UpstreamClient::from_env_proxy()
        } else {
            UpstreamClient::direct()
        };

        Ok(Self {
            issuer,
            upstream,
            inspect: cfg.inspect,
            max_body_bytes: cfg.max_body_bytes,
        })
    }

    fn tls_acceptor_data_for_host(&self, host: &str) -> Result<TlsAcceptorData> {
        let (cert_pem, key_pem) = issue_host_certificate_pem(host, &self.issuer)?;
        let cert_chain = DataEncoding::Pem(
            NonEmptyStr::try_from(cert_pem.as_str()).context("failed to encode host cert PEM")?,
        );
        let private_key = DataEncoding::Pem(
            NonEmptyStr::try_from(key_pem.as_str()).context("failed to encode host key PEM")?,
        );
        let auth = ServerAuthData {
            private_key,
            cert_chain,
            ocsp: None,
        };

        let mut server_config = ServerConfig::new(ServerAuth::Single(auth));
        server_config.application_layer_protocol_negotiation = Some(vec![
            ApplicationProtocol::HTTP_2,
            ApplicationProtocol::HTTP_11,
        ]);

        TlsAcceptorData::try_from(server_config).context("failed to build boring acceptor config")
    }

    pub fn inspect_enabled(&self) -> bool {
        self.inspect
    }

    pub fn max_body_bytes(&self) -> usize {
        self.max_body_bytes
    }
}

pub async fn mitm_tunnel(upgraded: Upgraded) -> Result<()> {
    let state = upgraded
        .extensions()
        .get::<Arc<MitmState>>()
        .cloned()
        .context("missing MITM state")?;
    let target = upgraded
        .extensions()
        .get::<ProxyTarget>()
        .context("missing proxy target")?
        .0
        .clone();
    let host = normalize_host(&target.host.to_string());
    let acceptor_data = state.tls_acceptor_data_for_host(&host)?;

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
            .into_layer(service_fn(handle_mitm_request)),
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

async fn handle_mitm_request(req: Request) -> Result<Response, std::convert::Infallible> {
    let response = match forward_request(req).await {
        Ok(resp) => resp,
        Err(err) => {
            warn!("MITM upstream request failed: {err}");
            text_response(StatusCode::BAD_GATEWAY, "mitm upstream error")
        }
    };
    Ok(response)
}

async fn forward_request(req: Request) -> Result<Response> {
    let target = req
        .extensions()
        .get::<ProxyTarget>()
        .context("missing proxy target")?
        .0
        .clone();

    let target_host = normalize_host(&target.host.to_string());
    let target_port = target.port;
    let mode = req
        .extensions()
        .get::<NetworkMode>()
        .copied()
        .unwrap_or(NetworkMode::Full);
    let mitm = req
        .extensions()
        .get::<Arc<MitmState>>()
        .cloned()
        .context("missing MITM state")?;
    let app_state = req
        .extensions()
        .get::<Arc<NetworkProxyState>>()
        .cloned()
        .context("missing app state")?;

    if req.method().as_str() == "CONNECT" {
        return Ok(text_response(
            StatusCode::METHOD_NOT_ALLOWED,
            "CONNECT not supported inside MITM",
        ));
    }

    let method = req.method().as_str().to_string();
    let path = path_and_query(req.uri());
    let client = req
        .extensions()
        .get::<SocketInfo>()
        .map(|info| info.peer_addr().to_string());

    if let Some(request_host) = extract_request_host(&req) {
        let normalized = normalize_host(&request_host);
        if !normalized.is_empty() && normalized != target_host {
            warn!("MITM host mismatch (target={target_host}, request_host={normalized})");
            return Ok(text_response(StatusCode::BAD_REQUEST, "host mismatch"));
        }
    }

    if !mode.allows_method(&method) {
        let _ = app_state
            .record_blocked(BlockedRequest::new(
                target_host.clone(),
                REASON_METHOD_NOT_ALLOWED.to_string(),
                client.clone(),
                Some(method.clone()),
                Some(mode),
                "https".to_string(),
            ))
            .await;
        warn!(
            "MITM blocked by method policy (host={target_host}, method={method}, path={path}, mode={mode:?}, allowed_methods=GET, HEAD, OPTIONS)"
        );
        return Ok(blocked_text_response(REASON_METHOD_NOT_ALLOWED));
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
    let upstream_resp = mitm.upstream.serve(upstream_req).await?;
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

    // Create the final file using "create-new" semantics (no overwrite). `rename` on Unix can
    // overwrite existing files, so prefer a hard-link, which fails if the destination exists.
    match fs::hard_link(&tmp_path, path) {
        Ok(()) => {
            fs::remove_file(&tmp_path)
                .with_context(|| format!("failed to remove {}", tmp_path.display()))?;
        }
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
            let _ = fs::remove_file(&tmp_path);
            return Err(anyhow!(
                "refusing to overwrite existing file {}",
                path.display()
            ));
        }
        Err(_) => {
            // Best-effort fallback for environments where hard links are not supported.
            // This is still subject to a TOCTOU race, but the typical case is a private per-user
            // config directory, where other users cannot create files anyway.
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
        }
    }

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
