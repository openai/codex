#![cfg(unix)]

mod common;

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use codex_exec_server::HttpRedirectPolicy;
use codex_exec_server::HttpRequestParams;
use codex_exec_server::HttpRequestResponse;
use codex_exec_server::InitializeParams;
use codex_exec_server_protocol::JSONRPCMessage;
use codex_exec_server_protocol::JSONRPCResponse;
use codex_exec_server_protocol::RequestId;
use common::exec_server::ExecServerHarness;
use common::exec_server::exec_server_with_env;
use pretty_assertions::assert_eq;
use rcgen::BasicConstraints;
use rcgen::CertificateParams;
use rcgen::CertifiedIssuer;
use rcgen::DistinguishedName;
use rcgen::DnType;
use rcgen::ExtendedKeyUsagePurpose;
use rcgen::IsCa;
use rcgen::KeyPair;
use rcgen::KeyUsagePurpose;
use rcgen::PKCS_ECDSA_P256_SHA256;
use rustls::pki_types::CertificateDer;
use rustls::pki_types::PrivateKeyDer;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tempfile::TempDir;

const CHATGPT_MCP_URL: &str = "https://chatgpt.com/backend-api/ps/mcp";
const NON_CHATGPT_MCP_URL: &str = "https://api.openai.com/backend-api/ps/mcp";

#[derive(Debug)]
struct CapturedRequest {
    connect_authority: String,
    request_line: String,
    headers: BTreeMap<String, Vec<String>>,
}

struct TlsMaterial {
    ca_cert_pem: String,
    server_cert: CertificateDer<'static>,
    server_key: PrivateKeyDer<'static>,
}

struct TlsInterceptingProxy {
    ca_cert_pem: String,
    request_rx: mpsc::Receiver<Result<CapturedRequest, String>>,
    thread: thread::JoinHandle<Result<(), String>>,
    url: String,
}

/// Exercises the same `http/request` route used by remotely executed Streamable HTTP MCP calls.
/// Each RPC uses the shared route-aware client. The first response sets `__cflb`, and the second response
/// replaces it, proving cross-client persistence through the shared cookie store.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_server_replays_only_chatgpt_cloudflare_cookies() -> anyhow::Result<()> {
    let proxy = TlsInterceptingProxy::start(/*expected_requests*/ 4)?;
    let temp_dir = TempDir::new()?;
    let ca_path = temp_dir.path().join("cloudflare-affinity-test-ca.pem");
    fs::write(&ca_path, &proxy.ca_cert_pem)?;
    let proxy_url = OsString::from(&proxy.url);
    let empty = OsString::new();
    let env = vec![
        (
            OsString::from("CODEX_CA_CERTIFICATE"),
            ca_path.as_os_str().to_owned(),
        ),
        (OsString::from("HTTPS_PROXY"), proxy_url.clone()),
        (OsString::from("https_proxy"), proxy_url.clone()),
        (OsString::from("ALL_PROXY"), proxy_url.clone()),
        (OsString::from("all_proxy"), proxy_url),
        (OsString::from("NO_PROXY"), empty.clone()),
        (OsString::from("no_proxy"), empty),
    ];
    let mut server = exec_server_with_env(env).await?;
    initialize_exec_server(&mut server).await?;

    let first_response = execute_http_request(&mut server, CHATGPT_MCP_URL, "first").await?;
    assert_eq!(
        (first_response.status, first_response.body.into_inner()),
        (200, b"ok".to_vec())
    );
    let first = proxy.next_request()?;
    assert_eq!(
        (
            first.connect_authority.as_str(),
            first.request_line.as_str(),
            first.headers.get("cookie"),
        ),
        ("chatgpt.com:443", "POST /backend-api/ps/mcp HTTP/1.1", None,)
    );

    let west_response = execute_http_request(&mut server, CHATGPT_MCP_URL, "west").await?;
    assert_eq!(west_response.status, 200);
    let request_with_west_affinity = proxy.next_request()?;
    assert_eq!(
        request_with_west_affinity
            .headers
            .get("cookie")
            .cloned()
            .unwrap_or_default(),
        vec!["__cflb=west".to_string()]
    );

    let central_response = execute_http_request(&mut server, CHATGPT_MCP_URL, "central").await?;
    assert_eq!(central_response.status, 200);
    let request_with_central_affinity = proxy.next_request()?;
    assert_eq!(
        (
            request_with_central_affinity.request_line.as_str(),
            request_with_central_affinity
                .headers
                .get("cookie")
                .cloned()
                .unwrap_or_default(),
        ),
        (
            "POST /backend-api/ps/mcp HTTP/1.1",
            vec!["__cflb=central".to_string()],
        )
    );
    let other_host_response =
        execute_http_request(&mut server, NON_CHATGPT_MCP_URL, "other-host").await?;
    assert_eq!(other_host_response.status, 200);
    let other_host = proxy.next_request()?;
    assert_eq!(
        (
            other_host.connect_authority.as_str(),
            other_host.request_line.as_str(),
            other_host.headers.get("cookie"),
        ),
        (
            "api.openai.com:443",
            "POST /backend-api/ps/mcp HTTP/1.1",
            None,
        )
    );

    server.shutdown().await?;
    proxy.finish()?;
    Ok(())
}

impl TlsInterceptingProxy {
    fn start(expected_requests: usize) -> anyhow::Result<Self> {
        codex_utils_rustls_provider::ensure_rustls_crypto_provider();
        let material = generate_tls_material()?;
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let address = listener.local_addr()?;
        let config = Arc::new(
            rustls::ServerConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
                .with_no_client_auth()
                .with_single_cert(vec![material.server_cert], material.server_key)?,
        );
        let (request_tx, request_rx) = mpsc::channel();
        let thread = thread::spawn(move || {
            run_tls_intercepting_proxy(listener, config, request_tx, expected_requests)
                .map_err(|error| error.to_string())
        });

        Ok(Self {
            ca_cert_pem: material.ca_cert_pem,
            request_rx,
            thread,
            url: format!("http://{address}"),
        })
    }

    fn next_request(&self) -> anyhow::Result<CapturedRequest> {
        self.request_rx
            .recv_timeout(Duration::from_secs(5))
            .map_err(anyhow::Error::from)?
            .map_err(anyhow::Error::msg)
    }

    fn finish(self) -> anyhow::Result<()> {
        self.thread
            .join()
            .map_err(|_| anyhow::anyhow!("TLS proxy thread panicked"))?
            .map_err(anyhow::Error::msg)
    }
}

fn generate_tls_material() -> anyhow::Result<TlsMaterial> {
    let mut ca_params = CertificateParams::default();
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca_params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
    let mut ca_distinguished_name = DistinguishedName::new();
    ca_distinguished_name.push(DnType::CommonName, "Codex affinity test CA");
    ca_params.distinguished_name = ca_distinguished_name;
    let ca_key_pair = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)?;
    let ca = CertifiedIssuer::self_signed(ca_params, ca_key_pair)?;

    let mut server_params = CertificateParams::new(vec![
        "chatgpt.com".to_string(),
        "api.openai.com".to_string(),
    ])?;
    server_params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    server_params.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyEncipherment,
    ];
    let server_key_pair = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)?;
    let server_cert = server_params.signed_by(&server_key_pair, &ca)?;

    Ok(TlsMaterial {
        ca_cert_pem: ca.pem(),
        server_cert: server_cert.der().clone(),
        server_key: PrivateKeyDer::from(server_key_pair),
    })
}

fn run_tls_intercepting_proxy(
    listener: TcpListener,
    config: Arc<rustls::ServerConfig>,
    request_tx: mpsc::Sender<Result<CapturedRequest, String>>,
    expected_requests: usize,
) -> io::Result<()> {
    for request_index in 0..expected_requests {
        let (mut stream, _) = listener.accept()?;
        configure_stream(&stream)?;
        let connect_head = read_http_head(&mut stream)?;
        let connect_authority = connect_head
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .ok_or_else(|| io::Error::other("CONNECT request line should include an authority"))?
            .to_string();
        stream.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")?;
        stream.flush()?;

        let connection =
            rustls::ServerConnection::new(Arc::clone(&config)).map_err(io::Error::other)?;
        let mut tls = rustls::StreamOwned::new(connection, stream);
        let request = capture_http_request(&mut tls, connect_authority);
        match request {
            Ok(request) => request_tx
                .send(Ok(request))
                .map_err(|_| io::Error::other("request receiver was dropped"))?,
            Err(error) => {
                let message = error.to_string();
                let _ = request_tx.send(Err(message));
                return Err(error);
            }
        }

        let response = match request_index {
            0 => concat!(
                "HTTP/1.1 200 OK\r\n",
                "content-length: 2\r\n",
                "connection: close\r\n",
                "set-cookie: __cflb=west; Path=/; Secure; HttpOnly\r\n",
                "set-cookie: chatgpt_session=secret; Path=/; Secure; HttpOnly\r\n",
                "\r\n",
                "ok",
            ),
            1 => concat!(
                "HTTP/1.1 200 OK\r\n",
                "content-length: 2\r\n",
                "connection: close\r\n",
                "set-cookie: __cflb=central; Path=/; Secure; HttpOnly\r\n",
                "set-cookie: chatgpt_session=still-secret; Path=/; Secure; HttpOnly\r\n",
                "\r\n",
                "ok",
            ),
            _ => concat!(
                "HTTP/1.1 200 OK\r\n",
                "content-length: 2\r\n",
                "connection: close\r\n",
                "\r\n",
                "ok",
            ),
        };
        tls.write_all(response.as_bytes())?;
        tls.flush()?;
    }
    Ok(())
}

fn configure_stream(stream: &TcpStream) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))
}

fn capture_http_request(
    stream: &mut impl Read,
    connect_authority: String,
) -> io::Result<CapturedRequest> {
    let request_head = read_http_head(stream)?;
    let mut lines = request_head.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| io::Error::other("HTTP request should include a request line"))?
        .to_string();
    let mut headers: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in lines.filter(|line| !line.is_empty()) {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| io::Error::other(format!("invalid HTTP header: {line}")))?;
        headers
            .entry(name.to_ascii_lowercase())
            .or_default()
            .push(value.trim().to_string());
    }
    Ok(CapturedRequest {
        connect_authority,
        request_line,
        headers,
    })
}

fn read_http_head(stream: &mut impl Read) -> io::Result<String> {
    const MAX_HEADER_BYTES: usize = 64 * 1024;
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n\r\n") {
        if bytes.len() == MAX_HEADER_BYTES {
            return Err(io::Error::other("HTTP headers exceeded test limit"));
        }
        let mut byte = [0];
        stream.read_exact(&mut byte)?;
        bytes.push(byte[0]);
    }
    String::from_utf8(bytes).map_err(io::Error::other)
}

async fn initialize_exec_server(server: &mut ExecServerHarness) -> anyhow::Result<()> {
    let initialize_id = server
        .send_request(
            "initialize",
            serde_json::to_value(InitializeParams {
                client_name: "cloudflare-affinity-test".to_string(),
                resume_session_id: None,
            })?,
        )
        .await?;
    let _: Value = wait_for_response(server, initialize_id).await?;
    server
        .send_notification("initialized", serde_json::json!({}))
        .await
}

async fn execute_http_request(
    server: &mut ExecServerHarness,
    url: &str,
    request_id: &str,
) -> anyhow::Result<HttpRequestResponse> {
    let response_id = server
        .send_request(
            "http/request",
            serde_json::to_value(HttpRequestParams {
                method: "POST".to_string(),
                url: url.to_string(),
                headers: Vec::new(),
                body: None,
                timeout_ms: Some(5_000),
                redirect_policy: HttpRedirectPolicy::Follow,
                request_id: request_id.to_string(),
                stream_response: false,
            })?,
        )
        .await?;
    wait_for_response(server, response_id).await
}

async fn wait_for_response<T>(
    server: &mut ExecServerHarness,
    request_id: RequestId,
) -> anyhow::Result<T>
where
    T: DeserializeOwned,
{
    let message = server
        .wait_for_event(|event| match event {
            JSONRPCMessage::Response(JSONRPCResponse { id, .. })
            | JSONRPCMessage::Error(codex_exec_server_protocol::JSONRPCError { id, .. }) => {
                id == &request_id
            }
            _ => false,
        })
        .await?;
    match message {
        JSONRPCMessage::Response(JSONRPCResponse { result, .. }) => {
            Ok(serde_json::from_value(result)?)
        }
        JSONRPCMessage::Error(error) => {
            anyhow::bail!("exec-server returned an error for {request_id:?}: {error:?}")
        }
        _ => unreachable!("predicate only accepts responses for the requested id"),
    }
}
