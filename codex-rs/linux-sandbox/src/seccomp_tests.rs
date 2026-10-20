//! Verify network policies select the expected seccomp enforcement mode.

use super::NetworkSeccompMode;
use super::network_seccomp_mode;
use super::should_install_network_seccomp;
use codex_protocol::protocol::NetworkSandboxPolicy;
use pretty_assertions::assert_eq;

#[test]
fn managed_network_enforces_seccomp_even_for_full_network_policy() {
    assert_eq!(
        should_install_network_seccomp(
            NetworkSandboxPolicy::Enabled,
            /*allow_network_for_proxy*/ true,
        ),
        true
    );
}

#[test]
fn full_network_policy_without_managed_network_skips_seccomp() {
    assert_eq!(
        should_install_network_seccomp(
            NetworkSandboxPolicy::Enabled,
            /*allow_network_for_proxy*/ false,
        ),
        false
    );
}

#[test]
fn restricted_network_policy_always_installs_seccomp() {
    assert!(should_install_network_seccomp(
        NetworkSandboxPolicy::Restricted,
        /*allow_network_for_proxy*/ false,
    ));
    assert!(should_install_network_seccomp(
        NetworkSandboxPolicy::Restricted,
        /*allow_network_for_proxy*/ true,
    ));
}

#[test]
fn managed_proxy_routes_use_proxy_routed_seccomp_mode() {
    assert_eq!(
        network_seccomp_mode(
            NetworkSandboxPolicy::Enabled,
            /*allow_network_for_proxy*/ true,
            /*proxy_routed_network*/ true,
        ),
        Some(NetworkSeccompMode::ProxyRouted)
    );
}

#[test]
fn restricted_network_without_proxy_routing_uses_restricted_mode() {
    assert_eq!(
        network_seccomp_mode(
            NetworkSandboxPolicy::Restricted,
            /*allow_network_for_proxy*/ false,
            /*proxy_routed_network*/ false,
        ),
        Some(NetworkSeccompMode::Restricted)
    );
}

#[test]
fn full_network_without_managed_proxy_skips_network_seccomp_mode() {
    assert_eq!(
        network_seccomp_mode(
            NetworkSandboxPolicy::Enabled,
            /*allow_network_for_proxy*/ false,
            /*proxy_routed_network*/ false,
        ),
        None
    );
}
