#![cfg(target_os = "linux")]
#![allow(clippy::unwrap_used)]

use codex_core::exec_env::create_env;
use codex_protocol::config_types::ShellEnvironmentPolicy;
use codex_protocol::models::PermissionProfile;
use codex_sandboxing::ingress::INGRESS_LISTENER_FD_ENV_VAR;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::io::Read;
use std::io::Write;
use std::net::Ipv4Addr;
use std::net::Shutdown;
use std::net::TcpListener;
use std::net::TcpStream;
use std::os::fd::AsRawFd;
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
    "Creating new namespace failed: Operation not permitted",
    "SeccompInstall(Seccomp(Os { code: 22",
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

async fn ingress_skip_reason() -> Option<String> {
    if should_skip_bwrap_tests().await {
        return Some("bubblewrap is unavailable in this environment".to_string());
    }

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
    if output.status.success() {
        return None;
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if is_managed_proxy_permission_error(stderr.as_ref()) {
        return Some(format!(
            "ingress requires kernel namespace privileges unavailable here: {}",
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
    let mut cmd = linux_sandbox_command(
        command,
        permission_profile,
        allow_network_for_proxy,
        /*ingress*/ None,
        env,
    );
    tokio::time::timeout(Duration::from_millis(timeout_ms), cmd.output())
        .await
        .expect("sandbox command should not time out")
        .expect("sandbox command should execute")
}

fn linux_sandbox_command(
    command: &[&str],
    permission_profile: &PermissionProfile,
    allow_network_for_proxy: bool,
    ingress: Option<u16>,
    env: HashMap<String, String>,
) -> Command {
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
    if let Some(ingress_port) = ingress {
        args.push("--ingress".to_string());
        args.push(ingress_port.to_string());
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
    cmd
}

fn clear_close_on_exec(fd: libc::c_int) {
    // SAFETY: `fd` comes from this process's live `TcpListener`.
    let fd_flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    assert!(fd_flags >= 0, "listener fd flags should be readable");
    // SAFETY: `fd` comes from this process's live `TcpListener`.
    let result = unsafe { libc::fcntl(fd, libc::F_SETFD, fd_flags & !libc::FD_CLOEXEC) };
    assert_eq!(result, 0, "listener fd should be inherited");
}

fn connect_ingress_stream(port: u16) -> TcpStream {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        match TcpStream::connect((Ipv4Addr::LOCALHOST, port)) {
            Ok(stream) => return stream,
            Err(_) if std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(error) => panic!("client should connect to ingress listener: {error}"),
        }
    }
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
async fn ingress_routes_parent_connection_into_sandbox_local_server() {
    if let Some(skip_reason) = ingress_skip_reason().await {
        eprintln!("skipping ingress test: {skip_reason}");
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
        eprintln!("skipping ingress test: python3 is unavailable");
        return;
    }

    let listener =
        TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind parent-visible ingress listener");
    clear_close_on_exec(listener.as_raw_fd());
    let parent_visible_port = listener
        .local_addr()
        .expect("ingress listener local addr")
        .port();
    let ingress_port = 4173;

    let mut env = create_env_from_core_vars();
    strip_proxy_env(&mut env);
    env.insert(
        INGRESS_LISTENER_FD_ENV_VAR.to_string(),
        listener.as_raw_fd().to_string(),
    );

    let server_script = format!(
        concat!(
            "import http.server\n",
            "class Handler(http.server.BaseHTTPRequestHandler):\n",
            "    def do_GET(self):\n",
            "        body = b'ingress-ok'\n",
            "        self.send_response(200)\n",
            "        self.send_header('Content-Length', str(len(body)))\n",
            "        self.end_headers()\n",
            "        self.wfile.write(body)\n",
            "    def log_message(self, format, *args):\n",
            "        pass\n",
            "server = http.server.HTTPServer(('127.0.0.1', {ingress_port}), Handler)\n",
            "server.handle_request()\n",
        ),
        ingress_port = ingress_port,
    );
    let command = ["python3", "-c", server_script.as_str()];
    let mut cmd = linux_sandbox_command(
        &command,
        &PermissionProfile::read_only(),
        /*allow_network_for_proxy*/ false,
        Some(ingress_port),
        env,
    );
    cmd.kill_on_drop(true);
    let child = cmd.spawn().expect("ingress sandbox should spawn");

    let mut ingress_stream = connect_ingress_stream(parent_visible_port);
    ingress_stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("set ingress read timeout");
    ingress_stream
        .write_all(b"GET / HTTP/1.1\r\nHost: terminal.local\r\nConnection: close\r\n\r\n")
        .expect("write ingress request");
    ingress_stream
        .shutdown(Shutdown::Write)
        .expect("close ingress request body");
    let mut response = Vec::new();
    let response_result = ingress_stream.read_to_end(&mut response);
    drop(ingress_stream);

    let output = tokio::time::timeout(
        Duration::from_millis(NETWORK_TIMEOUT_MS),
        child.wait_with_output(),
    )
    .await
    .expect("ingress sandbox should exit after one request")
    .expect("ingress sandbox should execute");
    response_result.unwrap_or_else(|error| {
        panic!(
            "read ingress response: {error}; status={:?}; stdout={}; stderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    });
    let response = String::from_utf8_lossy(&response);
    assert!(
        response.contains("ingress-ok"),
        "expected ingress response body, got response: {response}"
    );
    assert!(
        output.status.success(),
        "expected ingress sandbox to execute successfully; status={:?}; stdout={}; stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
