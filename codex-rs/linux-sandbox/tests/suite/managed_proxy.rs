//! Linux managed-proxy integration tests exercise the real sandbox with isolated environments.
//! Handoff must preserve routing and attribution without leaking privileged sockets; cancellation
//! must close active routes while filesystem and network restrictions remain enforced.

#![cfg(target_os = "linux")]
#![allow(clippy::unwrap_used)]

use codex_core::exec_env::create_env;
use codex_network_proxy::PROXY_ATTRIBUTION_TOKEN_ENV_KEY;
use codex_network_proxy::write_attribution_frame;
use codex_protocol::config_types::ShellEnvironmentPolicy;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::io::Read;
use std::io::Write;
use std::net::Ipv4Addr;
use std::net::Shutdown;
use std::net::TcpListener;
use std::net::TcpStream;
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Output;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;
use tempfile::NamedTempFile;
use tokio::process::Command;
use url::Url;

#[path = "managed_proxy_unix_sockets_tests.rs"]
mod unix_sockets;

const BWRAP_UNAVAILABLE_ERR: &str = "bubblewrap is unavailable: no system bwrap was found";
const NETWORK_TIMEOUT_MS: u64 = 4_000;
const OPERATION_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 8);
const SANDBOX_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 60);
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
    "WS_PROXY",
    "WSS_PROXY",
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
    let stderr = String::from_utf8_lossy(&output.stderr);
    is_bwrap_unavailable_output(&output)
        || (!output.status.success() && is_managed_proxy_permission_error(stderr.as_ref()))
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
    let mut command =
        linux_sandbox_command(command, permission_profile, allow_network_for_proxy, env);
    tokio::time::timeout(Duration::from_millis(timeout_ms), command.output())
        .await
        .expect("sandbox command should not time out")
        .expect("sandbox command should execute")
}

fn linux_sandbox_command(
    command: &[&str],
    permission_profile: &PermissionProfile,
    allow_network_for_proxy: bool,
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
        args.push("--managed-network".to_string());
        args.push("{}".to_string());
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

#[tokio::test]
async fn sandboxed_commands_have_a_seccomp_filtered_namespace_reaper() {
    if should_skip_bwrap_tests().await {
        eprintln!("skipping bwrap test: bubblewrap is unavailable");
        return;
    }

    let mut env = create_env_from_core_vars();
    strip_proxy_env(&mut env);

    assert_seccomp_filtered_namespace_reaper(
        &PermissionProfile::read_only(),
        /*allow_network_for_proxy*/ false,
        env,
    )
    .await;
}

#[tokio::test]
async fn managed_proxy_commands_have_a_seccomp_filtered_namespace_reaper() {
    if let Some(skip_reason) = managed_proxy_skip_reason().await {
        eprintln!("skipping managed proxy test: {skip_reason}");
        return;
    }

    let mut env = create_env_from_core_vars();
    strip_proxy_env(&mut env);
    env.insert("HTTP_PROXY".to_string(), "http://127.0.0.1:9".to_string());

    for permission_profile in [PermissionProfile::read_only(), PermissionProfile::Disabled] {
        assert_seccomp_filtered_namespace_reaper(
            &permission_profile,
            /*allow_network_for_proxy*/ true,
            env.clone(),
        )
        .await;
    }
}

async fn assert_seccomp_filtered_namespace_reaper(
    permission_profile: &PermissionProfile,
    allow_network_for_proxy: bool,
    env: HashMap<String, String>,
) {
    let inherited_seccomp_filters = std::fs::read_to_string("/proc/self/status")
        .expect("test process status should be readable")
        .lines()
        .find_map(|line| line.strip_prefix("Seccomp_filters:"))
        .and_then(|count| count.trim().parse::<usize>().ok())
        .expect("test process status should expose its seccomp filter count");

    let output = run_linux_sandbox_direct(
        &[
            "bash",
            "-c",
            "if [ \"$(readlink /proc/1/ns/pid 2>/dev/null)\" != \"$(readlink /proc/self/ns/pid 2>/dev/null)\" ]; then printf 'namespace proc unavailable\\n'; exit 0; fi; printf '%s\\n' \"$PPID\"; awk '$1 == \"Seccomp:\" && FNR == NR { print $2 } $1 == \"Seccomp_filters:\" { print $2 }' /proc/1/status /proc/self/status",
        ],
        permission_profile,
        allow_network_for_proxy,
        env,
        NETWORK_TIMEOUT_MS,
    )
    .await;

    assert_eq!(
        output.status.success(),
        true,
        "sandboxed command should execute; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    if output.stdout == b"namespace proc unavailable\n" {
        eprintln!("skipping namespace reaper check: a namespaced proc mount is unavailable");
        return;
    }
    let expected_filter_count = inherited_seccomp_filters + 1;
    assert_eq!(
        output.stdout,
        format!("1\n2\n{expected_filter_count}\n{expected_filter_count}\n").as_bytes()
    );
}

#[tokio::test]
async fn namespace_reaper_collects_orphaned_descendants() {
    if should_skip_bwrap_tests().await {
        eprintln!("skipping bwrap test: bubblewrap is unavailable");
        return;
    }

    let mut env = create_env_from_core_vars();
    strip_proxy_env(&mut env);

    let output = run_linux_sandbox_direct(
        &[
            "bash",
            "-c",
            "if [ \"$(readlink /proc/1/ns/pid 2>/dev/null)\" != \"$(readlink /proc/self/ns/pid 2>/dev/null)\" ]; then printf 'namespace proc unavailable\\n'; exit 0; fi; orphan=$(bash -c 'sleep 0.05 </dev/null >/dev/null 2>&1 & printf \"%s\\n\" \"$!\"'); for _ in $(seq 1 100); do if [ ! -e \"/proc/$orphan\" ]; then printf 'orphan reaped\\n'; exit 0; fi; sleep 0.01; done; exit 1",
        ],
        &PermissionProfile::read_only(),
        /*allow_network_for_proxy*/ false,
        env,
        NETWORK_TIMEOUT_MS,
    )
    .await;

    assert_eq!(
        output.status.success(),
        true,
        "namespace init should reap orphaned descendants; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    if output.stdout == b"namespace proc unavailable\n" {
        eprintln!("skipping orphan reaping check: a namespaced proc mount is unavailable");
        return;
    }
    assert_eq!(output.stdout, b"orphan reaped\n");
}

#[tokio::test]
async fn unsupported_system_bwrap_falls_back_to_bundled_bwrap() {
    if option_env!("CODEX_BWRAP_SHA256")
        .is_some_and(|digest| digest.chars().any(|character| character != '0'))
    {
        eprintln!("skipping system bwrap fallback test: bundled binaries require a release digest");
        return;
    }

    if should_skip_bwrap_tests().await {
        eprintln!("skipping bwrap test: bubblewrap is unavailable");
        return;
    }

    let Some(system_bwrap) = codex_sandboxing::find_system_bwrap_in_path() else {
        eprintln!("skipping system bwrap fallback test: no system bubblewrap is available");
        return;
    };

    let tempdir = tempfile::tempdir().expect("create isolated sandbox installation");
    let sandbox_executable = tempdir.path().join("codex-linux-sandbox");
    let original_executable = env!("CARGO_BIN_EXE_codex-linux-sandbox");
    if std::fs::hard_link(original_executable, &sandbox_executable).is_err() {
        std::fs::copy(original_executable, &sandbox_executable).expect("copy sandbox executable");
    }

    let resources_dir = tempdir.path().join("codex-resources");
    std::fs::create_dir(&resources_dir).expect("create bundled resource directory");
    std::os::unix::fs::symlink(&system_bwrap, resources_dir.join("bwrap"))
        .expect("install bundled bubblewrap");

    let system_dir = tempdir.path().join("system");
    std::fs::create_dir(&system_dir).expect("create fake system binary directory");
    let unsupported_bwrap = system_dir.join("bwrap");
    std::fs::write(
        &unsupported_bwrap,
        "#!/bin/sh\nif [ \"$1\" = \"--help\" ]; then printf '%s\\n' '--perms'; exit 0; fi\nexit 91\n",
    )
    .expect("write unsupported system bubblewrap");
    std::fs::set_permissions(&unsupported_bwrap, std::fs::Permissions::from_mode(0o755))
        .expect("make unsupported system bubblewrap executable");

    let mut env = create_env_from_core_vars();
    strip_proxy_env(&mut env);
    let original_path = env.get("PATH").cloned().unwrap_or_default();
    env.insert(
        "PATH".to_string(),
        format!("{}:{original_path}", system_dir.display()),
    );

    let cwd = std::env::current_dir().expect("current directory should exist");
    let permission_profile =
        serde_json::to_string(&PermissionProfile::read_only()).expect("serialize profile");
    let output = tokio::time::timeout(
        Duration::from_millis(NETWORK_TIMEOUT_MS),
        Command::new(&sandbox_executable)
            .args([
                "--sandbox-policy-cwd",
                cwd.to_str().expect("UTF-8 current directory"),
                "--permission-profile",
                &permission_profile,
                "--",
                "bash",
                "-c",
                "printf 'bundled fallback\\n'",
            ])
            .current_dir(&cwd)
            .env_clear()
            .envs(env)
            .output(),
    )
    .await
    .expect("bundled bubblewrap should not time out")
    .expect("sandbox command should execute");

    assert_eq!(
        output.status.success(),
        true,
        "unsupported system bubblewrap should fall back to bundled bubblewrap; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"bundled fallback\n");
}

#[tokio::test]
async fn managed_proxy_full_filesystem_uses_minimal_dev_nodes() {
    if let Some(skip_reason) = managed_proxy_skip_reason().await {
        eprintln!("skipping managed proxy test: {skip_reason}");
        return;
    }

    let host_dev = std::fs::metadata("/dev").expect("host /dev should exist");
    let host_dev_id = format!("{}:{}", host_dev.dev(), host_dev.ino());
    let shared_memory_file = match NamedTempFile::new_in("/dev/shm") {
        Ok(file) => {
            std::fs::write(file.path(), "host-before").expect("seed /dev/shm file");
            Some(file)
        }
        Err(err) => {
            eprintln!("skipping /dev/shm interoperability check: {err}");
            None
        }
    };

    let mut env = create_env_from_core_vars();
    strip_proxy_env(&mut env);
    env.insert("HTTP_PROXY".to_string(), "http://127.0.0.1:9".to_string());
    if let Some(file) = &shared_memory_file {
        env.insert(
            "CODEX_TEST_SHM_PATH".to_string(),
            file.path().to_string_lossy().into_owned(),
        );
    }

    let output = run_linux_sandbox_direct(
        &[
            "bash",
            "-c",
            concat!(
                "set -e; ",
                "for node in null zero urandom; do test -c \"/dev/$node\"; done; ",
                "printf test >/dev/null; ",
                "head -c 1 /dev/zero >/dev/null; ",
                "head -c 1 /dev/urandom >/dev/null; ",
                "if command -v python3 >/dev/null 2>&1; then ",
                "python3 -c 'import os; assert len(os.urandom(16)) == 16'; ",
                "fi; ",
                "if [ -n \"${CODEX_TEST_SHM_PATH:-}\" ]; then ",
                "test \"$(cat \"$CODEX_TEST_SHM_PATH\")\" = host-before; ",
                "printf sandbox-after >\"$CODEX_TEST_SHM_PATH\"; ",
                "fi; ",
                "stat -c '%d:%i' /dev",
            ),
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
        "standard devices should be usable; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_ne!(
        String::from_utf8_lossy(&output.stdout).trim(),
        host_dev_id,
        "/dev should be synthetic rather than inherited from the host"
    );
    if let Some(file) = shared_memory_file {
        assert_eq!(
            std::fs::read_to_string(file.path()).expect("read /dev/shm file"),
            "sandbox-after"
        );
    }
}

#[tokio::test]
async fn managed_proxy_bridges_release_command_output_after_exit() {
    if let Some(skip_reason) = managed_proxy_skip_reason().await {
        eprintln!("skipping managed proxy test: {skip_reason}");
        return;
    }

    let mut env = create_env_from_core_vars();
    strip_proxy_env(&mut env);
    env.insert("HTTP_PROXY".to_string(), "http://127.0.0.1:9".to_string());

    let output = run_linux_sandbox_direct(
        &["bash", "-c", "printf 'bridge output closed\\n'"],
        &PermissionProfile::Disabled,
        /*allow_network_for_proxy*/ true,
        env,
        NETWORK_TIMEOUT_MS,
    )
    .await;

    assert_eq!(output.status.success(), true);
    assert_eq!(output.stdout, b"bridge output closed\n");
}

#[tokio::test]
async fn managed_proxy_readiness_survives_closed_standard_descriptors() {
    if let Some(skip_reason) = managed_proxy_skip_reason().await {
        eprintln!("skipping managed proxy test: {skip_reason}");
        return;
    }

    let mut env = create_env_from_core_vars();
    strip_proxy_env(&mut env);
    env.insert("HTTP_PROXY".to_string(), "http://127.0.0.1:9".to_string());

    let mut command = linux_sandbox_command(
        &["bash", "-c", "true"],
        &PermissionProfile::Disabled,
        /*allow_network_for_proxy*/ true,
        env,
    );
    unsafe {
        command.pre_exec(|| {
            libc::close(libc::STDIN_FILENO);
            libc::close(libc::STDOUT_FILENO);
            Ok(())
        });
    }

    let output = tokio::time::timeout(Duration::from_millis(NETWORK_TIMEOUT_MS), command.output())
        .await
        .expect("sandbox command should not time out")
        .expect("sandbox command should execute");

    assert_eq!(
        output.status.success(),
        true,
        "managed proxy readiness should survive closed standard descriptors; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
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
    env.insert(
        "WSS_PROXY".to_string(),
        format!("http://127.0.0.1:{proxy_port}"),
    );

    let sandbox_helper_dir = std::path::Path::new(env!("CARGO_BIN_EXE_codex-linux-sandbox"))
        .parent()
        .expect("sandbox helper should have a parent");
    let file_system_sandbox_policy =
        FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Minimal,
            },
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        }])
        .with_additional_readable_roots(
            std::env::current_dir()
                .expect("current directory should exist")
                .as_path(),
            &[AbsolutePathBuf::try_from(sandbox_helper_dir).expect("absolute helper dir")],
        );
    let permission_profile = PermissionProfile::from_runtime_permissions(
        &file_system_sandbox_policy,
        NetworkSandboxPolicy::Restricted,
    );

    let routed_output = run_linux_sandbox_direct(
        &[
            "bash",
            "-c",
            "proxy=\"${WSS_PROXY#*://}\"; host=\"${proxy%%:*}\"; port=\"${proxy##*:}\"; exec 3<>/dev/tcp/${host}/${port}; printf 'GET http://example.com/ HTTP/1.1\\r\\nHost: example.com\\r\\n\\r\\n' >&3; IFS= read -r line <&3; printf '%s\\n' \"$line\"",
        ],
        &permission_profile,
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

/// Runs as the model-controlled command, after the trusted listener handoff.
#[test]
#[ignore = "invoked inside the managed proxy sandbox"]
fn handoff_client() {
    let Ok(label) = std::env::var("CODEX_TEST_HANDOFF_LABEL") else {
        return;
    };
    assert!(std::env::var_os(PROXY_ATTRIBUTION_TOKEN_ENV_KEY).is_none());
    let inherited_sockets = std::fs::read_dir("/proc/self/fd")
        .expect("enumerate inherited descriptors")
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let target = std::fs::read_link(entry.path()).ok()?;
            target
                .to_string_lossy()
                .starts_with("socket:")
                .then(|| (entry.file_name(), target))
        })
        .collect::<Vec<_>>();
    assert_eq!(inherited_sockets, Vec::new(), "privileged socket leaked");

    let shared = std::env::var("CODEX_TEST_HANDOFF_SHARED").expect("shared directory");
    let shared = Path::new(&shared);
    let peer = std::env::var("CODEX_TEST_HANDOFF_PEER").expect("peer label");
    std::fs::write(shared.join(&label), b"ready").expect("announce sandbox readiness");
    let deadline = Instant::now() + OPERATION_TIMEOUT;
    while !shared.join(&peer).exists() {
        assert!(Instant::now() < deadline, "peer sandbox did not start");
        std::thread::sleep(Duration::from_millis(/*millis*/ 10));
    }

    let http =
        Url::parse(&std::env::var("HTTP_PROXY").expect("HTTP proxy")).expect("parse HTTP proxy");
    let https =
        Url::parse(&std::env::var("HTTPS_PROXY").expect("HTTPS proxy")).expect("parse HTTPS proxy");
    let other =
        Url::parse(&std::env::var("ALL_PROXY").expect("other proxy")).expect("parse other proxy");
    for (route, proxy) in [("http", http), ("https", https), ("all", other)] {
        let endpoint = (
            proxy.host_str().expect("proxy host"),
            proxy.port().expect("proxy port"),
        );
        let mut stream = TcpStream::connect(endpoint).expect("connect through handoff listener");
        stream
            .set_read_timeout(Some(OPERATION_TIMEOUT))
            .expect("read timeout");
        stream
            .set_write_timeout(Some(OPERATION_TIMEOUT))
            .expect("write timeout");
        let expected = format!("{label}/{route}");
        stream
            .write_all(expected.as_bytes())
            .expect("send route marker");
        stream.shutdown(Shutdown::Write).expect("finish request");
        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .expect("read routed reply");
        assert_eq!(response, expected);
    }
    println!("handoff routes verified for {label}");
}

/// Both filesystem policies must preserve route identity across concurrent launches.
#[tokio::test]
async fn handoff_isolates_concurrent_endpoints_and_closes_privileged_descriptors() {
    if let Some(reason) = managed_proxy_skip_reason().await {
        eprintln!("skipping managed proxy test: {reason}");
        return;
    }

    let shared = tempfile::tempdir_in("/tmp").expect("shared writable test directory");
    let test_executable = std::env::current_exe().expect("integration test executable");
    let test_executable = test_executable.to_str().expect("UTF-8 executable path");
    // Libtest selectors include the module path but omit the integration crate name.
    let (_, test_module) = module_path!()
        .split_once("::")
        .expect("handoff tests live in an integration test module");
    let client_test = format!("{test_module}::handoff_client");
    let mut commands = Vec::new();
    let mut servers = Vec::new();
    for (label, peer, profile) in [
        ("first", "second", PermissionProfile::workspace_write()),
        ("second", "first", PermissionProfile::Disabled),
    ] {
        let mut env = create_env_from_core_vars();
        strip_proxy_env(&mut env);
        env.insert(
            PROXY_ATTRIBUTION_TOKEN_ENV_KEY.to_string(),
            format!("handoff-{label}"),
        );
        env.insert("CODEX_TEST_HANDOFF_LABEL".to_string(), label.to_string());
        env.insert("CODEX_TEST_HANDOFF_PEER".to_string(), peer.to_string());
        env.insert(
            "CODEX_TEST_HANDOFF_SHARED".to_string(),
            shared.path().to_string_lossy().into_owned(),
        );

        for (routes, keys) in [
            (&["http", "https"][..], &["HTTP_PROXY", "HTTPS_PROXY"][..]),
            (&["all"][..], &["ALL_PROXY"][..]),
        ] {
            let listener =
                TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("local proxy listener");
            let endpoint = listener.local_addr().expect("proxy endpoint");
            for key in keys {
                env.insert((*key).to_string(), format!("http://{endpoint}"));
            }
            listener
                .set_nonblocking(/*nonblocking*/ true)
                .expect("bounded accept");
            servers.push(std::thread::spawn(move || {
                for route in routes {
                    let deadline = Instant::now() + OPERATION_TIMEOUT;
                    let (mut stream, _) = loop {
                        match listener.accept() {
                            Ok(connection) => break connection,
                            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                                assert!(Instant::now() < deadline, "proxy request timed out");
                                std::thread::sleep(Duration::from_millis(/*millis*/ 10));
                            }
                            Err(error) => panic!("accept proxy request: {error}"),
                        }
                    };
                    stream
                        .set_read_timeout(Some(OPERATION_TIMEOUT))
                        .expect("read timeout");
                    stream
                        .set_write_timeout(Some(OPERATION_TIMEOUT))
                        .expect("write timeout");
                    let marker = format!("{label}/{route}");
                    let mut expected = Vec::new();
                    write_attribution_frame(&mut expected, &format!("handoff-{label}"))
                        .expect("expected attribution frame");
                    expected.extend_from_slice(marker.as_bytes());
                    let mut request = Vec::new();
                    stream
                        .read_to_end(&mut request)
                        .expect("read attributed marker");
                    assert_eq!(request, expected);
                    stream
                        .write_all(marker.as_bytes())
                        .expect("reply with route marker");
                }
            }));
        }

        let mut command = linux_sandbox_command(
            &[
                test_executable,
                "--exact",
                &client_test,
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ],
            &profile,
            /*allow_network_for_proxy*/ true,
            env,
        );
        command.kill_on_drop(true);
        commands.push(command);
    }

    let mut second = commands.pop().expect("second sandbox command");
    let mut first = commands.pop().expect("first sandbox command");
    let outputs = tokio::time::timeout(SANDBOX_TIMEOUT, async {
        tokio::join!(first.output(), second.output())
    })
    .await
    .expect("concurrent sandboxes should finish and release output");
    for (label, output) in [("first", outputs.0), ("second", outputs.1)] {
        let output = output.expect("run sandbox");
        assert_eq!(
            output.status.success(),
            true,
            "{label} sandbox failed; stdout={}; stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout)
                .contains(&format!("handoff routes verified for {label}"))
        );
    }
    for server in servers {
        server.join().expect("proxy responder should succeed");
    }
}

#[tokio::test]
async fn cancelling_sandbox_closes_active_proxy_connection() {
    if let Some(reason) = managed_proxy_skip_reason().await {
        eprintln!("skipping managed proxy test: {reason}");
        return;
    }

    for profile in [
        PermissionProfile::workspace_write(),
        PermissionProfile::Disabled,
    ] {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("local proxy listener");
        listener
            .set_nonblocking(/*nonblocking*/ true)
            .expect("bounded accept");
        let mut env = create_env_from_core_vars();
        strip_proxy_env(&mut env);
        env.remove(PROXY_ATTRIBUTION_TOKEN_ENV_KEY);
        env.insert(
            "HTTP_PROXY".to_string(),
            format!("http://{}", listener.local_addr().expect("proxy endpoint")),
        );
        let mut command = linux_sandbox_command(
            &[
                "bash",
                "-c",
                concat!(
                    "set -eu; proxy=${HTTP_PROXY#*://}; ",
                    "exec 3<>/dev/tcp/${proxy%:*}/${proxy##*:}; ",
                    "printf ready >&3; IFS= read -r response <&3",
                ),
            ],
            &profile,
            /*allow_network_for_proxy*/ true,
            env,
        );
        command.kill_on_drop(true);
        let mut child = command.spawn().expect("launch sandbox");
        let (mut upstream, _) = tokio::time::timeout(OPERATION_TIMEOUT, async {
            loop {
                match listener.accept() {
                    Ok(connection) => break connection,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        tokio::time::sleep(Duration::from_millis(/*millis*/ 10)).await;
                    }
                    Err(error) => panic!("accept proxy connection: {error}"),
                }
            }
        })
        .await
        .expect("sandbox should establish its proxy connection");
        upstream
            .set_read_timeout(Some(OPERATION_TIMEOUT))
            .expect("bounded read");
        let mut ready = [0; 5];
        upstream
            .read_exact(&mut ready)
            .expect("read client readiness");
        assert_eq!(ready, *b"ready");

        child.start_kill().expect("cancel sandbox");
        let output = tokio::time::timeout(OPERATION_TIMEOUT, child.wait_with_output())
            .await
            .expect("cancelled sandbox should release output")
            .expect("wait for cancelled sandbox");
        assert_eq!(output.status.success(), false);
        assert_eq!(
            upstream
                .read(&mut [0])
                .expect("host bridge should close its active connection"),
            0,
            "cancelling the sandbox must stop its host bridge"
        );
    }
}
