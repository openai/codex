//! Exercises account preparation with real marker files and DPAPI credentials.
//! Setup and account lookup callbacks isolate machine state. This does not exercise
//! the public wrapper, native account queries, payload serialization, singleflight,
//! or helper launches.

use super::require_sandbox_account_with_setup;
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
use windows_sys::Win32::NetworkManagement::NetManagement::UF_NORMAL_ACCOUNT;
use windows_sys::Win32::NetworkManagement::NetManagement::UF_PASSWORD_EXPIRED;

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
fn credential_setup_reconciles_effective_firewall_policy() -> Result<()> {
    let permissions = ResolvedWindowsSandboxPermissions::try_from_permission_profile(
        &PermissionProfile::read_only(),
    )?;
    for (stored_binding, desired_binding, ports, expected_full_setups) in [
        (true, true, "8080", 0),
        (true, true, "", 0),
        (true, true, "8080,3129", 0),
        (false, false, "8080", 1),
        (false, false, "", 1),
        (false, true, "3128", 1),
        (true, false, "3128", 1),
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
                |_, desired| {
                    full_setups.set(full_setups.get() + 1);
                    let mut reconciled = marker.clone();
                    reconciled.proxy_ports = desired.proxy_ports.clone();
                    reconciled.allow_local_binding = desired.allow_local_binding;
                    fs::write(
                        setup_marker_path(home.path()),
                        serde_json::to_vec(&reconciled)?,
                    )?;
                    Ok(())
                },
                |_| Ok(Some(UF_NORMAL_ACCOUNT)),
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
