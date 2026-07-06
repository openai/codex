use std::collections::HashMap;

use codex_network_proxy::ManagedNetworkSandboxContext;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::NetworkSandboxPolicy;
use pretty_assertions::assert_eq;

use super::BrowserNetworkSandbox;
use super::browser_direct_spawn_runtime;
use super::browser_file_system_policy;
use super::browser_network_sandbox;
use super::ensure_isolated_binary_root;
#[cfg(target_os = "macos")]
use super::prepare_browser_launch;
use crate::network::BrowserNetworkPolicy;
use crate::sandbox::BrowserLaunchContext;
use codex_sandboxing::SandboxDirectSpawnRuntime;
use codex_sandboxing::SandboxRuntimeProxyArgument;
use codex_utils_absolute_path::AbsolutePathBuf;

#[test]
fn browser_policy_can_write_only_its_runtime_root() {
    let root = tempfile::tempdir().expect("test root");
    let root = AbsolutePathBuf::from_absolute_path(root.path()).expect("absolute test root");
    let browser_root = root.join("browser-runtime");
    let binary_root = root.join("carbonyl-bundle");
    let workspace_root = root.join("workspace");
    let policy = browser_file_system_policy(
        &browser_root,
        &browser_root.join("profile"),
        &binary_root,
        &HashMap::new(),
    )
    .expect("browser policy");

    assert_eq!(
        policy.resolve_access_with_cwd(browser_root.as_path(), browser_root.as_path()),
        FileSystemAccessMode::Write
    );
    assert_eq!(
        policy.resolve_access_with_cwd(binary_root.as_path(), browser_root.as_path()),
        FileSystemAccessMode::Read
    );
    assert_eq!(
        policy.resolve_access_with_cwd(
            browser_root.join("workspace").as_path(),
            browser_root.as_path()
        ),
        FileSystemAccessMode::Write
    );
    assert_eq!(
        policy.resolve_access_with_cwd(workspace_root.as_path(), browser_root.as_path()),
        FileSystemAccessMode::Deny
    );
    #[cfg(target_os = "macos")]
    {
        let readable_system_resources = [
            "/System/Library/CoreServices/SystemAppearance.bundle/Contents/Resources/SystemAppearance.car",
            "/System/Library/CoreServices/SystemVersion.bundle/Contents/Resources/English.lproj/SystemVersion.strings",
            "/System/Library/Fonts/LastResort.otf",
        ];
        let unrelated_core_service = AbsolutePathBuf::from_absolute_path(
            "/System/Library/CoreServices/Finder.app/Contents/MacOS/Finder",
        )
        .expect("absolute unrelated CoreServices path");

        for path in readable_system_resources {
            let path = AbsolutePathBuf::from_absolute_path(path).expect("absolute system path");
            assert_eq!(
                policy.resolve_access_with_cwd(path.as_path(), browser_root.as_path()),
                FileSystemAccessMode::Read
            );
        }
        assert_eq!(
            policy
                .resolve_access_with_cwd(unrelated_core_service.as_path(), browser_root.as_path()),
            FileSystemAccessMode::Deny
        );
    }
}

#[test]
fn managed_proxy_enforces_only_its_exact_loopback_port() {
    let http_addr = "127.0.0.1:8080".parse().expect("proxy address");

    assert_eq!(
        browser_network_sandbox(&BrowserNetworkPolicy::ManagedProxy { http_addr })
            .expect("managed proxy sandbox"),
        BrowserNetworkSandbox {
            policy: NetworkSandboxPolicy::Restricted,
            enforce_managed_network: true,
            managed_network: Some(ManagedNetworkSandboxContext {
                loopback_ports: vec![8_080],
                allow_local_binding: false,
            }),
        },
    );
}

#[test]
fn carbonyl_direct_spawn_runtime_rewrites_only_managed_proxy() {
    assert_eq!(
        browser_direct_spawn_runtime(&BrowserNetworkPolicy::Direct),
        SandboxDirectSpawnRuntime {
            proxy_argument: SandboxRuntimeProxyArgument::Unchanged,
        }
    );
    assert_eq!(
        browser_direct_spawn_runtime(&BrowserNetworkPolicy::ManagedProxy {
            http_addr: "127.0.0.1:43128".parse().expect("proxy address"),
        }),
        SandboxDirectSpawnRuntime {
            proxy_argument: SandboxRuntimeProxyArgument::RewriteFromHttpProxy {
                argument_prefix: "--proxy-server=".to_string(),
            },
        }
    );
}

#[test]
fn direct_network_preserves_the_unrestricted_network_policy() {
    assert_eq!(
        browser_network_sandbox(&BrowserNetworkPolicy::Direct).expect("direct network sandbox"),
        BrowserNetworkSandbox {
            policy: NetworkSandboxPolicy::Enabled,
            enforce_managed_network: false,
            managed_network: None,
        },
    );
}

#[test]
fn managed_proxy_rejects_non_loopback_addresses() {
    let http_addr = "192.0.2.10:8080".parse().expect("proxy address");

    let error = browser_network_sandbox(&BrowserNetworkPolicy::ManagedProxy { http_addr })
        .expect_err("non-loopback proxy must be rejected");

    assert_eq!(
        error.to_string(),
        "managed terminal-browser proxy must use a loopback address"
    );
}

#[test]
fn managed_proxy_rejects_port_zero() {
    let http_addr = "127.0.0.1:0".parse().expect("proxy address");

    let error = browser_network_sandbox(&BrowserNetworkPolicy::ManagedProxy { http_addr })
        .expect_err("port-zero proxy must be rejected");

    assert_eq!(
        error.to_string(),
        "managed terminal-browser proxy must use a nonzero port"
    );
}

#[cfg(target_os = "macos")]
#[test]
fn prepared_managed_proxy_seatbelt_policy_grants_only_the_proxy_port() {
    let root = tempfile::tempdir().expect("test root");
    let root = AbsolutePathBuf::from_absolute_path(root.path()).expect("absolute test root");
    let binary_root = root.join("carbonyl-bundle");
    std::fs::create_dir_all(binary_root.as_path()).expect("create Carbonyl bundle");
    let binary = binary_root.join("carbonyl");
    std::fs::write(binary.as_path(), "test").expect("create Carbonyl binary");
    let browser_root = root.join("runtime");
    let launch = prepare_browser_launch(
        binary.as_path(),
        Vec::new(),
        &browser_root,
        &browser_root.join("profile"),
        HashMap::new(),
        &BrowserNetworkPolicy::ManagedProxy {
            http_addr: "127.0.0.1:8080".parse().expect("proxy address"),
        },
        &BrowserLaunchContext::default(),
    )
    .expect("prepare managed Carbonyl launch");

    assert_eq!(launch.program, "/usr/bin/sandbox-exec");
    assert_eq!(launch.args.first().map(String::as_str), Some("-p"));
    let policy = launch.args.get(/*index*/ 1).expect("Seatbelt policy");
    assert!(policy.contains("(allow network-outbound (remote ip \"localhost:8080\"))"));
    assert!(!policy.contains("(allow network-bind (local ip \"*:*\"))"));
    assert!(!policy.contains("(allow network-outbound)\n"));
}

#[test]
fn binary_bundle_must_not_contain_the_workspace() {
    let root = tempfile::tempdir().expect("test root");
    let root = AbsolutePathBuf::from_absolute_path(root.path()).expect("absolute test root");
    let bundle = root.join("bundle");
    let workspace = bundle.join("workspace");
    let context = BrowserLaunchContext {
        workspace_root: Some(workspace),
        ..Default::default()
    };

    let error = ensure_isolated_binary_root(
        &bundle,
        &root.join("runtime"),
        &root.join("runtime/profile"),
        &context,
    )
    .expect_err("overlapping bundle must be rejected");

    assert!(error.to_string().contains("dedicated bundle"));
}
