//! Check TLS trust-service access in applied Seatbelt policies without external network dependencies.

#![cfg(target_os = "macos")]

use super::CreateSeatbeltCommandArgsParams;
use super::MACOS_PATH_TO_SEATBELT_EXECUTABLE;
use super::create_seatbelt_command_args;
use codex_network_proxy::ManagedNetworkSandboxContext;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::net::TcpListener;
use std::net::TcpStream;
use std::process::Command;
use std::time::Duration;

const EXPECTATION_ENV: &str = "CODEX_SEATBELT_TLS_EXPECTATION";
const PROXY_ADDRESS_ENV: &str = "CODEX_SEATBELT_TLS_PROXY_ADDRESS";
const OTHER_ADDRESS_ENV: &str = "CODEX_SEATBELT_TLS_OTHER_ADDRESS";
const SANDBOX_FILTER_GLOBAL_NAME: libc::c_int = 2;

// These symbols are exported by libSystem, which Rust already links on macOS.
unsafe extern "C" {
    static SANDBOX_CHECK_NO_REPORT: libc::c_int;
    fn sandbox_check(
        pid: libc::pid_t,
        operation: *const libc::c_char,
        filter: libc::c_int,
        ...
    ) -> libc::c_int;
}

#[test]
fn trust_evaluation_agent_access_follows_network_policy() {
    let workspace = tempfile::tempdir().expect("temporary workspace");
    let proxy = TcpListener::bind("127.0.0.1:0").expect("proxy listener");
    let other = TcpListener::bind("127.0.0.1:0").expect("unapproved listener");
    let proxy_address = proxy.local_addr().expect("proxy address");
    let other_address = other.local_addr().expect("unapproved address");
    let unix_sockets = [
        AbsolutePathBuf::from_absolute_path(workspace.path().join("service.sock"))
            .expect("absolute Unix socket path"),
    ];
    let managed_network = ManagedNetworkSandboxContext {
        loopback_ports: vec![proxy_address.port()],
        ..Default::default()
    };
    let missing_endpoint = ManagedNetworkSandboxContext::default();
    let unix_only = ManagedNetworkSandboxContext {
        allow_unix_sockets: vec![unix_sockets[0].to_string_lossy().into_owned()],
        ..Default::default()
    };
    let all_unix_sockets = ManagedNetworkSandboxContext {
        dangerously_allow_all_unix_sockets: true,
        ..Default::default()
    };
    let local_binding = ManagedNetworkSandboxContext {
        allow_local_binding: true,
        ..Default::default()
    };
    let module = module_path!().split_once("::").expect("test module path").1;
    for (name, network_policy, managed_network, extra_unix_sockets, expectation) in [
        (
            "disabled",
            NetworkSandboxPolicy::Restricted,
            None,
            &[][..],
            "deny",
        ),
        (
            "enabled",
            NetworkSandboxPolicy::Enabled,
            None,
            &[][..],
            "allow",
        ),
        (
            "managed-proxy",
            NetworkSandboxPolicy::Restricted,
            Some(&managed_network),
            &[][..],
            "proxy",
        ),
        (
            "managed-proxy-overrides-enabled",
            NetworkSandboxPolicy::Enabled,
            Some(&managed_network),
            &[][..],
            "proxy",
        ),
        (
            "missing-endpoint",
            NetworkSandboxPolicy::Restricted,
            Some(&missing_endpoint),
            &[][..],
            "deny",
        ),
        (
            "missing-endpoint-overrides-enabled",
            NetworkSandboxPolicy::Enabled,
            Some(&missing_endpoint),
            &[][..],
            "deny",
        ),
        (
            "unix-only",
            NetworkSandboxPolicy::Restricted,
            None,
            unix_sockets.as_slice(),
            "deny",
        ),
        (
            "managed-unix-only",
            NetworkSandboxPolicy::Restricted,
            Some(&unix_only),
            &[][..],
            "deny",
        ),
        (
            "all-unix-sockets",
            NetworkSandboxPolicy::Restricted,
            Some(&all_unix_sockets),
            &[][..],
            "deny",
        ),
        (
            "local-binding",
            NetworkSandboxPolicy::Restricted,
            Some(&local_binding),
            &[][..],
            "allow",
        ),
        (
            "enabled-with-unix-socket",
            NetworkSandboxPolicy::Enabled,
            None,
            unix_sockets.as_slice(),
            "allow",
        ),
    ] {
        let args = create_seatbelt_command_args(CreateSeatbeltCommandArgsParams {
            command: vec![
                std::env::current_exe()
                    .expect("test executable")
                    .to_string_lossy()
                    .into_owned(),
                "--exact".into(),
                format!("{module}::trust_evaluation_agent_child"),
                "--ignored".into(),
                "--nocapture".into(),
            ],
            file_system_sandbox_policy: &FileSystemSandboxPolicy::read_only(),
            network_sandbox_policy: network_policy,
            sandbox_policy_cwd: workspace.path(),
            enforce_managed_network: managed_network.is_some(),
            managed_network,
            extra_allow_unix_sockets: extra_unix_sockets,
        })
        .expect("generated Seatbelt arguments");
        let output = Command::new(MACOS_PATH_TO_SEATBELT_EXECUTABLE)
            .args(args)
            .current_dir(workspace.path())
            .env(EXPECTATION_ENV, expectation)
            .env(PROXY_ADDRESS_ENV, proxy_address.to_string())
            .env(OTHER_ADDRESS_ENV, other_address.to_string())
            .output()
            .expect("run Seatbelt trust-service probe");
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !output.status.success()
            && stderr.contains("sandbox-exec: sandbox_apply: Operation not permitted")
        {
            eprintln!("skipping trust-service regression: nested Seatbelt unavailable");
            return;
        }
        assert!(output.status.success(), "{name}: {output:?}");
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("trust-service probes completed"),
            "{name}: child must execute the probes: {output:?}"
        );
    }
}

#[test]
#[ignore]
fn trust_evaluation_agent_child() {
    let Ok(expectation) = std::env::var(EXPECTATION_ENV) else {
        return;
    };
    let expected = match expectation.as_str() {
        "allow" => ([true, false], [true, true]),
        "proxy" => ([true, false], [true, false]),
        "deny" => ([false, false], [false, false]),
        _ => panic!("invalid trust-service expectation: {expectation}"),
    };
    // Query the applied policy, independent of whether the OS currently runs this service.
    let access = [
        c"com.apple.TrustEvaluationAgent",
        c"com.apple.TrustEvaluationAgent.unrelated",
    ]
    .map(|service| {
        // SAFETY: GLOBAL_NAME takes one NUL-terminated service name; all pointers remain live.
        let result = unsafe {
            sandbox_check(
                libc::getpid(),
                c"mach-lookup".as_ptr(),
                SANDBOX_FILTER_GLOBAL_NAME | SANDBOX_CHECK_NO_REPORT,
                service.as_ptr(),
            )
        };
        assert!(
            result >= 0,
            "sandbox_check failed for {service:?}: {result}"
        );
        result == 0
    });
    let connections = [PROXY_ADDRESS_ENV, OTHER_ADDRESS_ENV].map(|key| {
        let address = std::env::var(key)
            .expect("listener address")
            .parse()
            .expect("valid socket address");
        match TcpStream::connect_timeout(&address, Duration::from_secs(1)) {
            Ok(_) => true,
            Err(error) => {
                assert!(
                    matches!(error.raw_os_error(), Some(libc::EPERM | libc::EACCES)),
                    "expected sandbox denial connecting to {address}: {error}"
                );
                false
            }
        }
    });
    assert_eq!((access, connections), expected);
    println!("trust-service probes completed");
}
