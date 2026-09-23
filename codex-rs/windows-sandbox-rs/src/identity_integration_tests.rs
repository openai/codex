//! Exercises account preparation with real marker files and DPAPI credentials.
//! Setup and account lookup callbacks isolate machine state. This does not exercise
//! the public wrapper, native account queries, payload serialization, singleflight,
//! or helper launches.

use super::SandboxAccountCredentialMismatch;
use super::account_logon_error;
use super::logon_existing_sandbox_account;
use super::require_sandbox_account_with_setup;
use super::sandbox_setup_is_complete_with_settings;
use crate::SandboxRuntimeAccount;
use crate::WindowsSandboxProvisioningSettings;
use crate::WindowsSandboxProxySettingsMode;
use crate::resolved_permissions::ResolvedWindowsSandboxPermissions;
use crate::setup::OFFLINE_USERNAME;
use crate::setup::ONLINE_USERNAME;
use crate::setup::SETUP_VERSION;
use crate::setup::SandboxSetupRequest;
use crate::setup::SandboxUserRecord;
use crate::setup::SandboxUsersFile;
use crate::setup::SetupMarker;
use crate::setup::sandbox_users_path;
use crate::setup::setup_marker_path;
use anyhow::Result;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use codex_protocol::models::PermissionProfile;
use pretty_assertions::assert_eq;
use std::cell::Cell;
use std::collections::HashMap;
use std::fs;
use std::io;
use windows_sys::Win32::Foundation as win32;
use windows_sys::Win32::NetworkManagement::NetManagement::UF_ACCOUNTDISABLE;
use windows_sys::Win32::NetworkManagement::NetManagement::UF_NORMAL_ACCOUNT;
use windows_sys::Win32::NetworkManagement::NetManagement::UF_PASSWORD_EXPIRED;

#[test]
fn only_native_bad_or_expired_passwords_trigger_credential_repair() -> Result<()> {
    for code in [
        win32::ERROR_LOGON_FAILURE,
        win32::ERROR_PASSWORD_EXPIRED,
        win32::ERROR_PASSWORD_MUST_CHANGE,
    ] {
        let error = account_logon_error(io::Error::from_raw_os_error(code as i32));
        assert!(error.is::<SandboxAccountCredentialMismatch>());
        assert_eq!(
            error.downcast_ref::<io::Error>().unwrap().raw_os_error(),
            Some(code as i32)
        );
    }
    for code in [
        win32::ERROR_ACCESS_DENIED,
        win32::ERROR_ACCOUNT_RESTRICTION,
        win32::ERROR_LOGON_TYPE_NOT_GRANTED,
    ] {
        let error = account_logon_error(io::Error::from_raw_os_error(code as i32));
        assert!(!error.is::<SandboxAccountCredentialMismatch>());
    }
    let home = tempfile::tempdir()?;
    let error =
        logon_existing_sandbox_account(home.path(), SandboxRuntimeAccount::Offline).unwrap_err();
    assert!(!error.is::<SandboxAccountCredentialMismatch>());
    Ok(())
}

#[test]
fn credential_setup_repairs_expired_accounts_once_and_reloads_credentials() -> Result<()> {
    let permissions = ResolvedWindowsSandboxPermissions::try_from_permission_profile(
        &PermissionProfile::read_only(),
    )?;
    for (expired_username, repair_succeeds) in [
        (None, true),
        (Some(OFFLINE_USERNAME), true),
        (Some(ONLINE_USERNAME), true),
        (Some(OFFLINE_USERNAME), false),
        (Some(ONLINE_USERNAME), false),
    ] {
        let home = tempfile::tempdir()?;
        let marker = SetupMarker {
            version: SETUP_VERSION,
            offline_username: OFFLINE_USERNAME.into(),
            online_username: ONLINE_USERNAME.into(),
            created_at: None,
            proxy_ports: vec![],
            allow_local_binding: false,
        };
        let mut users = SandboxUsersFile {
            version: SETUP_VERSION,
            offline: SandboxUserRecord {
                username: OFFLINE_USERNAME.into(),
                password: BASE64.encode(crate::dpapi::protect(b"old-test-password")?),
            },
            online: SandboxUserRecord {
                username: ONLINE_USERNAME.into(),
                password: BASE64.encode(crate::dpapi::protect(b"old-test-password")?),
            },
        };
        for (path, bytes) in [
            (setup_marker_path(home.path()), serde_json::to_vec(&marker)?),
            (sandbox_users_path(home.path()), serde_json::to_vec(&users)?),
        ] {
            fs::create_dir_all(path.parent().unwrap())?;
            fs::write(path, bytes)?;
        }
        let setups = Cell::new(/*value*/ 0);
        let env = HashMap::new();
        let result = require_sandbox_account_with_setup(
            &SandboxSetupRequest {
                permissions: &permissions,
                command_cwd: home.path(),
                env_map: &env,
                codex_home: home.path(),
                proxy_enforced: false,
            },
            WindowsSandboxProxySettingsMode::Preserve,
            |_, _| {
                // Older services must not accept stale credentials instead of repairing expiry.
                assert!(!sandbox_users_path(home.path()).exists());
                setups.set(setups.get() + 1);
                users.offline.password =
                    BASE64.encode(crate::dpapi::protect(b"new-test-password")?);
                users.online.password = users.offline.password.clone();
                fs::write(sandbox_users_path(home.path()), serde_json::to_vec(&users)?)?;
                Ok(())
            },
            |username| {
                Ok(Some(
                    if expired_username == Some(username) && (setups.get() == 0 || !repair_succeeds)
                    {
                        UF_NORMAL_ACCOUNT | UF_PASSWORD_EXPIRED
                    } else {
                        UF_NORMAL_ACCOUNT
                    },
                ))
            },
        );
        assert_eq!(setups.get(), usize::from(expired_username.is_some()));
        assert_eq!(
            result
                .map(|(creds, _)| (creds.username, creds.password))
                .map_err(|err| err.to_string()),
            if !repair_succeeds {
                Err("Windows sandbox account password is still expired after setup".into())
            } else {
                Ok((
                    OFFLINE_USERNAME.into(),
                    if expired_username.is_some() {
                        "new-test-password"
                    } else {
                        "old-test-password"
                    }
                    .into(),
                ))
            }
        );
    }
    Ok(())
}

#[test]
fn credential_setup_reconciles_policy_and_repairs_accounts() -> Result<()> {
    let permissions = ResolvedWindowsSandboxPermissions::try_from_permission_profile(
        &PermissionProfile::read_only(),
    )?;
    let enabled = Some(UF_NORMAL_ACCOUNT);
    let disabled = Some(UF_NORMAL_ACCOUNT | UF_ACCOUNTDISABLE);
    for (stored_binding, desired_binding, ports, account_flags, expected_full_setups) in [
        (true, true, "8080", enabled, 0),
        (true, true, "", enabled, 0),
        (true, true, "8080,3129", enabled, 0),
        (false, false, "8080", enabled, 1),
        (false, false, "", enabled, 1),
        (false, true, "3128", enabled, 1),
        (true, false, "3128", enabled, 1),
        (true, true, "3128", None, 1),
        (true, true, "3128", disabled, 1),
    ] {
        let full_setups = Cell::new(/*value*/ 0);
        let home = tempfile::tempdir()?;
        let marker = SetupMarker {
            version: SETUP_VERSION,
            offline_username: "offline".into(),
            online_username: "online".into(),
            created_at: None,
            proxy_ports: vec![3128],
            allow_local_binding: stored_binding,
        };
        let user = SandboxUserRecord {
            username: marker.offline_username.clone(),
            password: BASE64.encode(crate::dpapi::protect(b"test-password")?),
        };
        let users = SandboxUsersFile {
            version: SETUP_VERSION,
            offline: user.clone(),
            online: SandboxUserRecord {
                username: marker.online_username.clone(),
                ..user
            },
        };
        let marker_bytes = serde_json::to_vec(&marker)?;
        for (path, bytes) in [
            (setup_marker_path(home.path()), marker_bytes.clone()),
            (sandbox_users_path(home.path()), serde_json::to_vec(&users)?),
        ] {
            fs::create_dir_all(path.parent().unwrap())?;
            fs::write(path, bytes)?;
        }
        let env = HashMap::from([
            ("CODEX_WINDOWS_SANDBOX_PROXY_PORTS".into(), ports.into()),
            (
                "CODEX_NETWORK_ALLOW_LOCAL_BINDING".into(),
                u8::from(desired_binding).to_string(),
            ),
        ]);
        let account_flags = Cell::new(account_flags);
        let prepare = || {
            require_sandbox_account_with_setup(
                &SandboxSetupRequest {
                    permissions: &permissions,
                    command_cwd: home.path(),
                    env_map: &env,
                    codex_home: home.path(),
                    proxy_enforced: true,
                },
                WindowsSandboxProxySettingsMode::Reconcile,
                |request, desired| {
                    // Matching artifacts must not let setup skip account repair.
                    assert!(!sandbox_setup_is_complete_with_settings(
                        request.codex_home,
                        &WindowsSandboxProvisioningSettings {
                            proxy_ports: desired.proxy_ports.clone(),
                            allow_local_binding: desired.allow_local_binding,
                        }
                    ));
                    full_setups.set(full_setups.get() + 1);
                    let mut reconciled = marker.clone();
                    reconciled.proxy_ports = desired.proxy_ports.clone();
                    reconciled.allow_local_binding = desired.allow_local_binding;
                    fs::write(
                        setup_marker_path(home.path()),
                        serde_json::to_vec(&reconciled)?,
                    )?;
                    fs::write(sandbox_users_path(home.path()), serde_json::to_vec(&users)?)?;
                    account_flags.set(enabled);
                    Ok(())
                },
                |_| Ok(account_flags.get()),
            )
        };
        for _ in 0..2 {
            let (creds, _) = prepare()?;
            assert_eq!(
                (creds.username, creds.password),
                ("offline".into(), "test-password".into())
            );
        }
        // The second command reuses reconciled settings, including when filtering removed the proxy.
        assert_eq!(full_setups.get(), expected_full_setups);
    }
    Ok(())
}
