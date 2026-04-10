use super::ProxyPolicyInputs;
use super::UnixDomainSocketPolicy;
use super::proxy_policy_inputs;
use super::unix_socket_path_param_key;
use super::unix_socket_path_params;
use codex_network_proxy::NetworkProxy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use std::path::PathBuf;

const MACOS_SEATBELT_NETWORK_ONLY_BASE_POLICY: &str = "(version 1)\n(allow default)";

pub(crate) fn create_network_only_seatbelt_command_args(
    command: Vec<String>,
    network_sandbox_policy: NetworkSandboxPolicy,
    enforce_managed_network: bool,
    network: Option<&NetworkProxy>,
) -> Vec<String> {
    let proxy = proxy_policy_inputs(network);
    let network_policy =
        network_only_policy_for_network(network_sandbox_policy, enforce_managed_network, &proxy);
    let full_policy = [MACOS_SEATBELT_NETWORK_ONLY_BASE_POLICY, &network_policy].join("\n");

    let mut seatbelt_args: Vec<String> = vec!["-p".to_string(), full_policy];
    seatbelt_args.extend(
        unix_socket_dir_params(&proxy)
            .into_iter()
            .map(|(key, value): (String, PathBuf)| format!("-D{key}={}", value.to_string_lossy())),
    );
    seatbelt_args.push("--".to_string());
    seatbelt_args.extend(command);
    seatbelt_args
}

pub(super) fn network_only_policy_for_network(
    network_policy: NetworkSandboxPolicy,
    enforce_managed_network: bool,
    proxy: &ProxyPolicyInputs,
) -> String {
    if network_policy.is_enabled()
        && !enforce_managed_network
        && proxy.ports.is_empty()
        && !proxy.has_proxy_config
    {
        return String::new();
    }

    let mut policy = String::new();
    push_ip_network_policy(&mut policy, proxy);
    push_unix_socket_network_policy(&mut policy, proxy);
    policy
}

fn push_ip_network_policy(policy: &mut String, proxy: &ProxyPolicyInputs) {
    let mut outbound_exceptions = Vec::new();
    if proxy.allow_local_binding {
        outbound_exceptions.push(r#"(remote ip "localhost:*")"#.to_string());
    }
    outbound_exceptions.extend(
        proxy
            .ports
            .iter()
            .map(|port| format!(r#"(remote ip "localhost:{port}")"#)),
    );
    push_deny_rule(
        policy,
        "network-outbound",
        r#"(remote ip "*:*")"#,
        &outbound_exceptions,
    );

    let local_exceptions = if proxy.allow_local_binding {
        vec![r#"(local ip "localhost:*")"#.to_string()]
    } else {
        Vec::new()
    };
    push_deny_rule(
        policy,
        "network-bind",
        r#"(local ip "*:*")"#,
        &local_exceptions,
    );
    push_deny_rule(
        policy,
        "network-inbound",
        r#"(local ip "*:*")"#,
        &local_exceptions,
    );
}

fn push_unix_socket_network_policy(policy: &mut String, proxy: &ProxyPolicyInputs) {
    let UnixDomainSocketPolicy::Restricted { .. } = proxy.unix_domain_socket_policy else {
        return;
    };
    let exceptions = unix_socket_path_params(proxy)
        .into_iter()
        .map(|param| {
            let key = unix_socket_path_param_key(param.index);
            format!(r#"(remote unix-socket (subpath (param "{key}")))"#)
        })
        .collect::<Vec<_>>();
    push_deny_rule(
        policy,
        "network-outbound",
        "(remote unix-socket)",
        &exceptions,
    );

    let exceptions = unix_socket_path_params(proxy)
        .into_iter()
        .map(|param| {
            let key = unix_socket_path_param_key(param.index);
            format!(r#"(local unix-socket (subpath (param "{key}")))"#)
        })
        .collect::<Vec<_>>();
    push_deny_rule(policy, "network-bind", "(local unix-socket)", &exceptions);
}

fn push_deny_rule(policy: &mut String, operation: &str, selector: &str, exceptions: &[String]) {
    if exceptions.is_empty() {
        policy.push_str(&format!("(deny {operation} {selector})\n"));
        return;
    }

    policy.push_str(&format!("(deny {operation} (require-all {selector}"));
    for exception in exceptions {
        policy.push_str(&format!(" (require-not {exception})"));
    }
    policy.push_str("))\n");
}

fn unix_socket_dir_params(proxy: &ProxyPolicyInputs) -> Vec<(String, PathBuf)> {
    unix_socket_path_params(proxy)
        .into_iter()
        .map(|param| {
            (
                unix_socket_path_param_key(param.index),
                param.path.into_path_buf(),
            )
        })
        .collect()
}
