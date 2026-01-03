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

// This is the stable identifier we use to find/update the rule idempotently.
// It intentionally does not change between installs.
const OFFLINE_BLOCK_RULE_NAME: &str = "codex_sandbox_offline_block_outbound";

// Friendly text shown in the firewall UI.
const OFFLINE_BLOCK_RULE_FRIENDLY: &str = "Codex Sandbox Offline - Block Outbound";

pub fn ensure_offline_outbound_block(offline_sid: &str, log: &mut File) -> Result<()> {
    let local_user_spec = format!("O:LSD:(A;;CC;;;{offline_sid})");

    let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    if hr.is_err() {
        return Err(anyhow::anyhow!("CoInitializeEx failed: {hr:?}"));
    }

    let result = unsafe {
        (|| -> Result<()> {
            let policy: INetFwPolicy2 = CoCreateInstance(&NetFwPolicy2, None, CLSCTX_INPROC_SERVER)
                .map_err(|e| anyhow::anyhow!("CoCreateInstance NetFwPolicy2: {e:?}"))?;
            let rules = policy
                .Rules()
                .map_err(|e| anyhow::anyhow!("INetFwPolicy2::Rules: {e:?}"))?;

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
        Ok(existing) => existing
            .cast()
            .map_err(|e| anyhow::anyhow!("cast existing firewall rule to INetFwRule3: {e:?}"))?,
        Err(_) => {
            let new_rule: INetFwRule3 =
                unsafe { CoCreateInstance(&NetFwRule, None, CLSCTX_INPROC_SERVER) }
                    .map_err(|e| anyhow::anyhow!("CoCreateInstance NetFwRule: {e:?}"))?;
            unsafe { new_rule.SetName(&name) }.map_err(|e| anyhow::anyhow!("SetName: {e:?}"))?;
            // Set all properties before adding the rule so we don't leave half-configured rules.
            configure_rule(
                &new_rule,
                friendly_desc,
                protocol,
                local_user_spec,
                offline_sid,
            )?;
            unsafe { rules.Add(&new_rule) }.map_err(|e| anyhow::anyhow!("Rules::Add: {e:?}"))?;
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
            .map_err(|e| anyhow::anyhow!("SetDescription: {e:?}"))?;
        rule.SetDirection(NET_FW_RULE_DIR_OUT)
            .map_err(|e| anyhow::anyhow!("SetDirection: {e:?}"))?;
        rule.SetAction(NET_FW_ACTION_BLOCK)
            .map_err(|e| anyhow::anyhow!("SetAction: {e:?}"))?;
        rule.SetEnabled(VARIANT_TRUE)
            .map_err(|e| anyhow::anyhow!("SetEnabled: {e:?}"))?;
        rule.SetProfiles(NET_FW_PROFILE2_ALL.0)
            .map_err(|e| anyhow::anyhow!("SetProfiles: {e:?}"))?;
        rule.SetProtocol(protocol)
            .map_err(|e| anyhow::anyhow!("SetProtocol: {e:?}"))?;
        rule.SetLocalUserAuthorizedList(&BSTR::from(local_user_spec))
            .map_err(|e| anyhow::anyhow!("SetLocalUserAuthorizedList: {e:?}"))?;
    }

    // Read-back verification: ensure we actually wrote the expected SID scope.
    let actual = unsafe { rule.LocalUserAuthorizedList() }
        .map_err(|e| anyhow::anyhow!("LocalUserAuthorizedList (read-back): {e:?}"))?;
    let actual_str = actual.to_string();
    if !actual_str.contains(offline_sid) {
        anyhow::bail!(
            "offline firewall rule user scope mismatch: expected SID {offline_sid}, got {actual_str}"
        );
    }
    Ok(())
}

fn log_line(log: &mut File, msg: &str) -> Result<()> {
    let ts = chrono::Utc::now().to_rfc3339();
    writeln!(log, "[{ts}] {msg}")?;
    Ok(())
}
