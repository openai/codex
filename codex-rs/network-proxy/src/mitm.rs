#[cfg(feature = "mitm")]
mod imp {
    use crate::config::MitmConfig;
    use crate::config::NetworkMode;
    use crate::policy::method_allowed;
    use crate::policy::normalize_host;
    use crate::responses::text_response;
    use anyhow::Context;
    use anyhow::Result;
    use anyhow::anyhow;
    use hyper::Body;
    use hyper::Method;
    use hyper::Request;
    use hyper::Response;
    use hyper::StatusCode;
    use hyper::Uri;
    use hyper::Version;
    use hyper::body::HttpBody;
    use hyper::header::HOST;
    use hyper::server::conn::Http;
    use hyper::service::service_fn;
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
    use rustls::Certificate as RustlsCertificate;
    use rustls::ClientConfig;
    use rustls::PrivateKey;
    use rustls::RootCertStore;
    use rustls::ServerConfig;
    use std::collections::HashSet;
    use std::convert::Infallible;
    use std::fs;
    use std::io::Cursor;
    use std::net::IpAddr;
    use std::path::Path;
    use std::sync::Arc;
    use tokio::net::TcpStream;
    use tokio_rustls::TlsAcceptor;
    use tokio_rustls::TlsConnector;
    use tracing::info;
    use tracing::warn;

    #[derive(Clone, Copy, Debug)]
    enum MitmProtocol {
        Http1,
        Http2,
    }

    struct MitmTarget {
        host: String,
        port: u16,
    }

    impl MitmTarget {
        fn authority(&self) -> String {
            if self.port == 443 {
                self.host.clone()
            } else {
                format!("{}:{}", self.host, self.port)
            }
        }
    }

    struct RequestLogContext {
        host: String,
        method: Method,
        path: String,
    }

    struct ResponseLogContext {
        host: String,
        method: Method,
        path: String,
        status: StatusCode,
    }

    pub struct MitmState {
        ca_key: KeyPair,
        ca_cert: Certificate,
        client_config: Arc<ClientConfig>,
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
            let client_config = build_client_config()?;

            Ok(Self {
                ca_key,
                ca_cert,
                client_config,
                inspect: cfg.inspect,
                max_body_bytes: cfg.max_body_bytes,
            })
        }

        pub fn server_config_for_host(&self, host: &str) -> Result<Arc<ServerConfig>> {
            let (certs, key) = issue_host_certificate(host, &self.ca_cert, &self.ca_key)?;
            let mut config = ServerConfig::builder()
                .with_safe_defaults()
                .with_no_client_auth()
                .with_single_cert(certs, key)
                .context("failed to build server TLS config")?;
            config.alpn_protocols = vec![b"http/1.1".to_vec()];
            Ok(Arc::new(config))
        }

        pub fn client_config(&self) -> Arc<ClientConfig> {
            Arc::clone(&self.client_config)
        }

        pub fn inspect_enabled(&self) -> bool {
            self.inspect
        }

        pub fn max_body_bytes(&self) -> usize {
            self.max_body_bytes
        }
    }

    pub async fn mitm_tunnel(
        stream: hyper::upgrade::Upgraded,
        host: &str,
        port: u16,
        mode: NetworkMode,
        state: Arc<MitmState>,
    ) -> Result<()> {
        let server_config = state.server_config_for_host(host)?;
        let acceptor = TlsAcceptor::from(server_config);
        let tls_stream = acceptor
            .accept(stream)
            .await
            .context("client TLS handshake failed")?;
        let protocol = match tls_stream.get_ref().1.alpn_protocol() {
            Some(proto) if proto == b"h2" => MitmProtocol::Http2,
            _ => MitmProtocol::Http1,
        };
        info!(
            host = %host,
            port = port,
            protocol = ?protocol,
            mode = ?mode,
            inspect = state.inspect_enabled(),
            max_body_bytes = state.max_body_bytes(),
            "MITM TLS established"
        );

        let target = Arc::new(MitmTarget {
            host: host.to_string(),
            port,
        });
        let service = {
            let state = state.clone();
            let target = target.clone();
            service_fn(move |req| handle_mitm_request(req, target.clone(), mode, state.clone()))
        };

        let mut http = Http::new();
        match protocol {
            MitmProtocol::Http2 => {
                http.http2_only(true);
            }
            MitmProtocol::Http1 => {
                http.http1_only(true);
            }
        }
        http.serve_connection(tls_stream, service)
            .await
            .context("MITM HTTP handling failed")?;
        Ok(())
    }

    async fn handle_mitm_request(
        req: Request<Body>,
        target: Arc<MitmTarget>,
        mode: NetworkMode,
        state: Arc<MitmState>,
    ) -> Result<Response<Body>, Infallible> {
        let response = match forward_request(req, target.as_ref(), mode, state.as_ref()).await {
            Ok(resp) => resp,
            Err(err) => {
                warn!(error = %err, host = %target.host, "MITM upstream request failed");
                text_response(StatusCode::BAD_GATEWAY, "mitm upstream error")
            }
        };
        Ok(response)
    }

    async fn forward_request(
        req: Request<Body>,
        target: &MitmTarget,
        mode: NetworkMode,
        state: &MitmState,
    ) -> Result<Response<Body>> {
        if req.method() == Method::CONNECT {
            return Ok(text_response(
                StatusCode::METHOD_NOT_ALLOWED,
                "CONNECT not supported inside MITM",
            ));
        }

        let (parts, body) = req.into_parts();
        let request_version = parts.version;
        let method = parts.method.clone();
        let inspect = state.inspect_enabled();
        let max_body_bytes = state.max_body_bytes();

        if let Some(request_host) = extract_request_host(&parts) {
            let normalized = normalize_host(&request_host);
            if !normalized.is_empty() && normalized != target.host {
                warn!(
                    target = %target.host,
                    request_host = %normalized,
                    "MITM host mismatch"
                );
                return Ok(text_response(StatusCode::BAD_REQUEST, "host mismatch"));
            }
        }

        let path = path_and_query(&parts.uri);
        let uri = build_origin_form_uri(&path)?;
        let authority = target.authority();

        if !method_allowed(mode, &method) {
            warn!(
                host = %authority,
                method = %method,
                path = %path,
                mode = ?mode,
                allowed_methods = "GET, HEAD, OPTIONS",
                "MITM blocked by method policy"
            );
            return Ok(text_response(StatusCode::FORBIDDEN, "method not allowed"));
        }

        let mut builder = Request::builder()
            .method(method.clone())
            .uri(uri)
            .version(Version::HTTP_11);

        let hop_headers = hop_by_hop_headers();
        for (name, value) in parts.headers.iter() {
            let name_str = name.as_str().to_ascii_lowercase();
            if hop_headers.contains(name_str.as_str()) || name == &HOST {
                continue;
            }
            builder = builder.header(name, value);
        }
        builder = builder.header(HOST, authority.as_str());

        let body = if inspect {
            let (tx, out_body) = Body::channel();
            let ctx = RequestLogContext {
                host: authority.clone(),
                method: method.clone(),
                path: path.clone(),
            };
            tokio::spawn(async move {
                stream_body(body, tx, max_body_bytes, ctx).await;
            });
            out_body
        } else {
            body
        };

        let upstream_req = builder
            .body(body)
            .context("failed to build upstream request")?;
        let upstream_resp = send_upstream_request(upstream_req, target, state).await?;

        respond_with_inspection(
            upstream_resp,
            request_version,
            inspect,
            max_body_bytes,
            &method,
            &path,
            &authority,
        )
        .await
    }

    async fn send_upstream_request(
        req: Request<Body>,
        target: &MitmTarget,
        state: &MitmState,
    ) -> Result<Response<Body>> {
        let upstream = TcpStream::connect((target.host.as_str(), target.port))
            .await
            .context("failed to connect to upstream")?;
        let server_name = match target.host.parse::<IpAddr>() {
            Ok(ip) => rustls::ServerName::IpAddress(ip),
            Err(_) => rustls::ServerName::try_from(target.host.as_str())
                .map_err(|_| anyhow!("invalid server name"))?,
        };
        let connector = TlsConnector::from(state.client_config());
        let tls_stream = connector
            .connect(server_name, upstream)
            .await
            .context("upstream TLS handshake failed")?;
        let (mut sender, conn) = hyper::client::conn::Builder::new()
            .handshake(tls_stream)
            .await
            .context("upstream HTTP handshake failed")?;
        tokio::spawn(async move {
            if let Err(err) = conn.await {
                warn!(error = %err, "MITM upstream connection error");
            }
        });
        let resp = sender
            .send_request(req)
            .await
            .context("upstream request failed")?;
        Ok(resp)
    }

    async fn respond_with_inspection(
        resp: Response<Body>,
        request_version: Version,
        inspect: bool,
        max_body_bytes: usize,
        method: &Method,
        path: &str,
        authority: &str,
    ) -> Result<Response<Body>> {
        let (parts, body) = resp.into_parts();

        let mut builder = Response::builder()
            .status(parts.status)
            .version(request_version);
        let hop_headers = hop_by_hop_headers();
        for (name, value) in parts.headers.iter() {
            if hop_headers.contains(name.as_str().to_ascii_lowercase().as_str()) {
                continue;
            }
            builder = builder.header(name, value);
        }
        let body = if inspect {
            let (tx, out_body) = Body::channel();
            let ctx = ResponseLogContext {
                host: authority.to_string(),
                method: method.clone(),
                path: path.to_string(),
                status: parts.status,
            };
            tokio::spawn(async move {
                stream_body(body, tx, max_body_bytes, ctx).await;
            });
            out_body
        } else {
            body
        };
        Ok(builder
            .body(body)
            .unwrap_or_else(|_| Response::new(Body::from("proxy error"))))
    }

    async fn stream_body<T>(
        mut body: Body,
        mut tx: hyper::body::Sender,
        max_body_bytes: usize,
        ctx: T,
    ) where
        T: BodyLoggable,
    {
        let mut len: usize = 0;
        let mut truncated = false;
        while let Some(chunk) = body.data().await {
            match chunk {
                Ok(bytes) => {
                    len = len.saturating_add(bytes.len());
                    if len > max_body_bytes {
                        truncated = true;
                    }
                    if tx.send_data(bytes).await.is_err() {
                        break;
                    }
                }
                Err(err) => {
                    warn!(error = %err, "MITM body stream error");
                    break;
                }
            }
        }
        if let Ok(Some(trailers)) = body.trailers().await {
            let _ = tx.send_trailers(trailers).await;
        }
        ctx.log(len, truncated);
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

    fn extract_request_host(parts: &hyper::http::request::Parts) -> Option<String> {
        parts
            .headers
            .get(HOST)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.to_string())
            .or_else(|| parts.uri.authority().map(|a| a.as_str().to_string()))
    }

    fn path_and_query(uri: &Uri) -> String {
        uri.path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or("/")
            .to_string()
    }

    fn build_origin_form_uri(path: &str) -> Result<Uri> {
        path.parse().context("invalid request path")
    }

    fn hop_by_hop_headers() -> HashSet<&'static str> {
        [
            "connection",
            "proxy-connection",
            "keep-alive",
            "proxy-authenticate",
            "proxy-authorization",
            "te",
            "trailer",
            "transfer-encoding",
            "upgrade",
        ]
        .into_iter()
        .collect()
    }

    fn build_client_config() -> Result<Arc<ClientConfig>> {
        let mut roots = RootCertStore::empty();
        let certs = rustls_native_certs::load_native_certs()
            .map_err(|err| anyhow!("failed to load native certs: {err}"))?;
        for cert in certs {
            if roots.add(&RustlsCertificate(cert.0)).is_err() {
                warn!("skipping invalid root cert");
            }
        }
        if roots.is_empty() {
            return Err(anyhow!("no root certificates available"));
        }
        let mut config = ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(roots)
            .with_no_client_auth();
        config.alpn_protocols = vec![b"http/1.1".to_vec()];
        Ok(Arc::new(config))
    }

    fn issue_host_certificate(
        host: &str,
        ca_cert: &Certificate,
        ca_key: &KeyPair,
    ) -> Result<(Vec<RustlsCertificate>, PrivateKey)> {
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

        let cert_pem = cert.pem();
        let key_pem = key_pair.serialize_pem();
        let certs = certs_from_pem(&cert_pem)?;
        let key = private_key_from_pem(&key_pem)?;
        Ok((certs, key))
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
        let cert_pem = cert.pem();
        let key_pem = key_pair.serialize_pem();
        Ok((cert_pem, key_pem))
    }

    fn certs_from_pem(pem: &str) -> Result<Vec<RustlsCertificate>> {
        let mut reader = Cursor::new(pem);
        let certs = rustls_pemfile::certs(&mut reader).context("failed to parse cert PEM")?;
        if certs.is_empty() {
            return Err(anyhow!("no certificates found"));
        }
        Ok(certs.into_iter().map(RustlsCertificate).collect())
    }

    fn private_key_from_pem(pem: &str) -> Result<PrivateKey> {
        let mut reader = Cursor::new(pem);
        let mut keys =
            rustls_pemfile::pkcs8_private_keys(&mut reader).context("failed to parse pkcs8 key")?;
        if let Some(key) = keys.pop() {
            return Ok(PrivateKey(key));
        }
        let mut reader = Cursor::new(pem);
        let mut keys =
            rustls_pemfile::rsa_private_keys(&mut reader).context("failed to parse rsa key")?;
        if let Some(key) = keys.pop() {
            return Ok(PrivateKey(key));
        }
        Err(anyhow!("no private key found"))
    }

    fn write_private_file(path: &Path, contents: &[u8], mode: u32) -> Result<()> {
        fs::write(path, contents).with_context(|| format!("failed to write {}", path.display()))?;
        set_permissions(path, mode)?;
        Ok(())
    }

    #[cfg(unix)]
    fn set_permissions(path: &Path, mode: u32) -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
            .with_context(|| format!("failed to set permissions on {}", path.display()))?;
        Ok(())
    }

    #[cfg(not(unix))]
    fn set_permissions(_path: &Path, _mode: u32) -> Result<()> {
        Ok(())
    }
}

#[cfg(not(feature = "mitm"))]
mod imp {
    use crate::config::MitmConfig;
    use crate::config::NetworkMode;
    use anyhow::Result;
    use anyhow::anyhow;
    use hyper::upgrade::Upgraded;
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
        _stream: Upgraded,
        _host: &str,
        _port: u16,
        _mode: NetworkMode,
        _state: Arc<MitmState>,
    ) -> Result<()> {
        Err(anyhow!("MITM feature disabled at build time"))
    }
}

pub use imp::*;
