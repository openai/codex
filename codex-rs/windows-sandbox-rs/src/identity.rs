use crate::dpapi;
use crate::logging::debug_log;
use crate::resolved_permissions::ResolvedWindowsSandboxPermissions;
use crate::setup::SandboxNetworkIdentity;
use crate::setup::SandboxUserRecord;
use crate::setup::SandboxUsersFile;
use crate::setup::SetupMarker;
use crate::setup::gather_read_roots;
use crate::setup::gather_write_roots_for_permissions;
use crate::setup::offline_proxy_settings_from_env;
use crate::setup::run_elevated_setup_with_proxy_settings;
use crate::setup::run_setup_refresh_with_overrides_and_proxy_settings;
use crate::setup::sandbox_users_path;
use crate::setup::setup_marker_path;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone)]
struct SandboxIdentity {
    username: String,
    password: String,
}

#[derive(Debug, Clone)]
pub struct SandboxCreds {
    pub username: String,
    pub password: String,
}

/// Returns true when the on-disk setup artifacts exist and match the current
/// setup version.
///
/// This is a coarse readiness check; `require_logon_sandbox_creds` performs the
/// additional runtime validation for offline firewall settings.
pub fn sandbox_setup_is_complete(codex_home: &Path) -> bool {
    let marker_ok = matches!(load_marker(codex_home), Ok(Some(marker)) if marker.version_matches());
    if !marker_ok {
        return false;
    }
    matches!(load_users(codex_home), Ok(Some(users)) if users.version_matches())
}

fn load_marker(codex_home: &Path) -> Result<Option<SetupMarker>> {
    let path = setup_marker_path(codex_home);
    let marker = match fs::read_to_string(&path) {
        Ok(contents) => match serde_json::from_str::<SetupMarker>(&contents) {
            Ok(m) => Some(m),
            Err(err) => {
                debug_log(
                    &format!("sandbox setup marker parse failed: {err}"),
                    Some(codex_home),
                );
                None
            }
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => {
            debug_log(
                &format!("sandbox setup marker read failed: {err}"),
                Some(codex_home),
            );
            None
        }
    };
    Ok(marker)
}

fn load_users(codex_home: &Path) -> Result<Option<SandboxUsersFile>> {
    let path = sandbox_users_path(codex_home);
    let file = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            debug_log(
                &format!("sandbox users read failed: {err}"),
                Some(codex_home),
            );
            return Ok(None);
        }
    };
    match serde_json::from_str::<SandboxUsersFile>(&file) {
        Ok(users) => Ok(Some(users)),
        Err(err) => {
            debug_log(
                &format!("sandbox users parse failed: {err}"),
                Some(codex_home),
            );
            Ok(None)
        }
    }
}

fn remove_sandbox_users_file(codex_home: &Path, reason: &str) -> Result<()> {
    let path = sandbox_users_path(codex_home);
    debug_log(
        &format!("{reason}; deleting {}", path.display()),
        Some(codex_home),
    );
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err).with_context(|| format!("delete {}", path.display())),
    }
}

fn decode_password(record: &SandboxUserRecord) -> Result<String> {
    let blob = BASE64_STANDARD
        .decode(record.password.as_bytes())
        .context("base64 decode password")?;
    let decrypted = dpapi::unprotect(&blob)?;
    let pwd = String::from_utf8(decrypted).context("sandbox password not utf-8")?;
    Ok(pwd)
}

fn select_identity(
    network_identity: SandboxNetworkIdentity,
    codex_home: &Path,
) -> Result<Option<SandboxIdentity>> {
    let _marker = match load_marker(codex_home)? {
        Some(m) if m.version_matches() => m,
        _ => return Ok(None),
    };
    let users = match load_users(codex_home)? {
        Some(u) if u.version_matches() => u,
        _ => return Ok(None),
    };
    let chosen = match network_identity {
        SandboxNetworkIdentity::Offline => users.offline,
        SandboxNetworkIdentity::Online => users.online,
    };
    let password = decode_password(&chosen)?;
    Ok(Some(SandboxIdentity {
        username: chosen.username,
        password,
    }))
}

#[allow(clippy::too_many_arguments)]
pub fn require_logon_sandbox_creds(
    permissions: &ResolvedWindowsSandboxPermissions,
    command_cwd: &Path,
    env_map: &HashMap<String, String>,
    codex_home: &Path,
    read_roots_override: Option<&[PathBuf]>,
    read_roots_include_platform_defaults: bool,
    write_roots_override: Option<&[PathBuf]>,
    deny_read_paths_override: &[PathBuf],
    deny_write_paths_override: &[PathBuf],
    proxy_enforced: bool,
    proxy_settings_mode: crate::WindowsSandboxProxySettingsMode,
) -> Result<SandboxCreds> {
    let sandbox_dir = crate::setup::sandbox_dir(codex_home);
    let needed_read = read_roots_override
        .map(<[PathBuf]>::to_vec)
        .unwrap_or_else(|| gather_read_roots(command_cwd, permissions, env_map, codex_home));
    let needed_write = write_roots_override
        .map(<[PathBuf]>::to_vec)
        .unwrap_or_else(|| gather_write_roots_for_permissions(permissions, command_cwd, env_map));
    let network_identity = SandboxNetworkIdentity::from_permissions(permissions, proxy_enforced);
    let marker = load_marker(codex_home)?;
    let desired_offline_proxy_settings = desired_offline_proxy_settings(
        marker.as_ref(),
        proxy_settings_mode,
        env_map,
        network_identity,
    );
    // NOTE: Do not add CODEX_HOME/.sandbox to `needed_write`; it must remain non-writable by the
    // restricted capability token. The setup helper's `lock_sandbox_dir` is responsible for
    // granting the sandbox group access to this directory without granting the capability SID.
    let mut setup_reason: Option<String> = None;

    let mut identity = match marker {
        Some(marker) if marker.version_matches() => {
            if let Some(reason) =
                marker.request_mismatch_reason(network_identity, &desired_offline_proxy_settings)
            {
                setup_reason = Some(reason);
                None
            } else {
                let selected = select_identity(network_identity, codex_home)?;
                if selected.is_none() {
                    setup_reason = Some(
                        "sandbox users missing or incompatible with marker version".to_string(),
                    );
                }
                selected
            }
        }
        _ => {
            setup_reason = Some("sandbox setup marker missing or incompatible".to_string());
            None
        }
    };

    if identity.is_none() {
        if let Some(reason) = &setup_reason {
            crate::logging::log_note(
                &format!("sandbox setup required: {reason}"),
                Some(&sandbox_dir),
            );
        } else {
            crate::logging::log_note("sandbox setup required", Some(&sandbox_dir));
        }
        run_elevated_setup_with_proxy_settings(
            crate::setup::SandboxSetupRequest {
                permissions,
                command_cwd,
                env_map,
                codex_home,
                proxy_enforced,
            },
            crate::setup::SetupRootOverrides {
                read_roots: Some(needed_read.clone()),
                read_roots_include_platform_defaults,
                write_roots: Some(needed_write.clone()),
                deny_read_paths: Some(deny_read_paths_override.to_vec()),
                deny_write_paths: Some(deny_write_paths_override.to_vec()),
            },
            &desired_offline_proxy_settings,
        )?;
        identity = select_identity(network_identity, codex_home)?;
    }
    // Always refresh ACLs (non-elevated) for current roots via the setup binary.
    run_setup_refresh_with_overrides_and_proxy_settings(
        crate::setup::SandboxSetupRequest {
            permissions,
            command_cwd,
            env_map,
            codex_home,
            proxy_enforced,
        },
        crate::setup::SetupRootOverrides {
            read_roots: Some(needed_read),
            read_roots_include_platform_defaults,
            write_roots: Some(needed_write),
            deny_read_paths: Some(deny_read_paths_override.to_vec()),
            deny_write_paths: Some(deny_write_paths_override.to_vec()),
        },
        &desired_offline_proxy_settings,
    )?;
    let identity = identity.ok_or_else(|| {
        anyhow!(
            "Windows sandbox setup is missing or out of date; rerun the sandbox setup with elevation"
        )
    })?;
    Ok(SandboxCreds {
        username: identity.username,
        password: identity.password,
    })
}

fn desired_offline_proxy_settings(
    marker: Option<&SetupMarker>,
    proxy_settings_mode: crate::WindowsSandboxProxySettingsMode,
    env_map: &HashMap<String, String>,
    network_identity: SandboxNetworkIdentity,
) -> crate::setup::OfflineProxySettings {
    match (marker, proxy_settings_mode) {
        (Some(marker), crate::WindowsSandboxProxySettingsMode::Preserve)
            if marker.version_matches() =>
        {
            marker.offline_proxy_settings()
        }
        _ => offline_proxy_settings_from_env(env_map, network_identity),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn refresh_logon_sandbox_creds(
    permissions: &ResolvedWindowsSandboxPermissions,
    command_cwd: &Path,
    env_map: &HashMap<String, String>,
    codex_home: &Path,
    read_roots_override: Option<&[PathBuf]>,
    read_roots_include_platform_defaults: bool,
    write_roots_override: Option<&[PathBuf]>,
    deny_read_paths_override: &[PathBuf],
    deny_write_paths_override: &[PathBuf],
    proxy_enforced: bool,
    proxy_settings_mode: crate::WindowsSandboxProxySettingsMode,
) -> Result<SandboxCreds> {
    remove_sandbox_users_file(codex_home, "sandbox user login failed")?;
    require_logon_sandbox_creds(
        permissions,
        command_cwd,
        env_map,
        codex_home,
        read_roots_override,
        read_roots_include_platform_defaults,
        write_roots_override,
        deny_read_paths_override,
        deny_write_paths_override,
        proxy_enforced,
        proxy_settings_mode,
    )
}

#[cfg(test)]
mod tests {
    use super::desired_offline_proxy_settings;
    use super::remove_sandbox_users_file;
    use crate::WindowsSandboxProxySettingsMode;
    use crate::setup::SandboxNetworkIdentity;
    use crate::setup::SetupMarker;
    use crate::setup::sandbox_users_path;
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn remove_sandbox_users_file_deletes_existing_file() {
        let codex_home = TempDir::new().expect("tempdir");
        let users_path = sandbox_users_path(codex_home.path());
        fs::create_dir_all(users_path.parent().expect("sandbox secrets dir"))
            .expect("create sandbox secrets dir");
        fs::write(&users_path, "users").expect("write users");

        remove_sandbox_users_file(codex_home.path(), "stale creds").expect("remove users");
        assert!(!users_path.exists());
    }

    #[test]
    fn remove_sandbox_users_file_ignores_missing_file() {
        let codex_home = TempDir::new().expect("tempdir");
        let users_path = sandbox_users_path(codex_home.path());

        remove_sandbox_users_file(codex_home.path(), "stale creds").expect("remove users");
        assert!(!users_path.exists());
    }

    #[test]
    fn preserving_proxy_settings_uses_the_existing_marker() {
        let marker = SetupMarker {
            version: crate::setup::SETUP_VERSION,
            offline_username: "offline".to_string(),
            online_username: "online".to_string(),
            created_at: None,
            proxy_ports: vec![7890],
            allow_local_binding: true,
        };
        let env_map = HashMap::from([(
            "HTTP_PROXY".to_string(),
            "http://127.0.0.1:8080".to_string(),
        )]);

        assert_eq!(
            desired_offline_proxy_settings(
                Some(&marker),
                WindowsSandboxProxySettingsMode::Preserve,
                &env_map,
                SandboxNetworkIdentity::Offline,
            ),
            marker.offline_proxy_settings()
        );
        assert_eq!(
            desired_offline_proxy_settings(
                Some(&marker),
                WindowsSandboxProxySettingsMode::Reconcile,
                &env_map,
                SandboxNetworkIdentity::Offline,
            )
            .proxy_ports,
            vec![8080]
        );
    }
}
