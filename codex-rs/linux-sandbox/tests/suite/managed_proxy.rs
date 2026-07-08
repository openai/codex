#![cfg(target_os = "linux")]
#![allow(clippy::unwrap_used)]

use codex_core::exec_env::create_env;
use codex_protocol::config_types::ShellEnvironmentPolicy;
use codex_protocol::models::PermissionProfile;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::fs;
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

async fn command_available(program: &str) -> bool {
    matches!(
        Command::new(program)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await,
        Ok(status) if status.success()
    )
}

async fn should_skip_bwrap_tests() -> bool {
    let mut env = create_env_from_core_vars();
    strip_proxy_env(&mut env);

    let output = run_linux_sandbox_direct(
        &["bash", "-c", "true"],
        &PermissionProfile::read_only(),
        /*allow_network_for_proxy*/ false,
        /*dns_domain_policy*/ None,
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
        /*dns_domain_policy*/ None,
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
    dns_domain_policy: Option<&str>,
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
    if let Some(policy) = dns_domain_policy {
        args.extend(["--dns-domain-policy".to_string(), policy.to_string()]);
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
        /*dns_domain_policy*/ None,
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
        /*dns_domain_policy*/ None,
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
        /*dns_domain_policy*/ None,
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

    if !command_available("python3").await {
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
        /*dns_domain_policy*/ None,
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
async fn managed_dns_applies_domain_policy_and_drops_setup_capabilities() {
    if managed_proxy_skip_reason().await.is_some()
        || !command_available("python3").await
        || !command_available("cc").await
    {
        return;
    }

    let temp = tempfile::tempdir().expect("temporary loader-hook directory");
    let source = temp.path().join("cap_hook.c");
    let library = temp.path().join("cap_hook.so");
    let hook_log = temp.path().join("cap_hook.log");
    fs::write(
        &source,
        r#"#define _GNU_SOURCE
#include <dlfcn.h>
#include <netdb.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
typedef int (*getaddrinfo_fn)(const char *, const char *, const struct addrinfo *, struct addrinfo **);
static int dns_name_equal(const char *node, const char *fixture) {
    size_t fixture_len;
    if (!node || !fixture) return 0;
    fixture_len = strlen(fixture);
    return !strcmp(node, fixture) ||
        (!strncmp(node, fixture, fixture_len) && node[fixture_len] == '.' && !node[fixture_len + 1]);
}
static const char *resolver_fixture_address(const char *node) {
    const char *fixture4 = getenv("DNS_RESOLVER_FIXTURE4");
    const char *fixture6 = getenv("DNS_RESOLVER_FIXTURE6");
    const char *denied_canonical = getenv("DNS_RESOLVER_FIXTURE_DENIED_CANONICAL");
    char exe[4096];
    ssize_t len;
    if (!dns_name_equal(node, fixture4) && !dns_name_equal(node, fixture6) &&
        !dns_name_equal(node, denied_canonical)) return NULL;
    len = readlink("/proc/self/exe", exe, sizeof(exe) - 1);
    if (len < 0) return NULL;
    exe[len] = 0;
    if (!strstr(exe, "codex-linux-sandbox")) return NULL;
    if (dns_name_equal(node, fixture4)) return "192.0.2.53";
    if (dns_name_equal(node, fixture6)) return "2001:db8::53";
    return "192.0.2.54";
}
int getaddrinfo(const char *node, const char *service, const struct addrinfo *hints, struct addrinfo **res) {
    getaddrinfo_fn real = (getaddrinfo_fn)dlsym(RTLD_NEXT, "getaddrinfo");
    const char *address = resolver_fixture_address(node);
    struct addrinfo numeric = {0};
    if (!real) return EAI_SYSTEM;
    if (!address) return real(node, service, hints, res);
    if (hints) {
        numeric.ai_socktype = hints->ai_socktype;
        numeric.ai_protocol = hints->ai_protocol;
    }
    numeric.ai_flags = AI_NUMERICHOST;
    numeric.ai_family = strchr(address, ':') ? AF_INET6 : AF_INET;
    int status = real(address, service, &numeric, res);
    if (!status && hints && (hints->ai_flags & AI_CANONNAME) && *res) {
        const char *canonical = dns_name_equal(node, getenv("DNS_RESOLVER_FIXTURE_DENIED_CANONICAL"))
            ? "blocked.fixture.test"
            : (strchr(address, ':') ? "canonical6.fixture.test" : "canonical.fixture.test");
        char *copy = strdup(canonical);
        if (!copy) { freeaddrinfo(*res); *res = NULL; return EAI_MEMORY; }
        (*res)->ai_canonname = copy;
    }
    return status;
}
__attribute__((constructor)) static void record_caps(void) {
    const char *path = getenv("DNS_CAP_LOG");
    if (!path) return;
    FILE *status = fopen("/proc/self/status", "r");
    char line[256], exe[4096];
    unsigned long long caps = 0;
    while (status && fgets(line, sizeof(line), status))
        if (!strncmp(line, "Cap", 3)) caps |= strtoull(strchr(line, '\t') + 1, NULL, 16);
    if (status) fclose(status);
    ssize_t len = readlink("/proc/self/exe", exe, sizeof(exe) - 1);
    if (len < 0) return;
    exe[len] = 0;
    FILE *out = fopen(path, "a");
    if (out) { fprintf(out, "%s %llx\n", exe, caps); fclose(out); }
}
"#,
    )
    .expect("write loader hook");
    let build = Command::new("cc")
        .args(["-shared", "-fPIC", "-o"])
        .arg(&library)
        .arg(&source)
        .arg("-ldl")
        .output()
        .await
        .expect("compile loader hook");
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let mut env = create_env_from_core_vars();
    strip_proxy_env(&mut env);
    env.insert("HTTP_PROXY".to_string(), "http://127.0.0.1:9".to_string());
    env.insert("LD_PRELOAD".to_string(), library.display().to_string());
    env.insert("DNS_CAP_LOG".to_string(), hook_log.display().to_string());
    env.insert(
        "DNS_RESOLVER_FIXTURE4".to_string(),
        "resolver.fixture.test".to_string(),
    );
    env.insert(
        "DNS_RESOLVER_FIXTURE6".to_string(),
        "resolver6.fixture.test".to_string(),
    );
    env.insert(
        "DNS_RESOLVER_FIXTURE_DENIED_CANONICAL".to_string(),
        "denied-canonical.fixture.test".to_string(),
    );
    let policy = r#"{"allowedDomains":["localhost","**.fixture.test"],"deniedDomains":["blocked.fixture.test"]}"#;
    let script = r#"
import socket, struct
assert open('/etc/resolv.conf').read() == 'nameserver 127.0.0.1\n'
def skip_name(wire, offset):
    while True:
        size = wire[offset]
        offset += 1
        if size == 0: return offset
        if size & 0xc0 == 0xc0: return offset + 1
        offset += size
def query(name, qtype=1, tcp=False):
    labels = b''.join(bytes([len(part)]) + part.encode() for part in name.split('.')) + b'\0'
    wire = struct.pack('!HHHHHH', 1, 0x100, 1, 0, 0, 0) + labels + struct.pack('!HH', qtype, 1)
    if tcp:
        with socket.create_connection(('127.0.0.1', 53)) as sock:
            stream = sock.makefile('rwb')
            stream.write(struct.pack('!H', len(wire)) + wire); stream.flush()
            size = struct.unpack('!H', stream.read(2))[0]
            reply = stream.read(size)
            assert len(reply) == size
    else:
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sock:
            sock.sendto(wire, ('127.0.0.1', 53)); reply = sock.recv(65535)
    question_count, answer_count = struct.unpack('!HH', reply[4:8])
    offset = 12
    for _ in range(question_count): offset = skip_name(reply, offset) + 4
    answer_types = []
    for _ in range(answer_count):
        offset = skip_name(reply, offset)
        answer_types.append(struct.unpack('!H', reply[offset:offset + 2])[0])
        data_length = struct.unpack('!H', reply[offset + 8:offset + 10])[0]
        offset += 10 + data_length
    return reply[3] & 15, answer_types
assert query('resolver.fixture.test') == (0, [5, 1])
assert query('resolver6.fixture.test', qtype=28) == (0, [5, 28])
assert query('resolver.fixture.test', qtype=5) == (0, [5])
assert query('resolver.fixture.test', tcp=True) == (0, [5, 1])
assert query('denied-canonical.fixture.test') == (0, [1])
assert query('denied-canonical.fixture.test', qtype=5) == (5, [])
assert query('blocked.fixture.test') == (5, [])
resolved = socket.getaddrinfo('resolver.fixture.test', 0, socket.AF_INET, 0, 0, socket.AI_CANONNAME)
assert {entry[4][0] for entry in resolved} == {'192.0.2.53'}
assert resolved[0][3] == 'canonical.fixture.test'
try: socket.getaddrinfo('blocked.fixture.test', 0)
except socket.gaierror: pass
else: raise AssertionError('denied name resolved')
with open('/proc/self/status') as status:
    caps = [int(line.split()[1], 16) for line in status if line.startswith(('CapInh:', 'CapPrm:', 'CapEff:', 'CapBnd:', 'CapAmb:'))]
assert caps == [0] * 5, caps
"#;
    let output = run_linux_sandbox_direct(
        &["python3", "-c", script],
        &PermissionProfile::Disabled,
        /*allow_network_for_proxy*/ true,
        /*dns_domain_policy*/ Some(policy),
        env,
        NETWORK_TIMEOUT_MS,
    )
    .await;

    assert!(
        output.status.success(),
        "expected policy-checked DNS with no residual capabilities: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let hook_log = fs::read_to_string(&hook_log).expect("read loader-hook log");
    assert!(
        hook_log.lines().any(|line| line.contains("python")),
        "{hook_log}"
    );
    assert!(
        hook_log.lines().all(|line| line.ends_with(" 0")),
        "loader hook ran with capabilities: {hook_log}"
    );
}
