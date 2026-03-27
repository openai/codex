#![cfg(target_os = "windows")]

use anyhow::Result;
use std::fs::File;
use std::io::Write;

use windows::Win32::Foundation::VARIANT_TRUE;
use windows::Win32::NetworkManagement::WindowsFirewall::INetFwPolicy2;
use windows::Win32::NetworkManagement::WindowsFirewall::INetFwRule3;
use windows::Win32::NetworkManagement::WindowsFirewall::INetFwRules;
use windows::Win32::NetworkManagement::WindowsFirewall::NET_FW_ACTION_BLOCK;
use windows::Win32::NetworkManagement::WindowsFirewall::NET_FW_IP_PROTOCOL_ANY;
use windows::Win32::NetworkManagement::WindowsFirewall::NET_FW_IP_PROTOCOL_TCP;
use windows::Win32::NetworkManagement::WindowsFirewall::NET_FW_IP_PROTOCOL_UDP;
use windows::Win32::NetworkManagement::WindowsFirewall::NET_FW_PROFILE2_ALL;
use windows::Win32::NetworkManagement::WindowsFirewall::NET_FW_RULE_DIR_OUT;
use windows::Win32::NetworkManagement::WindowsFirewall::NetFwPolicy2;
use windows::Win32::NetworkManagement::WindowsFirewall::NetFwRule;
use windows::Win32::System::Com::CLSCTX_INPROC_SERVER;
use windows::Win32::System::Com::COINIT_APARTMENTTHREADED;
use windows::Win32::System::Com::CoCreateInstance;
use windows::Win32::System::Com::CoInitializeEx;
use windows::Win32::System::Com::CoUninitialize;
use windows::core::BSTR;
use windows::core::Interface;

use codex_windows_sandbox::SetupErrorCode;
use codex_windows_sandbox::SetupFailure;

// This is the stable identifier we use to find/update the rule idempotently.
// It intentionally does not change between installs.
const OFFLINE_BLOCK_RULE_NAME: &str = "codex_sandbox_offline_block_outbound";
const OFFLINE_BLOCK_LOOPBACK_TCP_RULE_NAME: &str = "codex_sandbox_offline_block_loopback_tcp";
const OFFLINE_BLOCK_LOOPBACK_UDP_RULE_NAME: &str = "codex_sandbox_offline_block_loopback_udp";

// Friendly text shown in the firewall UI.
const OFFLINE_BLOCK_RULE_FRIENDLY: &str = "Codex Sandbox Offline - Block Non-Loopback Outbound";
const OFFLINE_BLOCK_LOOPBACK_TCP_RULE_FRIENDLY: &str =
    "Codex Sandbox Offline - Block Loopback TCP (Except Proxy)";
const OFFLINE_BLOCK_LOOPBACK_UDP_RULE_FRIENDLY: &str = "Codex Sandbox Offline - Block Loopback UDP";
const OFFLINE_PROXY_ALLOW_RULE_NAME: &str = "codex_sandbox_offline_allow_loopback_proxy";
const LOOPBACK_REMOTE_ADDRESSES: &str = "127.0.0.0/8,::1";
const NON_LOOPBACK_REMOTE_ADDRESSES: &str = "0.0.0.0-126.255.255.255,128.0.0.0-255.255.255.255,::,::2-ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff";

struct BlockRuleSpec<'a> {
    internal_name: &'a str,
    friendly_desc: &'a str,
    protocol: i32,
    local_user_spec: &'a str,
    offline_sid: &'a str,
    remote_addresses: Option<&'a str>,
    remote_ports: Option<&'a str>,
}

pub fn ensure_offline_proxy_allowlist(
    offline_sid: &str,
    proxy_ports: &[u16],
    allow_local_binding: bool,
    log: &mut File,
) -> Result<()> {
    let local_user_spec = format!("O:LSD:(A;;CC;;;{offline_sid})");

    let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    if hr.is_err() {
        return Err(anyhow::Error::new(SetupFailure::new(
            SetupErrorCode::HelperFirewallComInitFailed,
            format!("CoInitializeEx failed: {hr:?}"),
        )));
    }

    let result = unsafe {
        (|| -> Result<()> {
            let policy: INetFwPolicy2 = CoCreateInstance(&NetFwPolicy2, None, CLSCTX_INPROC_SERVER)
                .map_err(|err| {
                    anyhow::Error::new(SetupFailure::new(
                        SetupErrorCode::HelperFirewallPolicyAccessFailed,
                        format!("CoCreateInstance NetFwPolicy2 failed: {err:?}"),
                    ))
                })?;
            let rules = policy.Rules().map_err(|err| {
                anyhow::Error::new(SetupFailure::new(
                    SetupErrorCode::HelperFirewallPolicyAccessFailed,
                    format!("INetFwPolicy2::Rules failed: {err:?}"),
                ))
            })?;

            if allow_local_binding {
                // Remove the legacy overlapping allow rule before returning to the local-binding
                // mode so stale proxy exceptions do not linger.
                remove_rule_if_present(&rules, OFFLINE_PROXY_ALLOW_RULE_NAME, log)?;
                remove_rule_if_present(&rules, OFFLINE_BLOCK_LOOPBACK_UDP_RULE_NAME, log)?;
                remove_rule_if_present(&rules, OFFLINE_BLOCK_LOOPBACK_TCP_RULE_NAME, log)?;
                return Ok(());
            }

            ensure_block_rule(
                &rules,
                &BlockRuleSpec {
                    internal_name: OFFLINE_BLOCK_LOOPBACK_UDP_RULE_NAME,
                    friendly_desc: OFFLINE_BLOCK_LOOPBACK_UDP_RULE_FRIENDLY,
                    protocol: NET_FW_IP_PROTOCOL_UDP.0,
                    local_user_spec: &local_user_spec,
                    offline_sid,
                    remote_addresses: Some(LOOPBACK_REMOTE_ADDRESSES),
                    remote_ports: None,
                },
                log,
            )?;

            // Install a broad TCP loopback block before narrowing it to the allowed proxy port
            // complement. If the narrowing update fails, the sandbox remains fail-closed.
            ensure_block_rule(
                &rules,
                &BlockRuleSpec {
                    internal_name: OFFLINE_BLOCK_LOOPBACK_TCP_RULE_NAME,
                    friendly_desc: OFFLINE_BLOCK_LOOPBACK_TCP_RULE_FRIENDLY,
                    protocol: NET_FW_IP_PROTOCOL_TCP.0,
                    local_user_spec: &local_user_spec,
                    offline_sid,
                    remote_addresses: Some(LOOPBACK_REMOTE_ADDRESSES),
                    remote_ports: None,
                },
                log,
            )?;

            // Remove the legacy overlapping allow rule only after the explicit block rules are in
            // place so transitions back to proxy-only mode do not fail open.
            remove_rule_if_present(&rules, OFFLINE_PROXY_ALLOW_RULE_NAME, log)?;

            if let Some(blocked_remote_ports) = blocked_loopback_tcp_remote_ports(proxy_ports) {
                ensure_block_rule(
                    &rules,
                    &BlockRuleSpec {
                        internal_name: OFFLINE_BLOCK_LOOPBACK_TCP_RULE_NAME,
                        friendly_desc: OFFLINE_BLOCK_LOOPBACK_TCP_RULE_FRIENDLY,
                        protocol: NET_FW_IP_PROTOCOL_TCP.0,
                        local_user_spec: &local_user_spec,
                        offline_sid,
                        remote_addresses: Some(LOOPBACK_REMOTE_ADDRESSES),
                        remote_ports: Some(&blocked_remote_ports),
                    },
                    log,
                )?;
            }
            Ok(())
        })()
    };

    unsafe {
        CoUninitialize();
    }
    result
}

pub fn ensure_offline_outbound_block(offline_sid: &str, log: &mut File) -> Result<()> {
    let local_user_spec = format!("O:LSD:(A;;CC;;;{offline_sid})");

    let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    if hr.is_err() {
        return Err(anyhow::Error::new(SetupFailure::new(
            SetupErrorCode::HelperFirewallComInitFailed,
            format!("CoInitializeEx failed: {hr:?}"),
        )));
    }

    let result = unsafe {
        (|| -> Result<()> {
            let policy: INetFwPolicy2 = CoCreateInstance(&NetFwPolicy2, None, CLSCTX_INPROC_SERVER)
                .map_err(|err| {
                    anyhow::Error::new(SetupFailure::new(
                        SetupErrorCode::HelperFirewallPolicyAccessFailed,
                        format!("CoCreateInstance NetFwPolicy2 failed: {err:?}"),
                    ))
                })?;
            let rules = policy.Rules().map_err(|err| {
                anyhow::Error::new(SetupFailure::new(
                    SetupErrorCode::HelperFirewallPolicyAccessFailed,
                    format!("INetFwPolicy2::Rules failed: {err:?}"),
                ))
            })?;

            // Block all outbound IP protocols for this user.
            ensure_block_rule(
                &rules,
                &BlockRuleSpec {
                    internal_name: OFFLINE_BLOCK_RULE_NAME,
                    friendly_desc: OFFLINE_BLOCK_RULE_FRIENDLY,
                    protocol: NET_FW_IP_PROTOCOL_ANY.0,
                    local_user_spec: &local_user_spec,
                    offline_sid,
                    remote_addresses: Some(NON_LOOPBACK_REMOTE_ADDRESSES),
                    remote_ports: None,
                },
                log,
            )?;
            Ok(())
        })()
    };

    unsafe {
        CoUninitialize();
    }
    result
}

fn remove_rule_if_present(rules: &INetFwRules, internal_name: &str, log: &mut File) -> Result<()> {
    let name = BSTR::from(internal_name);
    if unsafe { rules.Item(&name) }.is_ok() {
        unsafe { rules.Remove(&name) }.map_err(|err| {
            anyhow::Error::new(SetupFailure::new(
                SetupErrorCode::HelperFirewallRuleCreateOrAddFailed,
                format!("Rules::Remove failed for {internal_name}: {err:?}"),
            ))
        })?;
        log_line(log, &format!("firewall rule removed name={internal_name}"))?;
    }
    Ok(())
}

fn ensure_block_rule(rules: &INetFwRules, spec: &BlockRuleSpec<'_>, log: &mut File) -> Result<()> {
    let name = BSTR::from(spec.internal_name);
    let rule: INetFwRule3 = match unsafe { rules.Item(&name) } {
        Ok(existing) => existing.cast().map_err(|err| {
            anyhow::Error::new(SetupFailure::new(
                SetupErrorCode::HelperFirewallRuleCreateOrAddFailed,
                format!("cast existing firewall rule to INetFwRule3 failed: {err:?}"),
            ))
        })?,
        Err(_) => {
            let new_rule: INetFwRule3 =
                unsafe { CoCreateInstance(&NetFwRule, None, CLSCTX_INPROC_SERVER) }.map_err(
                    |err| {
                        anyhow::Error::new(SetupFailure::new(
                            SetupErrorCode::HelperFirewallRuleCreateOrAddFailed,
                            format!("CoCreateInstance NetFwRule failed: {err:?}"),
                        ))
                    },
                )?;
            unsafe { new_rule.SetName(&name) }.map_err(|err| {
                anyhow::Error::new(SetupFailure::new(
                    SetupErrorCode::HelperFirewallRuleCreateOrAddFailed,
                    format!("SetName failed: {err:?}"),
                ))
            })?;
            // Set all properties before adding the rule so we don't leave half-configured rules.
            configure_rule(&new_rule, spec)?;
            unsafe { rules.Add(&new_rule) }.map_err(|err| {
                anyhow::Error::new(SetupFailure::new(
                    SetupErrorCode::HelperFirewallRuleCreateOrAddFailed,
                    format!("Rules::Add failed: {err:?}"),
                ))
            })?;
            new_rule
        }
    };

    // Always re-apply fields to keep the setup idempotent.
    configure_rule(&rule, spec)?;

    let remote_addresses_log = spec.remote_addresses.unwrap_or("*");
    let remote_ports_log = spec.remote_ports.unwrap_or("*");

    log_line(
        log,
        &format!(
            "firewall rule configured name={} protocol={} RemoteAddresses={remote_addresses_log} RemotePorts={remote_ports_log} LocalUserAuthorizedList={}",
            spec.internal_name, spec.protocol, spec.local_user_spec
        ),
    )?;
    Ok(())
}

fn configure_rule(rule: &INetFwRule3, spec: &BlockRuleSpec<'_>) -> Result<()> {
    unsafe {
        rule.SetDescription(&BSTR::from(spec.friendly_desc))
            .map_err(|err| {
                anyhow::Error::new(SetupFailure::new(
                    SetupErrorCode::HelperFirewallRuleCreateOrAddFailed,
                    format!("SetDescription failed: {err:?}"),
                ))
            })?;
        rule.SetDirection(NET_FW_RULE_DIR_OUT).map_err(|err| {
            anyhow::Error::new(SetupFailure::new(
                SetupErrorCode::HelperFirewallRuleCreateOrAddFailed,
                format!("SetDirection failed: {err:?}"),
            ))
        })?;
        rule.SetAction(NET_FW_ACTION_BLOCK).map_err(|err| {
            anyhow::Error::new(SetupFailure::new(
                SetupErrorCode::HelperFirewallRuleCreateOrAddFailed,
                format!("SetAction failed: {err:?}"),
            ))
        })?;
        rule.SetEnabled(VARIANT_TRUE).map_err(|err| {
            anyhow::Error::new(SetupFailure::new(
                SetupErrorCode::HelperFirewallRuleCreateOrAddFailed,
                format!("SetEnabled failed: {err:?}"),
            ))
        })?;
        rule.SetProfiles(NET_FW_PROFILE2_ALL.0).map_err(|err| {
            anyhow::Error::new(SetupFailure::new(
                SetupErrorCode::HelperFirewallRuleCreateOrAddFailed,
                format!("SetProfiles failed: {err:?}"),
            ))
        })?;
        rule.SetProtocol(spec.protocol).map_err(|err| {
            anyhow::Error::new(SetupFailure::new(
                SetupErrorCode::HelperFirewallRuleCreateOrAddFailed,
                format!("SetProtocol failed: {err:?}"),
            ))
        })?;
        let remote_addresses = spec.remote_addresses.unwrap_or("*");
        rule.SetRemoteAddresses(&BSTR::from(remote_addresses))
            .map_err(|err| {
                anyhow::Error::new(SetupFailure::new(
                    SetupErrorCode::HelperFirewallRuleCreateOrAddFailed,
                    format!("SetRemoteAddresses failed: {err:?}"),
                ))
            })?;
        let remote_ports = spec.remote_ports.unwrap_or("*");
        rule.SetRemotePorts(&BSTR::from(remote_ports))
            .map_err(|err| {
                anyhow::Error::new(SetupFailure::new(
                    SetupErrorCode::HelperFirewallRuleCreateOrAddFailed,
                    format!("SetRemotePorts failed: {err:?}"),
                ))
            })?;
        rule.SetLocalUserAuthorizedList(&BSTR::from(spec.local_user_spec))
            .map_err(|err| {
                anyhow::Error::new(SetupFailure::new(
                    SetupErrorCode::HelperFirewallRuleCreateOrAddFailed,
                    format!("SetLocalUserAuthorizedList failed: {err:?}"),
                ))
            })?;
    }

    // Read-back verification: ensure we actually wrote the expected SID scope.
    let actual = unsafe { rule.LocalUserAuthorizedList() }.map_err(|err| {
        anyhow::Error::new(SetupFailure::new(
            SetupErrorCode::HelperFirewallRuleVerifyFailed,
            format!("LocalUserAuthorizedList (read-back) failed: {err:?}"),
        ))
    })?;
    let actual_str = actual.to_string();
    if !actual_str.contains(spec.offline_sid) {
        return Err(anyhow::Error::new(SetupFailure::new(
            SetupErrorCode::HelperFirewallRuleVerifyFailed,
            format!(
                "offline firewall rule user scope mismatch: expected SID {}, got {actual_str}",
                spec.offline_sid
            ),
        )));
    }
    Ok(())
}

fn blocked_loopback_tcp_remote_ports(proxy_ports: &[u16]) -> Option<String> {
    let mut allowed_ports = proxy_ports
        .iter()
        .copied()
        .filter(|port| *port != 0)
        .collect::<Vec<_>>();
    allowed_ports.sort_unstable();
    allowed_ports.dedup();

    let mut blocked_ranges = Vec::new();
    let mut start = 1_u32;
    for port in allowed_ports {
        let port = u32::from(port);
        if port < start {
            continue;
        }
        if port > start {
            blocked_ranges.push(port_range_string(start, port - 1));
        }
        start = port + 1;
    }

    if start <= u32::from(u16::MAX) {
        blocked_ranges.push(port_range_string(start, u32::from(u16::MAX)));
    }

    if blocked_ranges.is_empty() {
        None
    } else {
        Some(blocked_ranges.join(","))
    }
}

fn port_range_string(start: u32, end: u32) -> String {
    if start == end {
        start.to_string()
    } else {
        format!("{start}-{end}")
    }
}

fn log_line(log: &mut File, msg: &str) -> Result<()> {
    let ts = chrono::Utc::now().to_rfc3339();
    writeln!(log, "[{ts}] {msg}")?;
    Ok(())
}
