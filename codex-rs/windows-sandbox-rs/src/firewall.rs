#![cfg(target_os = "windows")]

use anyhow::Result;
use std::fs::File;
use std::io::Write;

use windows::core::Interface;
use windows::core::BSTR;
use windows::Win32::Foundation::VARIANT_TRUE;
use windows::Win32::NetworkManagement::WindowsFirewall::INetFwPolicy2;
use windows::Win32::NetworkManagement::WindowsFirewall::INetFwRule3;
use windows::Win32::NetworkManagement::WindowsFirewall::NetFwPolicy2;
use windows::Win32::NetworkManagement::WindowsFirewall::NetFwRule;
use windows::Win32::NetworkManagement::WindowsFirewall::NET_FW_ACTION_BLOCK;
use windows::Win32::NetworkManagement::WindowsFirewall::NET_FW_IP_PROTOCOL_ANY;
use windows::Win32::NetworkManagement::WindowsFirewall::NET_FW_PROFILE2_ALL;
use windows::Win32::NetworkManagement::WindowsFirewall::NET_FW_RULE_DIR_OUT;
use windows::Win32::System::Com::CoCreateInstance;
use windows::Win32::System::Com::CoInitializeEx;
use windows::Win32::System::Com::CoUninitialize;
use windows::Win32::System::Com::CLSCTX_INPROC_SERVER;
use windows::Win32::System::Com::COINIT_APARTMENTTHREADED;

use codex_windows_sandbox::SetupErrorCode;
use codex_windows_sandbox::SetupFailure;

// This is the stable identifier we use to find/update the rule idempotently.
// It intentionally does not change between installs.
const OFFLINE_BLOCK_RULE_NAME: &str = "codex_sandbox_offline_block_outbound";

// Friendly text shown in the firewall UI.
const OFFLINE_BLOCK_RULE_FRIENDLY: &str = "Codex Sandbox Offline - Block Outbound";

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
                OFFLINE_BLOCK_RULE_NAME,
                OFFLINE_BLOCK_RULE_FRIENDLY,
                NET_FW_IP_PROTOCOL_ANY.0,
                &local_user_spec,
                offline_sid,
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

fn ensure_block_rule(
    rules: &windows::Win32::NetworkManagement::WindowsFirewall::INetFwRules,
    internal_name: &str,
    friendly_desc: &str,
    protocol: i32,
    local_user_spec: &str,
    offline_sid: &str,
    log: &mut File,
) -> Result<()> {
    let name = BSTR::from(internal_name);
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
            configure_rule(
                &new_rule,
                friendly_desc,
                protocol,
                local_user_spec,
                offline_sid,
            )?;
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
    configure_rule(&rule, friendly_desc, protocol, local_user_spec, offline_sid)?;

    log_line(
        log,
        &format!(
            "firewall rule configured name={internal_name} protocol={protocol} LocalUserAuthorizedList={local_user_spec}"
        ),
    )?;
    Ok(())
}

fn configure_rule(
    rule: &INetFwRule3,
    friendly_desc: &str,
    protocol: i32,
    local_user_spec: &str,
    offline_sid: &str,
) -> Result<()> {
    unsafe {
        rule.SetDescription(&BSTR::from(friendly_desc))
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
        rule.SetProtocol(protocol).map_err(|err| {
            anyhow::Error::new(SetupFailure::new(
                SetupErrorCode::HelperFirewallRuleCreateOrAddFailed,
                format!("SetProtocol failed: {err:?}"),
            ))
        })?;
        rule.SetLocalUserAuthorizedList(&BSTR::from(local_user_spec))
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
    if !actual_str.contains(offline_sid) {
        return Err(anyhow::Error::new(SetupFailure::new(
            SetupErrorCode::HelperFirewallRuleVerifyFailed,
            format!(
                "offline firewall rule user scope mismatch: expected SID {offline_sid}, got {actual_str}"
            ),
        )));
    }
    Ok(())
}

fn log_line(log: &mut File, msg: &str) -> Result<()> {
    let ts = chrono::Utc::now().to_rfc3339();
    writeln!(log, "[{ts}] {msg}")?;
    Ok(())
}
