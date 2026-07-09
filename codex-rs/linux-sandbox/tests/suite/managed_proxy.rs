#![cfg(target_os = "linux")]
#![allow(clippy::unwrap_used)]

use codex_core::exec_env::create_env;
use codex_network_proxy::DNS_PROXY_ENV_KEY;
use codex_network_proxy::DNS_PROXY_SESSION_PREFACE;
use codex_protocol::config_types::ShellEnvironmentPolicy;
use codex_protocol::models::PermissionProfile;
use hickory_proto::op::Message;
use hickory_proto::op::ResponseCode;
use hickory_proto::rr::RData;
use hickory_proto::rr::Record;
use hickory_proto::rr::rdata::A;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::io::Read;
use std::io::Write;
use std::net::Ipv4Addr;
use std::net::TcpListener;
use std::process::Output;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

const BWRAP_UNAVAILABLE_ERR: &str = "bubblewrap is unavailable: no system bwrap was found";
const NETWORK_TIMEOUT_MS: u64 = 4_000;
const MANAGED_PROXY_PERMISSION_ERR_SNIPPETS: &[&str] = &[
    "loopback: Failed RTM_NEWADDR",
    "loopback: Failed RTM_NEWLINK",
    "setting up uid map: Permission denied",
    "No permissions to create a new namespace",
    "error isolating Linux network namespace for proxy mode",
];

const PROXY_ENV_KEYS: &[&str] = &[
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "FTP_PROXY",
    "YARN_HTTP_PROXY",
    "YARN_HTTPS_PROXY",
    "NPM_CONFIG_HTTP_PROXY",
    "NPM_CONFIG_HTTPS_PROXY",
    "NPM_CONFIG_PROXY",
    "BUNDLE_HTTP_PROXY",
    "BUNDLE_HTTPS_PROXY",
    "PIP_PROXY",
    "DOCKER_HTTP_PROXY",
    "DOCKER_HTTPS_PROXY",
    DNS_PROXY_ENV_KEY,
];

fn create_env_from_core_vars() -> HashMap<String, String> {
    let policy = ShellEnvironmentPolicy::default();
    create_env(&policy, /*thread_id*/ None)
}

fn strip_proxy_env(env: &mut HashMap<String, String>) {
    for key in PROXY_ENV_KEYS {
        env.remove(*key);
        let lower = key.to_ascii_lowercase();
        env.remove(lower.as_str());
    }
}

fn is_bwrap_unavailable_output(output: &Output) -> bool {
    String::from_utf8_lossy(&output.stderr).contains(BWRAP_UNAVAILABLE_ERR)
}

async fn should_skip_bwrap_tests() -> bool {
    let mut env = create_env_from_core_vars();
    strip_proxy_env(&mut env);

    let output = run_linux_sandbox_direct(
        &["bash", "-c", "true"],
        &PermissionProfile::read_only(),
        /*allow_network_for_proxy*/ false,
        env,
        NETWORK_TIMEOUT_MS,
    )
    .await;
    is_bwrap_unavailable_output(&output)
}

fn is_managed_proxy_permission_error(stderr: &str) -> bool {
    MANAGED_PROXY_PERMISSION_ERR_SNIPPETS
        .iter()
        .any(|snippet| stderr.contains(snippet))
}

async fn managed_proxy_skip_reason() -> Option<String> {
    if should_skip_bwrap_tests().await {
        return Some("bubblewrap is unavailable in this environment".to_string());
    }

    let mut env = create_env_from_core_vars();
    strip_proxy_env(&mut env);
    env.insert("HTTP_PROXY".to_string(), "http://127.0.0.1:9".to_string());

    let output = run_linux_sandbox_direct(
        &["bash", "-c", "true"],
        &PermissionProfile::Disabled,
        /*allow_network_for_proxy*/ true,
        env,
        NETWORK_TIMEOUT_MS,
    )
    .await;
    if output.status.success() {
        return None;
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if is_managed_proxy_permission_error(stderr.as_ref()) {
        return Some(format!(
            "managed proxy requires kernel namespace privileges unavailable here: {}",
            stderr.trim()
        ));
    }

    None
}

async fn run_linux_sandbox_direct(
    command: &[&str],
    permission_profile: &PermissionProfile,
    allow_network_for_proxy: bool,
    env: HashMap<String, String>,
    timeout_ms: u64,
) -> Output {
    let cwd = std::env::current_dir().expect("current directory should exist");
    let permission_profile_json =
        serde_json::to_string(permission_profile).expect("permission profile should serialize");

    let mut args = vec![
        "--sandbox-policy-cwd".to_string(),
        cwd.to_string_lossy().to_string(),
        "--permission-profile".to_string(),
        permission_profile_json,
    ];
    if allow_network_for_proxy {
        args.push("--allow-network-for-proxy".to_string());
    }
    args.push("--".to_string());
    args.extend(command.iter().map(|entry| (*entry).to_string()));

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_codex-linux-sandbox"));
    cmd.args(args)
        .current_dir(cwd)
        .env_clear()
        .envs(env)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    tokio::time::timeout(Duration::from_millis(timeout_ms), cmd.output())
        .await
        .expect("sandbox command should not time out")
        .expect("sandbox command should execute")
}

#[tokio::test]
async fn managed_proxy_mode_fails_closed_without_proxy_env() {
    if let Some(skip_reason) = managed_proxy_skip_reason().await {
        eprintln!("skipping managed proxy test: {skip_reason}");
        return;
    }

    let mut env = create_env_from_core_vars();
    strip_proxy_env(&mut env);

    let output = run_linux_sandbox_direct(
        &["bash", "-c", "true"],
        &PermissionProfile::Disabled,
        /*allow_network_for_proxy*/ true,
        env,
        NETWORK_TIMEOUT_MS,
    )
    .await;

    assert_eq!(output.status.success(), false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("managed proxy mode requires proxy environment variables"),
        "expected fail-closed managed-proxy message, got stderr: {stderr}"
    );
}

#[tokio::test]
async fn managed_proxy_mode_routes_through_bridge_and_blocks_direct_egress() {
    if let Some(skip_reason) = managed_proxy_skip_reason().await {
        eprintln!("skipping managed proxy test: {skip_reason}");
        return;
    }

    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind proxy listener");
    let proxy_port = listener
        .local_addr()
        .expect("proxy listener local addr")
        .port();
    let (request_tx, request_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept proxy connection");
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .expect("set read timeout");
        let mut buf = [0_u8; 4096];
        let read = stream.read(&mut buf).expect("read proxy request");
        let request = String::from_utf8_lossy(&buf[..read]).to_string();
        request_tx.send(request).expect("send proxy request");
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")
            .expect("write proxy response");
    });

    let mut env = create_env_from_core_vars();
    strip_proxy_env(&mut env);
    env.insert(
        "HTTP_PROXY".to_string(),
        format!("http://127.0.0.1:{proxy_port}"),
    );

    let routed_output = run_linux_sandbox_direct(
        &[
            "bash",
            "-c",
            "proxy=\"${HTTP_PROXY#*://}\"; host=\"${proxy%%:*}\"; port=\"${proxy##*:}\"; exec 3<>/dev/tcp/${host}/${port}; printf 'GET http://example.com/ HTTP/1.1\\r\\nHost: example.com\\r\\n\\r\\n' >&3; IFS= read -r line <&3; printf '%s\\n' \"$line\"",
        ],
        &PermissionProfile::Disabled,
        /*allow_network_for_proxy*/ true,
        env.clone(),
        NETWORK_TIMEOUT_MS,
    )
    .await;

    assert_eq!(
        routed_output.status.success(),
        true,
        "expected routed command to execute successfully; status={:?}; stdout={}; stderr={}",
        routed_output.status.code(),
        String::from_utf8_lossy(&routed_output.stdout),
        String::from_utf8_lossy(&routed_output.stderr)
    );
    let stdout = String::from_utf8_lossy(&routed_output.stdout);
    assert!(
        stdout.contains("HTTP/1.1 200 OK"),
        "expected bridge-routed proxy response, got stdout: {stdout}"
    );

    let request = request_rx
        .recv_timeout(Duration::from_secs(3))
        .expect("expected proxy request");
    assert!(
        request.contains("GET http://example.com/ HTTP/1.1"),
        "expected HTTP proxy absolute-form request, got request: {request}"
    );

    let direct_egress_output = run_linux_sandbox_direct(
        &["bash", "-c", "echo hi > /dev/tcp/192.0.2.1/80"],
        &PermissionProfile::Disabled,
        /*allow_network_for_proxy*/ true,
        env,
        NETWORK_TIMEOUT_MS,
    )
    .await;
    assert_eq!(direct_egress_output.status.success(), false);
}

#[tokio::test]
async fn managed_proxy_mode_denies_af_unix_socket_but_allows_socketpair() {
    if let Some(skip_reason) = managed_proxy_skip_reason().await {
        eprintln!("skipping managed proxy test: {skip_reason}");
        return;
    }

    let python_available = Command::new("bash")
        .arg("-c")
        .arg("command -v python3 >/dev/null")
        .status()
        .await
        .expect("python3 probe should execute")
        .success();
    if !python_available {
        eprintln!("skipping managed proxy AF_UNIX test: python3 is unavailable");
        return;
    }

    let mut env = create_env_from_core_vars();
    strip_proxy_env(&mut env);
    env.insert("HTTP_PROXY".to_string(), "http://127.0.0.1:9".to_string());

    let output = run_linux_sandbox_direct(
        &[
            "python3",
            "-c",
            "import socket,sys\ntry:\n    socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)\nexcept PermissionError:\n    pass\nexcept OSError:\n    sys.exit(2)\nelse:\n    sys.exit(1)\nleft,right = socket.socketpair(socket.AF_UNIX, socket.SOCK_STREAM)\nleft.sendall(b'ok')\nif right.recv(2) != b'ok':\n    sys.exit(3)\n",
        ],
        &PermissionProfile::Disabled,
        /*allow_network_for_proxy*/ true,
        env,
        NETWORK_TIMEOUT_MS,
    )
    .await;

    assert_eq!(
        output.status.code(),
        Some(0),
        "expected AF_UNIX socket creation to be denied and socketpair to work; status={:?}; stdout={}; stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
async fn managed_proxy_mode_routes_native_dns_through_bridge() {
    if let Some(skip_reason) = managed_proxy_skip_reason().await {
        eprintln!("skipping managed proxy test: {skip_reason}");
        return;
    }

    if !command_available("python3").await {
        eprintln!("skipping managed proxy DNS test: python3 is unavailable");
        return;
    }

    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind DNS proxy listener");
    let proxy_port = listener.local_addr().expect("DNS proxy local addr").port();
    let (query_tx, query_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept DNS proxy connection");
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .expect("set DNS proxy read timeout");
        let mut preface = [0_u8; DNS_PROXY_SESSION_PREFACE.len()];
        stream.read_exact(&mut preface).expect("read DNS preface");
        assert_eq!(&preface, DNS_PROXY_SESSION_PREFACE);
        stream
            .write_all(DNS_PROXY_SESSION_PREFACE)
            .expect("write DNS preface ack");
        let query = read_frame(&mut stream).expect("read DNS query");
        let response = dns_a_response(&query, [203, 0, 113, 9]);
        query_tx.send(query).expect("send DNS query");
        write_frame(&mut stream, &response).expect("write DNS response");
    });

    let mut env = create_env_from_core_vars();
    strip_proxy_env(&mut env);
    env.insert(
        DNS_PROXY_ENV_KEY.to_string(),
        format!("tcp://127.0.0.1:{proxy_port}"),
    );

    let output = run_linux_sandbox_direct(
        &[
            "python3",
            "-c",
            "import os,socket,sys\nif 'CODEX_NETWORK_PROXY_DNS' in os.environ:\n    sys.exit(3)\ninfo = socket.getaddrinfo('fixture.test', 80, socket.AF_INET, socket.SOCK_STREAM)\nprint(info[0][4][0])\n",
        ],
        &PermissionProfile::Disabled,
        /*allow_network_for_proxy*/ true,
        env,
        NETWORK_TIMEOUT_MS,
    )
    .await;

    assert_eq!(
        output.status.success(),
        true,
        "expected native DNS to resolve through bridge; status={:?}; stdout={}; stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "203.0.113.9"
    );
    let query = query_rx
        .recv_timeout(Duration::from_secs(3))
        .expect("expected DNS query through proxy bridge");
    assert!(
        dns_query_name(&query).is_some_and(|name| name == "fixture.test"),
        "expected fixture.test DNS query, got wire bytes: {query:?}"
    );
}

fn dns_query_name(query: &[u8]) -> Option<String> {
    let query = Message::from_vec(query).ok()?;
    let question = query.queries().first()?;
    Some(question.name().to_utf8().trim_end_matches('.').to_string())
}

fn dns_a_response(query: &[u8], address: [u8; 4]) -> Vec<u8> {
    let query = Message::from_vec(query).expect("parse DNS query");
    let question = query
        .queries()
        .first()
        .expect("DNS query should include one question")
        .clone();
    let mut response = Message::error_msg(query.id(), query.op_code(), ResponseCode::NoError);
    response
        .set_recursion_desired(query.recursion_desired())
        .set_recursion_available(true)
        .add_query(question.clone())
        .add_answer(Record::from_rdata(
            question.name().clone(),
            /*ttl*/ 0,
            RData::A(A(Ipv4Addr::from(address))),
        ));
    response.to_vec().expect("serialize DNS response")
}

fn write_frame(writer: &mut impl Write, payload: &[u8]) -> std::io::Result<()> {
    let len = u16::try_from(payload.len()).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "DNS message exceeds maximum wire length",
        )
    })?;
    writer.write_all(&len.to_be_bytes())?;
    writer.write_all(payload)
}

fn read_frame(reader: &mut impl Read) -> std::io::Result<Vec<u8>> {
    let mut len = [0; 2];
    reader.read_exact(&mut len)?;
    let mut payload = vec![0; usize::from(u16::from_be_bytes(len))];
    reader.read_exact(&mut payload)?;
    Ok(payload)
}
