#![cfg(test)]

//! Tests session proxy policy across permission changes and MCP launches.

use super::*;
use pretty_assertions::assert_eq;
use test_case::test_case;

#[test_case("1.1.1.1", "8.8.8.8", Decision::Allow; "stale grant")]
#[test_case("1.1.1.1.", "8.8.8.8", Decision::Allow; "equivalent managed spelling")]
#[test_case("1.1.1.1", "bad{host}", Decision::Forbidden; "malformed denial")]
#[tokio::test]
async fn stale_saved_grant_keeps_valid_denial_on_native_proxy_and_mcp(
    configured: &str,
    host: &str,
    decision: Decision,
) -> anyhow::Result<()> {
    let permission_profile = PermissionProfile::workspace_write();
    let spec = crate::config::NetworkProxySpec::from_config_and_constraints(
        NetworkProxyConfig::default(),
        Some(NetworkConstraints {
            enabled: Some(true),
            domains: Some(NetworkDomainPermissionsToml {
                entries: std::collections::BTreeMap::from([(
                    configured.to_string(),
                    NetworkDomainPermissionToml::Allow,
                )]),
            }),
            managed_allowed_domains_only: Some(true),
            ..Default::default()
        }),
        &permission_profile,
    )?;
    let mut exec_policy = Policy::empty();
    exec_policy.add_network_rule(
        host,
        NetworkRuleProtocol::Https,
        decision,
        /*justification*/ None,
    )?;
    exec_policy.add_network_rule(
        "1.1.1.1",
        NetworkRuleProtocol::Https,
        Decision::Forbidden,
        /*justification*/ None,
    )?;

    let (started_proxy, _) = Session::start_managed_network_proxy(
        &spec,
        &exec_policy,
        &permission_profile,
        SandboxType::None,
        /*network_policy_decider*/ None,
        /*blocked_request_observer*/ None,
        /*managed_network_requirements_enabled*/ false,
        crate::config::NetworkProxyAuditMetadata::default(),
    )
    .await?;

    let native = started_proxy.proxy().current_cfg().await?;
    let mcp = crate::config::NetworkProxySpec::mcp_network_policy(
        Some(&spec),
        &permission_profile,
        crate::config::McpNetworkRoute {
            primary_allows_proxy: true,
            local_binding: codex_network_proxy::LocalBindingPolicy::DefaultFalse,
            owner: None,
        },
        &permission_profile,
        AskForApproval::OnRequest,
        &exec_policy,
    )?;
    let native_domains = (
        native.allowed_domains().unwrap_or_default(),
        native.denied_domains().unwrap_or_default(),
    );
    assert_eq!(native_domains.1, ["1.1.1.1"]);
    assert!(native_domains.0.iter().all(|host| host == configured));
    assert!(mcp.enabled && mcp.hard_deny_allowlist_misses);
    assert_eq!((mcp.allowed_domains, mcp.denied_domains), native_domains);
    Ok(())
}

#[test_case(true; "shared proxy")]
#[test_case(false; "owner proxy without controller")]
#[tokio::test]
async fn failed_network_rule_write_keeps_native_denial_in_mcp_policy(
    shared_proxy: bool,
) -> anyhow::Result<()> {
    const DENIED: &str = "denied.example";
    const GRANTED: &str = "granted.example";
    let home = tempfile::tempdir()?;
    let profile = PermissionProfile::workspace_write();
    let mut network = NetworkProxyConfig {
        enabled: true,
        allow_local_binding: Some(true),
        ..Default::default()
    };
    network.set_allowed_domains(vec![DENIED.to_owned()]);
    let spec = crate::config::NetworkProxySpec::from_config_and_constraints(
        network, /*requirements*/ None, &profile,
    )?;
    let controller = shared_proxy.then_some(&spec);
    let owner = (!shared_proxy).then(|| spec.environment_policy());
    let binding = codex_network_proxy::LocalBindingPolicy::DefaultFalse;
    let session = make_session_with_config(|config| {
        config.codex_home = home.path().abs();
        config
            .permissions
            .set_permission_profile(profile.clone())
            .expect("set profile");
        config.permissions.network = controller.cloned();
    })
    .await?;
    let policy_file = crate::exec_policy::default_policy_path(home.path());
    std::fs::create_dir_all(&policy_file)?;
    for (host, action) in [
        (GRANTED, NetworkPolicyRuleAction::Allow),
        (DENIED, NetworkPolicyRuleAction::Deny),
    ] {
        let mut at_apply = None;
        let error = session
            .persist_network_policy_amendment(
                &NetworkPolicyAmendment {
                    host: host.to_owned(),
                    action,
                },
                &NetworkApprovalContext {
                    host: host.to_owned(),
                    protocol: NetworkApprovalProtocol::Https,
                },
                || at_apply = Some(session.services.exec_policy.current()),
            )
            .await
            .expect_err("a directory cannot be opened for appending a rule");
        assert!(
            matches!(error.downcast_ref::<ExecPolicyUpdateError>(),
                Some(ExecPolicyUpdateError::AppendRule { path, .. }) if path == &policy_file
            ),
            "{error}"
        );
        // A shared proxy notifies at installation. With no shared proxy a failed save never notifies.
        let denied = match action {
            NetworkPolicyRuleAction::Allow => vec![],
            NetworkPolicyRuleAction::Deny => vec![host.to_owned()],
        };
        assert_eq!(
            at_apply.map(|policy| policy.compiled_network_domains()),
            shared_proxy.then_some((Vec::new(), denied))
        );
    }
    let policy = session.services.exec_policy.current();
    let native_domains = if let Some(proxy) = session.services.network_proxy.load_full() {
        proxy.proxy().current_cfg().await?.domains
    } else {
        crate::config::NetworkProxySpec::for_environment(
            controller,
            owner.as_ref().expect("owner policy"),
            &profile,
            &policy,
            binding,
        )?
        .environment_policy()
        .domains
    };
    let mcp = crate::config::NetworkProxySpec::mcp_network_policy(
        controller,
        &profile,
        crate::config::McpNetworkRoute {
            primary_allows_proxy: shared_proxy,
            local_binding: binding,
            owner: owner.as_ref(),
        },
        &profile,
        AskForApproval::OnRequest,
        &policy,
    )?;
    let mut expected = json!({ (DENIED): "deny" });
    if shared_proxy {
        expected[GRANTED] = json!("allow");
    }
    assert_eq!(serde_json::to_value(native_domains)?, expected);
    assert_eq!(
        (mcp.allowed_domains, mcp.denied_domains),
        (vec![], vec![DENIED.to_owned()])
    );
    Ok(())
}

#[test_case(true, false, 1; "approvals enabled")]
#[test_case(false, false, 0; "approvals disabled")]
#[test_case(true, true, 0; "managed allowlist is final")]
#[tokio::test]
async fn managed_network_proxy_respects_approval_policy_after_profile_change(
    approvals_enabled: bool,
    managed_allowed_domains_only: bool,
    expected_decider_calls: usize,
) -> anyhow::Result<()> {
    let full_access_permission_profile = PermissionProfile::Disabled;
    let spec = crate::config::NetworkProxySpec::from_config_and_constraints(
        NetworkProxyConfig::default(),
        Some(NetworkConstraints {
            enabled: Some(true),
            managed_allowed_domains_only: Some(managed_allowed_domains_only),
            ..Default::default()
        }),
        &full_access_permission_profile,
    )?;
    let exec_policy = Policy::empty();
    let decider_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let network_policy_decider: Arc<dyn codex_network_proxy::NetworkPolicyDecider> = Arc::new({
        let decider_calls = Arc::clone(&decider_calls);
        move |_request| {
            decider_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            async { codex_network_proxy::NetworkDecision::ask("not_allowed") }
        }
    });

    let (blocked_tx, mut blocked_rx) = tokio::sync::mpsc::unbounded_channel();
    let observer = move |request: codex_network_proxy::BlockedRequest| {
        blocked_tx
            .send(request.host)
            .expect("blocked request receiver");
        std::future::ready(())
    };
    let (started_proxy, _) = Session::start_managed_network_proxy(
        &spec,
        &exec_policy,
        &full_access_permission_profile,
        SandboxType::None,
        Some(network_policy_decider),
        Some(Arc::new(observer)),
        approvals_enabled,
        crate::config::NetworkProxyAuditMetadata::default(),
    )
    .await?;

    let spec = spec.recompute_for_permission_profile(&PermissionProfile::workspace_write())?;
    spec.apply_to_started_proxy(&started_proxy).await?;
    let current_cfg = started_proxy.proxy().current_cfg().await?;
    assert_eq!(current_cfg.allowed_domains(), None);

    use tokio::io::AsyncReadExt as _;
    use tokio::io::AsyncWriteExt as _;

    let prepared = started_proxy
        .proxy()
        .prepare_for_remote_environment(std::collections::HashMap::new(), "test-bridge")?;
    let proxy_addr = prepared.env["HTTP_PROXY"]
        .strip_prefix("http://")
        .expect("HTTP proxy URL")
        .parse::<std::net::SocketAddr>()?;
    let mut stream = tokio::net::TcpStream::connect(proxy_addr).await?;
    stream
        .write_all(
            b"GET http://example.com/ HTTP/1.1\r\nHost: example.com\r\nConnection: close\r\n\r\n",
        )
        .await?;
    let mut buffer = [0_u8; 4096];
    let bytes_read = tokio::time::timeout(StdDuration::from_secs(2), stream.read(&mut buffer))
        .await
        .expect("timed out waiting for proxy response")?;
    let response = String::from_utf8_lossy(&buffer[..bytes_read]);

    assert!(
        response.starts_with("HTTP/1.1 403 Forbidden"),
        "unexpected proxy response: {response}"
    );
    assert!(
        response.contains("x-proxy-error: blocked-by-allowlist"),
        "unexpected proxy response: {response}"
    );
    assert_eq!(blocked_rx.try_recv()?, "example.com");
    assert_eq!(
        decider_calls.load(std::sync::atomic::Ordering::SeqCst),
        expected_decider_calls,
        "unexpected proxy response: {response}"
    );
    Ok(())
}
