use std::fs::File;
use std::fs::{self};
use std::io::Read;
use std::io::Write;
use std::net::SocketAddr;
use std::net::TcpListener;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use clap::Parser;
use reqwest::Url;
use reqwest::blocking::Client;
use reqwest::header::AUTHORIZATION;
use reqwest::header::HOST;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;
use serde::Serialize;
use tiny_http::Header;
use tiny_http::Method;
use tiny_http::Request;
use tiny_http::Response;
use tiny_http::Server;
use tiny_http::StatusCode;

mod auth;
mod dump;
mod github_oidc;
mod read_api_key;
use auth::Auth;
use dump::ExchangeDumper;
use github_oidc::GithubOidc;
use read_api_key::read_auth_header_from_stdin;

const OPENAI_RESPONSES_URL: &str = "https://api.openai.com/v1/responses";

/// CLI arguments for the proxy.
#[derive(Clone, Debug, Parser)]
#[command(name = "responses-api-proxy", about = "Minimal OpenAI responses proxy")]
pub struct Args {
    /// Port to listen on. If not set, an ephemeral port is used.
    #[arg(long)]
    pub port: Option<u16>,

    /// Path to a JSON file to write startup info (single line). Includes {"port": <u16>}.
    #[arg(long, value_name = "FILE")]
    pub server_info: Option<PathBuf>,

    /// Enable HTTP shutdown endpoint at GET /shutdown
    #[arg(long)]
    pub http_shutdown: bool,

    /// Absolute URL the proxy should forward requests to (defaults to OpenAI).
    #[arg(long, default_value = "https://api.openai.com/v1/responses")]
    pub upstream_url: String,

    /// Directory where request/response dumps should be written as JSON.
    #[arg(long, value_name = "DIR")]
    pub dump_dir: Option<PathBuf>,

    /// Authenticate to the OpenAI API using GitHub Actions workload identity.
    /// In this mode stdin supplies the GitHub OIDC request token, not an API key.
    #[arg(long, requires_all = ["oidc_audience", "identity_provider_id", "service_account_id", "github_oidc_request_url"])]
    pub github_oidc: bool,

    /// Audience configured on the OpenAI workload identity provider.
    #[arg(long, requires = "github_oidc", value_name = "AUDIENCE")]
    pub oidc_audience: Option<String>,

    /// OpenAI workload identity provider ID.
    #[arg(long, requires = "github_oidc", value_name = "ID")]
    pub identity_provider_id: Option<String>,

    /// OpenAI service account ID selected by the provider mapping.
    #[arg(long, requires = "github_oidc", value_name = "ID")]
    pub service_account_id: Option<String>,

    /// HTTPS token-request URL from GitHub's runner-provided ACTIONS_ID_TOKEN_REQUEST_URL.
    ///
    /// Use the value directly from the trusted runner environment, never from
    /// a workflow input or repository content. The proxy sends the GitHub
    /// request credential to this URL.
    #[arg(long, requires = "github_oidc", value_name = "URL")]
    pub github_oidc_request_url: Option<String>,
}

impl Args {
    fn auth(&self) -> Result<Auth> {
        if self.github_oidc {
            Auth::validate_federated_upstream(&self.upstream_url)?;
            let provider = required(&self.identity_provider_id, "--identity-provider-id")?;
            let service_account = required(&self.service_account_id, "--service-account-id")?;
            let audience = required(&self.oidc_audience, "--oidc-audience")?;
            let request_url = required(&self.github_oidc_request_url, "--github-oidc-request-url")?;
            Auth::federated(
                GithubOidc::from_stdin(request_url, audience)?,
                provider.to_owned(),
                service_account.to_owned(),
            )
        } else {
            Ok(Auth::ApiKey(read_auth_header_from_stdin()?))
        }
    }
}

fn required<'a>(value: &'a Option<String>, name: &str) -> Result<&'a str> {
    value
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow!("workload identity requires a nonempty {name}"))
}

#[derive(Serialize)]
struct ServerInfo {
    port: u16,
    pid: u32,
}

struct ForwardConfig {
    upstream_url: Url,
    host_header: HeaderValue,
}

/// Entry point for the library main, for parity with other crates.
pub fn run_main(args: Args) -> Result<()> {
    let auth = Arc::new(args.auth()?);
    if args.github_oidc {
        // Complete the initial exchange before publishing server info.
        // API keys have already been validated when read from stdin.
        auth.header()?;
    }

    let upstream_url = Url::parse(&args.upstream_url).context("parsing --upstream-url")?;
    let host = match (upstream_url.host_str(), upstream_url.port()) {
        (Some(host), Some(port)) => format!("{host}:{port}"),
        (Some(host), None) => host.to_string(),
        _ => return Err(anyhow!("upstream URL must include a host")),
    };
    let host_header =
        HeaderValue::from_str(&host).context("constructing Host header from upstream URL")?;

    let forward_config = Arc::new(ForwardConfig {
        upstream_url,
        host_header,
    });
    let dump_dir = args
        .dump_dir
        .map(ExchangeDumper::new)
        .transpose()
        .context("creating --dump-dir")?
        .map(Arc::new);

    let (listener, bound_addr) = bind_listener(args.port)?;
    if let Some(path) = args.server_info.as_ref() {
        write_server_info(path, bound_addr.port())?;
    }
    let server = Server::from_listener(listener, None)
        .map_err(|err| anyhow!("creating HTTP server: {err}"))?;
    let client = Arc::new(
        Client::builder()
            .redirect(auth.redirect_policy())
            // Disable reqwest's 30s default so long-lived response streams keep flowing.
            .timeout(None::<Duration>)
            .build()
            .context("building reqwest client")?,
    );

    eprintln!("responses-api-proxy listening on {bound_addr}");

    let http_shutdown = args.http_shutdown;
    for request in server.incoming_requests() {
        let client = client.clone();
        let auth = auth.clone();
        let forward_config = forward_config.clone();
        let dump_dir = dump_dir.clone();
        std::thread::spawn(move || {
            if http_shutdown && request.method() == &Method::Get && request.url() == "/shutdown" {
                let _ = request.respond(Response::new_empty(StatusCode(200)));
                std::process::exit(0);
            }

            if let Err(e) = forward_request(
                &client,
                &auth,
                &forward_config,
                dump_dir.as_deref(),
                request,
            ) {
                eprintln!("forwarding error: {e}");
            }
        });
    }

    Err(anyhow!("server stopped unexpectedly"))
}

fn bind_listener(port: Option<u16>) -> Result<(TcpListener, SocketAddr)> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port.unwrap_or(0)));
    let listener = TcpListener::bind(addr).with_context(|| format!("failed to bind {addr}"))?;
    let bound = listener.local_addr().context("failed to read local_addr")?;
    Ok((listener, bound))
}

fn write_server_info(path: &Path, port: u16) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }

    let info = ServerInfo {
        port,
        pid: std::process::id(),
    };
    let mut data = serde_json::to_string(&info)?;
    data.push('\n');
    let mut f = File::create(path)?;
    f.write_all(data.as_bytes())?;
    Ok(())
}

fn forward_request(
    client: &Client,
    auth: &Auth,
    config: &ForwardConfig,
    dump_dir: Option<&ExchangeDumper>,
    mut req: Request,
) -> Result<()> {
    // Only allow POST /v1/responses exactly, no query string.
    let method = req.method().clone();
    let url_path = req.url().to_string();
    let allow = method == Method::Post && url_path == "/v1/responses";

    if !allow {
        let resp = Response::new_empty(StatusCode(403));
        let _ = req.respond(resp);
        return Ok(());
    }

    // Read request body
    let mut body = Vec::new();
    let reader = req.as_reader();
    reader.read_to_end(&mut body)?;

    let exchange_dump = dump_dir.and_then(|dump_dir| {
        dump_dir
            .dump_request(&method, &url_path, req.headers(), &body)
            .map_err(|err| {
                eprintln!("responses-api-proxy failed to dump request: {err}");
                err
            })
            .ok()
    });

    // Build headers for upstream, forwarding everything from the incoming
    // request except Authorization (we replace it below).
    let mut headers = HeaderMap::new();
    for header in req.headers() {
        let name_ascii = header.field.as_str();
        let lower = name_ascii.to_ascii_lowercase();
        if lower.as_str() == "authorization" || lower.as_str() == "host" {
            continue;
        }

        let header_name = match HeaderName::from_bytes(lower.as_bytes()) {
            Ok(name) => name,
            Err(_) => continue,
        };
        if let Ok(value) = HeaderValue::from_bytes(header.value.as_bytes()) {
            headers.append(header_name, value);
        }
    }

    let auth_header = match auth.header() {
        Ok(value) => value,
        Err(err) => {
            // No token, request URL or upstream error body is exposed to the caller.
            let _ = req.respond(Response::new_empty(StatusCode(502)));
            return Err(err);
        }
    };
    headers.insert(AUTHORIZATION, auth_header.value.clone());

    headers.insert(HOST, config.host_header.clone());

    let upstream_resp = client
        .post(config.upstream_url.clone())
        .headers(headers)
        .body(body)
        .send()
        .context("forwarding request to upstream")?;

    // We have to create an adapter between a `reqwest::blocking::Response`
    // and a `tiny_http::Response`. Fortunately, `reqwest::blocking::Response`
    // implements `Read`, so we can use it directly as the body of the
    // `tiny_http::Response`.
    let status = upstream_resp.status();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        // Do not replay a Responses POST. A subsequent request can obtain a
        // fresh credential, but a late 401 must not evict one renewed in-flight.
        auth_header.invalidate();
    }
    let mut response_headers = Vec::new();
    for (name, value) in upstream_resp.headers().iter() {
        // Skip headers that tiny_http manages itself.
        if matches!(
            name.as_str(),
            "content-length" | "transfer-encoding" | "connection" | "trailer" | "upgrade"
        ) {
            continue;
        }

        if let Ok(header) = Header::from_bytes(name.as_str().as_bytes(), value.as_bytes()) {
            response_headers.push(header);
        }
    }

    let content_length = upstream_resp.content_length().and_then(|len| {
        if len <= usize::MAX as u64 {
            Some(len as usize)
        } else {
            None
        }
    });

    let response_body: Box<dyn Read + Send> = if let Some(exchange_dump) = exchange_dump {
        let headers = upstream_resp.headers().clone();
        Box::new(exchange_dump.tee_response_body(status.as_u16(), &headers, upstream_resp))
    } else {
        Box::new(upstream_resp)
    };

    let response = Response::new(
        StatusCode(status.as_u16()),
        response_headers,
        response_body,
        content_length,
        None,
    );

    let _ = req.respond(response);
    Ok(())
}
