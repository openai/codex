#![cfg(test)]

//! Exercises the policy carried by MCP independently of tool-call publication.

use super::*;
use crate::config::ConfigBuilder;
use anyhow::Context;
use codex_execpolicy::Decision;
use codex_execpolicy::NetworkRuleProtocol::Https;
use codex_http_client::HttpClientBuilder;
use codex_network_proxy::NetworkMode;
use codex_protocol::models::PermissionProfile as Profile;
use codex_protocol::models::PermissionProfile::Disabled;
use codex_protocol::models::PermissionProfile::External;
use codex_protocol::permissions::NetworkSandboxPolicy::Enabled;
use codex_protocol::permissions::NetworkSandboxPolicy::Restricted;
use codex_protocol::protocol::AskForApproval::Never;
use codex_protocol::protocol::AskForApproval::OnRequest;
use http::StatusCode;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::time::Duration;
use test_case::test_case;

fn saved_rules(rules: &[(&str, Decision)]) -> anyhow::Result<Policy> {
    let mut policy = Policy::empty();
    for &(host, decision) in rules {
        policy.add_network_rule(host, Https, decision, /*justification*/ None)?;
    }
    Ok(policy)
}

fn primary_controller_policy(
    controller: &NetworkProxySpec,
    profile: &Profile,
    rules: &Policy,
) -> std::io::Result<NetworkPolicyMetadata> {
    NetworkProxySpec::mcp_network_policy(
        Some(controller),
        profile,
        McpNetworkRoute {
            primary_allows_proxy: !matches!(profile, Disabled),
            local_binding: LocalBindingPolicy::DefaultFalse,
            owner: None,
        },
        profile,
        OnRequest,
        rules,
    )
}

#[test_case(Profile::read_only(), false, None, false; "restricted offline")]
#[test_case(Profile::read_only(), true, None, true; "controller proxy")]
#[test_case(Profile::read_only(), false, Some(true), true; "managed proxy")]
#[test_case(Profile::read_only(), true, Some(false), false; "managed disable")]
#[test_case(External { network: Enabled }, false, None, true; "external network")]
#[test_case(Disabled, false, None, true; "disabled sandbox")]
#[test_case(Disabled, true, Some(false), false; "managed veto in disabled sandbox")]
fn controller_policy_keeps_managed_restrictions_after_stale_grants(
    profile: Profile,
    proxy_enabled: bool,
    managed_enabled: Option<bool>,
    expected_enabled: bool,
) -> anyhow::Result<()> {
    let requirements = serde_json::from_value(json!({
        "enabled": managed_enabled, "managed_allowed_domains_only": true, "allow_local_binding": true,
        "review_domains": ["allowed2.example.com"],
        "domains": {"allowed?.example.com": "allow", "blocked.example.com": "deny"},
    }))?;
    let config = NetworkProxyConfig {
        enabled: proxy_enabled,
        mode: NetworkMode::Limited,
        ..Default::default()
    };
    let controller =
        NetworkProxySpec::from_config_and_constraints(config, Some(requirements), &profile)?;
    let saved = saved_rules(&[
        ("previously-allowed.example.com", Decision::Allow),
        ("allowed1.example.com", Decision::Forbidden),
    ])?;
    let result = primary_controller_policy(&controller, &profile, &saved)?;
    let routed = managed_enabled.unwrap_or(proxy_enabled) && !matches!(profile, Disabled);
    let mut denied = vec!["blocked.example.com"];
    if routed && matches!(profile, Profile::Managed { .. }) {
        denied.push("allowed1.example.com");
    }
    denied.push("allowed2.example.com");
    assert_eq!(
        serde_json::to_value(result)?,
        json!({
            "enabled": expected_enabled, "mode": if routed { "limited" } else { "full" }, "allowedDomains": ["allowed?.example.com"],
            "deniedDomains": denied, "hardDenyAllowlistMisses": true,
        })
    );
    Ok(())
}

const MANAGED_DOMAINS: &str =
    "enabled = true\ndomains = { '*.example' = 'allow', 'admin.test' = 'deny' }";

#[test_case(None, false, Profile::read_only(), Some(MANAGED_DOMAINS), OnRequest, false, false; "no proxy restricted")]
#[test_case(None, false, Disabled, Some(MANAGED_DOMAINS), Never, true, false; "no proxy unrestricted")]
#[test_case(Some(Profile::workspace_write()), true, External { network: Enabled }, Some(MANAGED_DOMAINS), OnRequest, true, true; "external secondary retains thread denial under wildcard")]
#[test_case(Some(External { network: Enabled }), true, Profile::workspace_write(), Some(MANAGED_DOMAINS), Never, true, true; "managed secondary cannot restore rejected grants")]
#[test_case(Some(Disabled), false, Disabled, Some(MANAGED_DOMAINS), Never, true, false; "full access primary uses direct networking")]
#[test_case(Some(Disabled), false, Profile::workspace_write(), Some(MANAGED_DOMAINS), Never, false, false; "restricted secondary stays offline with full access primary")]
#[test_case(Some(Profile::workspace_write()), true, Disabled, Some(MANAGED_DOMAINS), OnRequest, true, true; "full access secondary uses primary proxy")]
#[test_case(Some(Profile::workspace_write()), true, Profile::workspace_write(), Some(MANAGED_DOMAINS), OnRequest, true, false; "managed call may approve misses")]
#[test_case(Some(Profile::workspace_write()), true, Profile::workspace_write(), Some(MANAGED_DOMAINS), Never, true, true; "never denies routed misses")]
#[test_case(Some(Profile::workspace_write()), true, Profile::workspace_write(), None, OnRequest, true, true; "profile-only proxy denies misses")]
#[test_case(Some(Disabled), false, Disabled, None, OnRequest, true, false; "direct call bypasses profile and saved rules")]
#[test_case(Some(Disabled), false, Disabled, Some("http_port = 3128"), OnRequest, true, false; "managed port leaves direct calls unrestricted")]
fn controller_allowlist_misses_follow_call_profile(
    thread: Option<Profile>,
    primary_allows_proxy: bool,
    profile: Profile,
    requirements: Option<&str>,
    approval: AskForApproval,
    enabled: bool,
    hard_deny: bool,
) -> anyhow::Result<()> {
    let mut config: NetworkProxyConfig = toml::from_str(
        r#"
        enabled = true
        mode = 'limited'
        allow_local_binding = true
        domains = { '**.profile.test' = 'allow', 'blocked.example' = 'deny' }
        mitm = true
        mitm_hooks = [{ host = 'Hook.Profile.Test.:443', match = { methods = ['GET'], path_prefixes = ['/allowed/'] } }]
    "#,
    )?;
    config.review_domains.push("review.profile.test".into());
    let spec = NetworkProxySpec::from_config_and_constraints(
        config,
        requirements.map(toml::from_str).transpose()?,
        &profile,
    )?;
    let saved = saved_rules(&[
        ("review.profile.test", Decision::Allow),
        ("saved-deny.test", Decision::Forbidden),
    ])?;
    let result = NetworkProxySpec::mcp_network_policy(
        thread.as_ref().map(|_| &spec),
        thread.as_ref().unwrap_or(&profile),
        McpNetworkRoute {
            primary_allows_proxy,
            local_binding: LocalBindingPolicy::DefaultFalse,
            owner: None,
        },
        &profile,
        approval,
        &saved,
    )?;
    let mut allowed = Vec::new();
    let mut denied = Vec::new();
    if thread.is_some() && requirements == Some(MANAGED_DOMAINS) {
        allowed.push("*.example");
        denied.push("admin.test");
    }
    let uses_profile_rules = matches!(thread, Some(Profile::Managed { .. }));
    if uses_profile_rules {
        allowed.extend(["**.profile.test", "review.profile.test"]);
        denied.extend(["blocked.example", "saved-deny.test"]);
    }
    if primary_allows_proxy {
        denied.extend(["review.profile.test", "hook.profile.test"]);
    }
    assert_eq!(
        serde_json::to_value(result)?,
        json!({
            "enabled": enabled, "mode": if primary_allows_proxy { "limited" } else { "full" },
            "allowedDomains": allowed, "deniedDomains": denied, "hardDenyAllowlistMisses": hard_deny,
        })
    );
    Ok(())
}

const FIXED_CONTROLLER_RULES: &str = "managed_allowed_domains_only = true\ndomains = { 'managed.example' = 'allow', 'managed-denied.example' = 'deny' }";
const ACTIVE_CONTROLLER: &str =
    "enabled = true\nmode = 'limited'\ndomains = { 'controller-denied.example' = 'deny' }";

#[test_case(None, None, None, [true, false, true]; "absent controller")]
#[test_case(Some("enabled = false"), Some(FIXED_CONTROLLER_RULES), Some("managed-denied.example"), [false; 3]; "disabled controller with a disjoint managed allowlist")]
#[test_case(Some(ACTIVE_CONTROLLER), None, Some("controller-denied.example"), [true; 3]; "profile-only proxy with a reviewable owner")]
fn owner_activation_and_traffic_policy(
    configured: Option<&str>,
    managed: Option<&str>,
    controller_denial: Option<&str>,
    [required, offline, direct]: [bool; 3],
) -> anyhow::Result<()> {
    let fixed_owner = managed.is_some() || configured.is_none();
    let spec = NetworkProxySpec::from_config_and_constraints(
        toml::from_str(configured.unwrap_or_default())?,
        managed.map(toml::from_str).transpose()?,
        &Profile::read_only(),
    )?;
    let controller = configured.or(managed).map(|_| &spec);
    let mut owner: EnvironmentNetworkPolicy = serde_json::from_value(json!({
        "domains": {"owner.example": "allow", "owner-denied.example": "deny"},
        "reviewDomains": ["owner-review.example"], "allowUpstreamProxy": true,
        "allowLocalBinding": true, "dangerouslyAllowAllUnixSockets": false, "managedAllowedDomainsOnly": fixed_owner,
    }))?;
    let saved = saved_rules(&[("owner-denied.example", Decision::Allow)])?;
    let mut denied = vec!["owner-denied.example"];
    denied.extend(controller_denial);
    denied.push("owner-review.example");
    for (requires_proxy, network, expected) in [
        (true, Restricted, required),
        (false, Restricted, offline),
        (false, Enabled, direct),
    ] {
        owner.requires_proxy = requires_proxy;
        let profile = Profile::from_runtime_permissions(
            &Profile::read_only().file_system_sandbox_policy(),
            network,
        );
        for approval in [OnRequest, Never] {
            let result = NetworkProxySpec::mcp_network_policy(
                controller,
                &Disabled,
                McpNetworkRoute {
                    primary_allows_proxy: false,
                    local_binding: LocalBindingPolicy::DefaultFalse,
                    owner: Some(&owner),
                },
                &profile,
                approval,
                &saved,
            )?;
            assert_eq!(
                json!(result),
                json!({
                    "enabled": expected, "mode": if fixed_owner { "full" } else { "limited" }, "allowedDomains": ["owner.example"],
                    "deniedDomains": denied, "hardDenyAllowlistMisses": fixed_owner || approval == Never,
                })
            );
        }
    }
    Ok(())
}

const MIXED_DOMAINS: &str = r#"
[permissions.profile.network.domains]
"*" = "allow"
"*.example" = "allow"
"named.example" = "allow"
"localhost" = "allow"
"127.0.0.1:*" = "allow"
"192.168.0.2" = "allow"
"[::1]" = "allow"
"#;

#[test_case("domains = { 'allowed.example' = 'allow' }", "outside.invalid", true, &["allowed.example"]; "allowlist miss")]
#[test_case(MIXED_DOMAINS, "127.0.0.1", false, &["192.168.0.2", "[::1]"]; "wildcard cannot grant loopback")]
#[test_case(MIXED_DOMAINS, "192.168.0.1", false, &["192.168.0.2", "[::1]"]; "wildcard cannot grant private address")]
#[tokio::test]
async fn profile_only_allowlist_projection_matches_native_proxy_denial(
    domains: &str,
    blocked_host: &str,
    allow_local_binding: bool,
    expected_grants: &[&str],
) -> anyhow::Result<()> {
    let home = tempfile::tempdir()?;
    let source = format!(
        r#"
default_permissions = "profile"
features.network_proxy = true
permissions.profile.filesystem.":root" = "read"
[permissions.profile.network]
enabled = true
enable_socks5 = false
allow_local_binding = {allow_local_binding}
{domains}
"#
    );
    std::fs::write(home.path().join("config.toml"), source)?;
    let config = ConfigBuilder::without_managed_config_for_tests()
        .codex_home(home.path().to_path_buf())
        .without_project_context()
        .build()
        .await?;
    let spec = config
        .permissions
        .network
        .as_ref()
        .context("loaded profile proxy")?;
    let profile = config.permissions.permission_profile();
    let requirements = config.config_layer_stack.requirements_toml();
    assert!(requirements.network.is_none());
    let result = primary_controller_policy(spec, profile, &Policy::empty())?;
    assert!(result.enabled && result.hard_deny_allowlist_misses);
    assert_eq!(result.allowed_domains, expected_grants);
    core_test_support::skip_if_no_network!(Ok(()));
    let proxy = spec
        .start_proxy(
            profile,
            ManagedProxyRouting::DedicatedListeners,
            LocalBindingPolicy::DefaultFalse,
            /*policy_decider*/ None,
            /*blocked_request_observer*/ None,
            /*enable_network_approval_flow*/ false,
            NetworkProxyAuditMetadata::default(),
        )
        .await?;
    let response = HttpClientBuilder::new()
        .build_direct()?
        .get(format!("http://{}/", proxy.proxy().http_addr()))
        .header(http::header::HOST, blocked_host)
        .timeout(Duration::from_secs(5))
        .send()
        .await?;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(response.headers()["x-proxy-error"], "blocked-by-allowlist");
    let denial = response.json::<Value>().await?;
    assert_eq!(denial["host"], blocked_host);
    assert_eq!(
        denial["reason"],
        if allow_local_binding {
            "not_allowed"
        } else {
            "not_allowed_local"
        }
    );
    Ok(())
}
