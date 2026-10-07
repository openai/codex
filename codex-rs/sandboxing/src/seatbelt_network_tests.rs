//! Exercise managed network restrictions in an actual Seatbelt child.

#![cfg(target_os = "macos")]

use super::CreateSeatbeltCommandArgsParams;
use super::MACOS_PATH_TO_SEATBELT_EXECUTABLE;
use super::create_seatbelt_command_args;
use codex_network_proxy::ManagedNetworkSandboxContext;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use std::net::TcpListener;
use std::net::TcpStream;
use std::net::UdpSocket;
use std::process::Command;
use std::time::Duration;

#[test]
fn managed_local_binding_does_not_allow_external_dns() {
    let workspace = tempfile::tempdir().expect("temporary workspace");
    let proxy = TcpListener::bind("127.0.0.1:0").expect("proxy listener");
    let proxy_address = proxy.local_addr().expect("proxy address");
    let managed_network = ManagedNetworkSandboxContext {
        loopback_ports: vec![proxy_address.port()],
        allow_local_binding: true,
        ..Default::default()
    };
    let module = module_path!().split_once("::").expect("test module path").1;
    let args = create_seatbelt_command_args(CreateSeatbeltCommandArgsParams {
        command: vec![
            std::env::current_exe()
                .expect("test executable")
                .to_string_lossy()
                .into_owned(),
            "--exact".into(),
            format!("{module}::external_dns_child"),
            "--ignored".into(),
            "--nocapture".into(),
        ],
        file_system_sandbox_policy: &FileSystemSandboxPolicy::read_only(),
        network_sandbox_policy: NetworkSandboxPolicy::Enabled,
        sandbox_policy_cwd: workspace.path(),
        enforce_managed_network: true,
        managed_network: Some(&managed_network),
        environment_id: None,
        network: None,
        extra_allow_unix_sockets: &[],
    })
    .expect("generated Seatbelt arguments");
    let output = Command::new(MACOS_PATH_TO_SEATBELT_EXECUTABLE)
        .args(args)
        .env("CODEX_TEST_DNS_PROXY_ADDRESS", proxy_address.to_string())
        .output()
        .expect("run managed DNS probe");
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success()
        && stderr.contains("sandbox-exec: sandbox_apply: Operation not permitted")
    {
        eprintln!("skipping DNS regression: nested Seatbelt unavailable");
        return;
    }
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("DNS probes completed"));
}

#[test]
#[ignore = "executed by the parent test inside Seatbelt"]
fn external_dns_child() {
    let Ok(proxy_address) = std::env::var("CODEX_TEST_DNS_PROXY_ADDRESS") else {
        return;
    };
    TcpStream::connect(proxy_address).expect("managed proxy remains reachable");
    let socket = UdpSocket::bind("0.0.0.0:0").expect("local binding remains allowed");
    // TEST-NET needs no live DNS service. UDP connect checks the destination
    // permission without sending a DNS query or depending on a server reply.
    let udp = socket.connect("192.0.2.1:53");
    let tcp =
        TcpStream::connect_timeout(&"192.0.2.1:53".parse().unwrap(), Duration::from_millis(100));
    for error in [udp.err(), tcp.err()] {
        assert!(
            error.as_ref().is_some_and(|error| matches!(
                error.raw_os_error(),
                Some(libc::EPERM | libc::EACCES)
            )),
            "external DNS must be denied by Seatbelt, got {error:?}",
        );
    }
    println!("DNS probes completed");
}
