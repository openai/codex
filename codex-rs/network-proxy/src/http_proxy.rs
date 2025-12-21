use crate::config::NetworkMode;
use crate::mitm;
use crate::policy::normalize_host;
use crate::responses::blocked_text;
use crate::responses::json_blocked;
use crate::responses::text_response;
use crate::state::AppState;
use crate::state::BlockedRequest;
use anyhow::Result;
use hyper::Body;
use hyper::Method;
use hyper::Request;
use hyper::Response;
use hyper::Server;
use hyper::StatusCode;
use hyper::Uri;
use hyper::body::to_bytes;
use hyper::header::HOST;
use hyper::header::HeaderName;
use hyper::service::make_service_fn;
use hyper::service::service_fn;
use std::collections::HashSet;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::copy_bidirectional;
use tokio::net::TcpStream;
use tracing::error;
use tracing::info;
use tracing::warn;

pub async fn run_http_proxy(state: Arc<AppState>, addr: SocketAddr) -> Result<()> {
    let make_svc = make_service_fn(move |conn: &hyper::server::conn::AddrStream| {
        let state = state.clone();
        let client_addr = conn.remote_addr();
        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                handle_proxy_request(req, state.clone(), client_addr)
            }))
        }
    });
    let server = Server::bind(&addr).serve(make_svc);
    info!(addr = %addr, "HTTP proxy listening");
    server.await?;
    Ok(())
}

async fn handle_proxy_request(
    req: Request<Body>,
    state: Arc<AppState>,
    client_addr: SocketAddr,
) -> Result<Response<Body>, Infallible> {
    let response = if req.method() == Method::CONNECT {
        handle_connect(req, state, client_addr).await
    } else {
        handle_http_forward(req, state, client_addr).await
    };
    Ok(response)
}

async fn handle_connect(
    req: Request<Body>,
    state: Arc<AppState>,
    client_addr: SocketAddr,
) -> Response<Body> {
    let authority = match req.uri().authority() {
        Some(auth) => auth.as_str().to_string(),
        None => return text_response(StatusCode::BAD_REQUEST, "missing authority"),
    };
    let (authority_host, target_port) = split_authority(&authority);
    let host = normalize_host(&authority_host);
    if host.is_empty() {
        return text_response(StatusCode::BAD_REQUEST, "invalid host");
    }

    match state.host_blocked(&host).await {
        Ok((true, reason)) => {
            let _ = state
                .record_blocked(BlockedRequest::new(
                    host.clone(),
                    reason.clone(),
                    Some(client_addr.to_string()),
                    Some("CONNECT".to_string()),
                    None,
                    "http-connect".to_string(),
                ))
                .await;
            warn!(client = %client_addr, host = %host, reason = %reason, "CONNECT blocked");
            return blocked_text(&reason);
        }
        Ok((false, _)) => {
            info!(client = %client_addr, host = %host, "CONNECT allowed");
        }
        Err(err) => {
            error!(error = %err, "failed to evaluate host");
            return text_response(StatusCode::INTERNAL_SERVER_ERROR, "error");
        }
    }

    let mode = match state.network_mode().await {
        Ok(mode) => mode,
        Err(err) => {
            error!(error = %err, "failed to read network mode");
            return text_response(StatusCode::INTERNAL_SERVER_ERROR, "error");
        }
    };

    let mitm_state = match state.mitm_state().await {
        Ok(state) => state,
        Err(err) => {
            error!(error = %err, "failed to load MITM state");
            return text_response(StatusCode::INTERNAL_SERVER_ERROR, "error");
        }
    };
    if mode == NetworkMode::Limited && mitm_state.is_none() {
        let _ = state
            .record_blocked(BlockedRequest::new(
                host.clone(),
                "mitm_required".to_string(),
                Some(client_addr.to_string()),
                Some("CONNECT".to_string()),
                Some(NetworkMode::Limited),
                "http-connect".to_string(),
            ))
            .await;
        warn!(
            client = %client_addr,
            host = %host,
            mode = "limited",
            allowed_methods = "GET, HEAD, OPTIONS",
            "CONNECT blocked; MITM required for read-only HTTPS in limited mode"
        );
        return blocked_text("mitm_required");
    }

    let on_upgrade = hyper::upgrade::on(req);
    tokio::spawn(async move {
        match on_upgrade.await {
            Ok(upgraded) => {
                if let Some(mitm_state) = mitm_state {
                    info!(client = %client_addr, host = %host, mode = ?mode, "CONNECT MITM enabled");
                    if let Err(err) =
                        mitm::mitm_tunnel(upgraded, &host, target_port, mode, mitm_state).await
                    {
                        warn!(error = %err, "MITM tunnel error");
                    }
                    return;
                }
                let mut upgraded = upgraded;
                match TcpStream::connect(&authority).await {
                    Ok(mut server_stream) => {
                        if let Err(err) =
                            copy_bidirectional(&mut upgraded, &mut server_stream).await
                        {
                            warn!(error = %err, "tunnel error");
                        }
                    }
                    Err(err) => {
                        warn!(error = %err, "failed to connect to upstream");
                    }
                }
            }
            Err(err) => warn!(error = %err, "upgrade failed"),
        }
    });

    Response::builder()
        .status(StatusCode::OK)
        .body(Body::empty())
        .unwrap_or_else(|_| Response::new(Body::empty()))
}

async fn handle_http_forward(
    req: Request<Body>,
    state: Arc<AppState>,
    client_addr: SocketAddr,
) -> Response<Body> {
    let (parts, body) = req.into_parts();
    let method_allowed = match state.method_allowed(&parts.method).await {
        Ok(allowed) => allowed,
        Err(err) => {
            error!(error = %err, "failed to evaluate method policy");
            return text_response(StatusCode::INTERNAL_SERVER_ERROR, "error");
        }
    };
    let unix_socket = parts
        .headers
        .get("x-unix-socket")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_string());

    if let Some(socket_path) = unix_socket {
        if !method_allowed {
            warn!(
                client = %client_addr,
                method = %parts.method,
                mode = "limited",
                allowed_methods = "GET, HEAD, OPTIONS",
                "unix socket blocked by method policy"
            );
            return json_blocked("unix-socket", "method_not_allowed");
        }
        if !cfg!(target_os = "macos") {
            warn!(path = %socket_path, "unix socket proxy unsupported on this platform");
            return text_response(StatusCode::NOT_IMPLEMENTED, "unix sockets unsupported");
        }
        match state.is_unix_socket_allowed(&socket_path).await {
            Ok(true) => {
                info!(client = %client_addr, path = %socket_path, "unix socket allowed");
                match proxy_via_unix_socket(Request::from_parts(parts, body), &socket_path).await {
                    Ok(resp) => return resp,
                    Err(err) => {
                        warn!(error = %err, "unix socket proxy failed");
                        return text_response(StatusCode::BAD_GATEWAY, "unix socket proxy failed");
                    }
                }
            }
            Ok(false) => {
                warn!(client = %client_addr, path = %socket_path, "unix socket blocked");
                return json_blocked("unix-socket", "not_allowed");
            }
            Err(err) => {
                warn!(error = %err, "unix socket check failed");
                return text_response(StatusCode::INTERNAL_SERVER_ERROR, "error");
            }
        }
    }

    let host_header = parts
        .headers
        .get(HOST)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_string())
        .or_else(|| parts.uri.authority().map(|a| a.as_str().to_string()));

    let authority = match host_header {
        Some(h) => h,
        None => return text_response(StatusCode::BAD_REQUEST, "missing host"),
    };
    let authority = authority.trim().to_string();
    let host = normalize_host(&authority);
    if host.is_empty() {
        return text_response(StatusCode::BAD_REQUEST, "invalid host");
    }

    match state.host_blocked(&host).await {
        Ok((true, reason)) => {
            let _ = state
                .record_blocked(BlockedRequest::new(
                    host.clone(),
                    reason.clone(),
                    Some(client_addr.to_string()),
                    Some(parts.method.to_string()),
                    None,
                    "http".to_string(),
                ))
                .await;
            warn!(client = %client_addr, host = %host, reason = %reason, "request blocked");
            return json_blocked(&host, &reason);
        }
        Ok((false, _)) => {}
        Err(err) => {
            error!(error = %err, "failed to evaluate host");
            return text_response(StatusCode::INTERNAL_SERVER_ERROR, "error");
        }
    }

    if !method_allowed {
        let _ = state
            .record_blocked(BlockedRequest::new(
                host.clone(),
                "method_not_allowed".to_string(),
                Some(client_addr.to_string()),
                Some(parts.method.to_string()),
                Some(NetworkMode::Limited),
                "http".to_string(),
            ))
            .await;
        warn!(
            client = %client_addr,
            host = %host,
            method = %parts.method,
            mode = "limited",
            allowed_methods = "GET, HEAD, OPTIONS",
            "request blocked by method policy"
        );
        return json_blocked(&host, "method_not_allowed");
    }
    info!(
        client = %client_addr,
        host = %host,
        method = %parts.method,
        "request allowed"
    );

    let uri = match build_forward_uri(&authority, &parts.uri) {
        Ok(uri) => uri,
        Err(err) => {
            warn!(error = %err, "failed to build upstream uri");
            return text_response(StatusCode::BAD_REQUEST, "invalid uri");
        }
    };

    let body_bytes = match to_bytes(body).await {
        Ok(bytes) => bytes,
        Err(err) => {
            warn!(error = %err, "failed to read body");
            return text_response(StatusCode::BAD_GATEWAY, "failed to read body");
        }
    };

    let mut builder = Request::builder()
        .method(parts.method)
        .uri(uri)
        .version(parts.version);
    let hop_headers = hop_by_hop_headers();
    for (name, value) in parts.headers.iter() {
        let name_str = name.as_str().to_ascii_lowercase();
        if hop_headers.contains(name_str.as_str())
            || name == &HeaderName::from_static("x-unix-socket")
        {
            continue;
        }
        builder = builder.header(name, value);
    }

    let forwarded_req = match builder.body(Body::from(body_bytes)) {
        Ok(req) => req,
        Err(err) => {
            warn!(error = %err, "failed to build request");
            return text_response(StatusCode::BAD_GATEWAY, "invalid request");
        }
    };

    match state.client.request(forwarded_req).await {
        Ok(resp) => filter_response(resp),
        Err(err) => {
            warn!(error = %err, "upstream request failed");
            text_response(StatusCode::BAD_GATEWAY, "upstream failure")
        }
    }
}

fn build_forward_uri(authority: &str, uri: &Uri) -> Result<Uri> {
    let path = path_and_query(uri);
    let target = format!("http://{authority}{path}");
    Ok(target.parse()?)
}

fn filter_response(resp: Response<Body>) -> Response<Body> {
    let mut builder = Response::builder().status(resp.status());
    let hop_headers = hop_by_hop_headers();
    for (name, value) in resp.headers().iter() {
        if hop_headers.contains(name.as_str().to_ascii_lowercase().as_str()) {
            continue;
        }
        builder = builder.header(name, value);
    }
    builder
        .body(resp.into_body())
        .unwrap_or_else(|_| Response::new(Body::from("proxy error")))
}

fn path_and_query(uri: &Uri) -> String {
    uri.path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/")
        .to_string()
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

fn split_authority(authority: &str) -> (String, u16) {
    if let Some(host) = authority.strip_prefix('[') {
        if let Some(end) = host.find(']') {
            let hostname = host[..end].to_string();
            let port = host[end + 1..]
                .strip_prefix(':')
                .and_then(|p| p.parse::<u16>().ok())
                .unwrap_or(443);
            return (hostname, port);
        }
    }
    let mut parts = authority.splitn(2, ':');
    let host = parts.next().unwrap_or("").to_string();
    let port = parts
        .next()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(443);
    (host, port)
}

async fn proxy_via_unix_socket(req: Request<Body>, socket_path: &str) -> Result<Response<Body>> {
    #[cfg(target_os = "macos")]
    {
        use hyper::client::conn::Builder as ConnBuilder;
        use tokio::net::UnixStream;

        let path = path_and_query(req.uri());
        let (parts, body) = req.into_parts();
        let body_bytes = to_bytes(body).await?;
        let mut builder = Request::builder()
            .method(parts.method)
            .uri(path)
            .version(parts.version);
        let hop_headers = hop_by_hop_headers();
        for (name, value) in parts.headers.iter() {
            let name_str = name.as_str().to_ascii_lowercase();
            if hop_headers.contains(name_str.as_str())
                || name == &HeaderName::from_static("x-unix-socket")
            {
                continue;
            }
            builder = builder.header(name, value);
        }
        let req = builder.body(Body::from(body_bytes))?;
        let stream = UnixStream::connect(socket_path).await?;
        let (mut sender, conn) = ConnBuilder::new().handshake(stream).await?;
        tokio::spawn(async move {
            if let Err(err) = conn.await {
                warn!(error = %err, "unix socket connection error");
            }
        });
        Ok(sender.send_request(req).await?)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = req;
        let _ = socket_path;
        Err(anyhow::anyhow!("unix sockets not supported"))
    }
}
