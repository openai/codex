//! Explicit Unix-socket grants preserve managed-proxy network isolation.

use super::*;
use codex_network_proxy::ConfigReloader;
use codex_network_proxy::ConfigReloaderFuture;
use codex_network_proxy::ConfigState;
use codex_network_proxy::NetworkProxy;
use codex_network_proxy::NetworkProxyConfig;
use codex_network_proxy::NetworkProxyConstraints;
use codex_network_proxy::NetworkProxyState;
use codex_network_proxy::build_config_state;
use pretty_assertions::assert_eq;
use std::os::unix::net::UnixListener;
use std::os::unix::net::UnixStream;
use std::sync::Arc;

struct TestConfigReloader(ConfigState);

impl ConfigReloader for TestConfigReloader {
    fn source_label(&self) -> String {
        "managed proxy Unix socket test".to_string()
    }

    fn maybe_reload(&self) -> ConfigReloaderFuture<'_, Option<ConfigState>> {
        Box::pin(async { Ok(None) })
    }

    fn reload_now(&self) -> ConfigReloaderFuture<'_, ConfigState> {
        Box::pin(async { Ok(self.0.clone()) })
    }
}

#[tokio::test]
async fn managed_proxy_allow_all_unix_sockets_preserves_network_isolation() {
    if let Some(reason) = managed_proxy_skip_reason().await {
        eprintln!("skipping managed proxy Unix socket test: {reason}");
        return;
    }
    if !Command::new("python3")
        .arg("--version")
        .output()
        .await
        .is_ok_and(|output| output.status.success())
    {
        eprintln!("skipping managed proxy Unix socket test: python3 is unavailable");
        return;
    }

    let temp = tempfile::tempdir().expect("socket directory");
    let socket_path = temp.path().join("daemon.sock");
    let listener = UnixListener::bind(&socket_path).expect("host Unix listener");
    let mut control = UnixStream::connect(&socket_path).expect("host Unix control connection");
    let (mut control_peer, _) = listener.accept().expect("accept host Unix control");
    control.write_all(b"ok").expect("write host control");
    let mut control_payload = [0; 2];
    control_peer
        .read_exact(&mut control_payload)
        .expect("read host control");
    assert_eq!(control_payload, *b"ok");

    // Use another loopback address so the namespace's proxy listener cannot
    // collide with this host-only endpoint even if it selects the same port.
    let tcp_listener =
        TcpListener::bind((Ipv4Addr::new(127, 0, 0, 2), 0)).expect("host TCP listener");
    let tcp_addr = tcp_listener.local_addr().expect("host TCP address");
    let _tcp_control = TcpStream::connect(tcp_addr).expect("host TCP control connection");
    let _tcp_peer = tcp_listener.accept().expect("accept host TCP control");

    let proxy_listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("host proxy listener");
    let proxy_addr = proxy_listener.local_addr().expect("host proxy address");

    listener
        .set_nonblocking(true)
        .expect("nonblocking listener");
    let server = std::thread::spawn(move || {
        let deadline = Instant::now() + OPERATION_TIMEOUT * 2;
        // The two allow-all cases below exercise this socket with and without a TCP proxy route.
        let mut requests = 0;
        while requests < 2 {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream
                        .set_read_timeout(Some(OPERATION_TIMEOUT))
                        .expect("set Unix read timeout");
                    let mut request = [0; 4];
                    stream.read_exact(&mut request).expect("read Unix request");
                    assert_eq!(request, *b"ping");
                    stream.write_all(b"pong").expect("write Unix response");
                    requests += 1;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "Unix request timed out");
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("accept Unix request: {error}"),
            }
        }
    });

    let probe = r#"
import errno, os, socket, sys
path, port, proxy_port, mode, attribution_key = sys.argv[1:]
assert attribution_key not in os.environ
try:
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as client:
        # With no network grants, seccomp can deny connect rather than socket creation.
        assert mode != 'deny', 'Unix socket unexpectedly allowed'
        client.settimeout(2)
        client.connect(path)
        assert mode == 'allow', 'Unix connection unexpectedly allowed'
        client.sendall(b'ping')
        assert client.recv(4, socket.MSG_WAITALL) == b'pong'
except OSError as error:
    assert mode != 'allow' and error.errno == errno.EPERM, error
unreachable = [('127.0.0.2', int(port)), ('192.0.2.1', 80)]
if mode == 'local-bind':
    # Local binding grants no route to the prepared HTTP proxy on the host.
    assert os.environ['HTTP_PROXY'] == f'http://127.0.0.1:{proxy_port}'
    unreachable.append(('127.0.0.1', int(proxy_port)))
    # The host already owns this address. The private listener must handle its own client
    # while the proxy URL that names the host listener stays unbridged.
    address = ('127.0.0.2', int(port))
    assert os.environ['ALL_PROXY'] == f'http://127.0.0.2:{port}'
    with socket.socket() as listener:
        listener.bind(address)
        listener.listen(1)
        with socket.create_connection(address, timeout=2) as client:
            with listener.accept()[0] as peer:
                client.sendall(b'local')
                assert peer.recv(5, socket.MSG_WAITALL) == b'local'
for address in unreachable:
    try:
        connection = socket.create_connection(address, timeout=0.5)
    except OSError as error:
        if mode == 'deny-all':
            assert error.errno == errno.EPERM, (address, error)
    else:
        connection.close()
        raise AssertionError(('direct network unexpectedly allowed', address))
for family in (socket.AF_NETLINK, getattr(socket, 'AF_VSOCK', 40)):
    try:
        unexpected = socket.socket(family, socket.SOCK_STREAM)
    except OSError as error:
        assert error.errno == errno.EPERM, (family, error)
    else:
        unexpected.close()
        raise AssertionError(('socket family unexpectedly allowed', family))
print('socket policy verified')
"#;
    let cwd = std::env::current_dir().expect("current directory");
    for (label, dangerously_allow_all_unix_sockets, allow_path, no_tcp_ports, local_binding) in [
        ("default denial", false, false, false, false),
        ("path grant stays restricted", false, true, false, false),
        ("explicit allow-all", true, false, false, false),
        (
            "explicit allow-all without a TCP proxy",
            true,
            false,
            true,
            false,
        ),
        ("empty managed network policy", false, false, true, false),
        (
            "private loopback without a host route",
            false,
            false,
            true,
            true,
        ),
    ] {
        let mut env = create_env_from_core_vars();
        strip_proxy_env(&mut env);
        let mut config = NetworkProxyConfig {
            enabled: true,
            proxy_url: format!("http://{proxy_addr}"),
            allow_local_binding: Some(local_binding),
            dangerously_allow_all_unix_sockets: Some(dangerously_allow_all_unix_sockets),
            ..Default::default()
        };
        if allow_path {
            config.set_allow_unix_sockets(vec![socket_path.to_string_lossy().into_owned()]);
        }
        let state = build_config_state(
            config,
            NetworkProxyConstraints::default(),
            codex_network_proxy::Platform::native(),
        )
        .expect("valid managed network configuration");
        let network = NetworkProxy::builder()
            .state(Arc::new(NetworkProxyState::with_reloader(
                state.clone(),
                Arc::new(TestConfigReloader(state)),
            )))
            .managed_by_codex(/*managed_by_codex*/ false)
            .build()
            .await
            .expect("build managed network proxy");
        let mut prepared = network
            .prepare_for_optional_environment(env, /*environment_id*/ None)
            .expect("prepare managed network policy");
        if no_tcp_ports {
            prepared.sandbox_context.portable.loopback_ports.clear();
        }
        let mut command = Command::new(env!("CARGO_BIN_EXE_codex-linux-sandbox"));
        command
            .arg("--sandbox-policy-cwd")
            .arg(&cwd)
            .arg("--permission-profile")
            .arg(serde_json::to_string(&PermissionProfile::Disabled).unwrap())
            .arg("--managed-network")
            .arg(serde_json::to_string(&prepared.sandbox_context.portable).unwrap())
            .args(["--", "python3", "-c", probe])
            .arg(&socket_path)
            .arg(tcp_addr.port().to_string())
            .arg(proxy_addr.port().to_string())
            .arg(if dangerously_allow_all_unix_sockets {
                "allow"
            } else if local_binding {
                "local-bind"
            } else if no_tcp_ports {
                "deny-all"
            } else {
                "deny"
            })
            .arg(PROXY_ATTRIBUTION_TOKEN_ENV_KEY)
            .env_clear()
            .envs(prepared.env)
            .env(PROXY_ATTRIBUTION_TOKEN_ENV_KEY, "unix-socket-test")
            .kill_on_drop(true);
        if local_binding {
            command.env("ALL_PROXY", format!("http://{tcp_addr}"));
        }
        let output = tokio::time::timeout(OPERATION_TIMEOUT, command.output())
            .await
            .expect("Unix socket probe timed out")
            .expect("Unix socket probe should execute");
        assert!(
            output.status.success(),
            "{label}; stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"socket policy verified\n", "{label}");
    }
    server.join().expect("Unix server should finish");
}
