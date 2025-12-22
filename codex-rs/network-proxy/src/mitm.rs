#[cfg(feature = "mitm")]
mod imp {
    use crate::config::MitmConfig;
    use crate::config::NetworkMode;
    use crate::policy::method_allowed;
    use crate::policy::normalize_host;
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
    use std::io::BufReader;
    use std::net::IpAddr;
    use std::pin::Pin;
    use std::sync::Arc;
    use std::task::Context as TaskContext;
    use std::task::Poll;
    use tracing::info;
    use tracing::warn;

    use rcgen::BasicConstraints;
    use rcgen::Certificate;
    use rcgen::CertificateParams;
    use rcgen::DistinguishedName;
    use rcgen::DnType;
    use rcgen::ExtendedKeyUsagePurpose;
    use rcgen::IsCa;
    use rcgen::KeyPair;
    use rcgen::KeyUsagePurpose;
    use rcgen::SanType;

    pub struct MitmState {
        ca_key: KeyPair,
        ca_cert: Certificate,
        upstream: rama::service::BoxService<Arc<AppState>, Request, Response, OpaqueError>,
        inspect: bool,
        max_body_bytes: usize,
    }

    impl MitmState {
        pub fn new(cfg: &MitmConfig) -> Result<Self> {
            let (ca_cert_pem, ca_key_pem) = load_or_create_ca(cfg)?;
            let ca_key = KeyPair::from_pem(&ca_key_pem).context("failed to parse CA key")?;
            let ca_params = CertificateParams::from_ca_cert_pem(&ca_cert_pem)
                .context("failed to parse CA cert")?;
            let ca_cert = ca_params
                .self_signed(&ca_key)
                .context("failed to reconstruct CA cert")?;

            let tls_config = rama::tls::rustls::client::TlsConnectorData::new_http_auto()
                .context("create upstream TLS config")?;
            let upstream = rama::http::client::EasyHttpWebClient::builder()
                .with_default_transport_connector()
                .without_tls_proxy_support()
                .without_proxy_support()
                .with_tls_support_using_rustls(Some(tls_config))
                .build()
                .boxed();

            Ok(Self {
                ca_key,
                ca_cert,
                upstream,
                inspect: cfg.inspect,
                max_body_bytes: cfg.max_body_bytes,
            })
        }

        fn tls_acceptor_data_for_host(&self, host: &str) -> Result<TlsAcceptorData> {
            let (cert_pem, key_pem) =
                issue_host_certificate_pem(host, &self.ca_cert, &self.ca_key)?;
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
                warn!(error = %err, "MITM upstream request failed");
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
                warn!(
                    target = %target_host,
                    request_host = %normalized,
                    "MITM host mismatch"
                );
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
                host = %target_host,
                method = %method,
                path = %path,
                mode = ?mode,
                allowed_methods = "GET, HEAD, OPTIONS",
                "MITM blocked by method policy"
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
            info!(
                host = %self.host,
                method = %self.method,
                path = %self.path,
                body_len = len,
                truncated = truncated,
                "MITM inspected request body"
            );
        }
    }

    impl BodyLoggable for ResponseLogContext {
        fn log(self, len: usize, truncated: bool) {
            info!(
                host = %self.host,
                method = %self.method,
                path = %self.path,
                status = %self.status,
                body_len = len,
                truncated = truncated,
                "MITM inspected response body"
            );
        }
    }

    fn extract_request_host(req: &Request) -> Option<String> {
        req.headers()
            .get(HOST)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.to_string())
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
            .map(|pq| pq.as_str())
            .unwrap_or("/")
            .to_string()
    }

    fn issue_host_certificate_pem(
        host: &str,
        ca_cert: &Certificate,
        ca_key: &KeyPair,
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

        let key_pair = KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256)
            .map_err(|err| anyhow!("failed to generate host key pair: {err}"))?;
        let cert = params
            .signed_by(&key_pair, ca_cert, ca_key)
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
        write_private_file(cert_path, cert_pem.as_bytes(), 0o644)?;
        write_private_file(key_path, key_pem.as_bytes(), 0o600)?;
        info!(
            cert_path = %cert_path.display(),
            key_path = %key_path.display(),
            "generated MITM CA"
        );
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

        let key_pair = KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256)
            .map_err(|err| anyhow!("failed to generate CA key pair: {err}"))?;
        let cert = params
            .self_signed(&key_pair)
            .map_err(|err| anyhow!("failed to generate CA cert: {err}"))?;
        Ok((cert.pem(), key_pair.serialize_pem()))
    }

    fn write_private_file(path: &std::path::Path, contents: &[u8], mode: u32) -> Result<()> {
        fs::write(path, contents).with_context(|| format!("failed to write {}", path.display()))?;
        set_permissions(path, mode)?;
        Ok(())
    }

    #[cfg(unix)]
    fn set_permissions(path: &std::path::Path, mode: u32) -> Result<()> {
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(path, fs::Permissions::from_mode(mode))
            .with_context(|| format!("failed to set permissions on {}", path.display()))?;
        Ok(())
    }

    #[cfg(not(unix))]
    fn set_permissions(_path: &std::path::Path, _mode: u32) -> Result<()> {
        Ok(())
    }

    fn blocked_text(reason: &str) -> Response {
        Response::builder()
            .status(StatusCode::FORBIDDEN)
            .header("content-type", "text/plain")
            .header("x-proxy-error", blocked_header_value(reason))
            .body(Body::from(blocked_message(reason)))
            .unwrap_or_else(|_| Response::new(Body::from("blocked")))
    }

    fn text_response(status: StatusCode, body: &str) -> Response {
        Response::builder()
            .status(status)
            .header("content-type", "text/plain")
            .body(Body::from(body.to_string()))
            .unwrap_or_else(|_| Response::new(Body::from(body.to_string())))
    }

    fn blocked_header_value(reason: &str) -> &'static str {
        match reason {
            "not_allowed" | "not_allowed_local" => "blocked-by-allowlist",
            "denied" => "blocked-by-denylist",
            "method_not_allowed" => "blocked-by-method-policy",
            "mitm_required" => "blocked-by-mitm-required",
            _ => "blocked-by-policy",
        }
    }

    fn blocked_message(reason: &str) -> &'static str {
        match reason {
            "method_not_allowed" => {
                "Codex blocked this request: method not allowed in limited mode."
            }
            _ => "Codex blocked this request by network policy.",
        }
    }
}

#[cfg(not(feature = "mitm"))]
mod imp {
    use crate::config::MitmConfig;
    use crate::config::NetworkMode;
    use crate::state::AppState;
    use anyhow::Result;
    use anyhow::anyhow;
    use rama::Context as RamaContext;
    use rama::http::layer::upgrade::Upgraded;
    use std::sync::Arc;

    #[derive(Debug)]
    pub struct MitmState;

    #[allow(dead_code)]
    impl MitmState {
        pub fn new(_cfg: &MitmConfig) -> Result<Self> {
            Err(anyhow!("MITM feature disabled at build time"))
        }

        pub fn inspect_enabled(&self) -> bool {
            false
        }

        pub fn max_body_bytes(&self) -> usize {
            0
        }
    }

    pub async fn mitm_tunnel(
        _ctx: RamaContext<Arc<AppState>>,
        _upgraded: Upgraded,
        _host: &str,
        _port: u16,
        _mode: NetworkMode,
        _state: Arc<MitmState>,
    ) -> Result<()> {
        Err(anyhow!("MITM feature disabled at build time"))
    }
}

pub use imp::*;
