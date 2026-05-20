use crate::SetupErrorCode;
use crate::dpapi;
use crate::extract_setup_failure;
use crate::logging::debug_log;
use crate::logging::log_note;
use crate::policy::SandboxPolicy;
use crate::setup::SandboxNetworkIdentity;
use crate::setup::SandboxUserRecord;
use crate::setup::SandboxUsersFile;
use crate::setup::SetupMarker;
use crate::setup::gather_read_roots;
use crate::setup::gather_write_roots;
use crate::setup::offline_proxy_settings_from_env;
use crate::setup::run_elevated_setup;
use crate::setup::run_setup_refresh_with_overrides;
use crate::setup::sandbox_users_path;
use crate::setup::setup_marker_path;
use crate::setup_error::DeferredElevatedSetup;
use crate::setup_error::clear_deferred_elevated_setup;
use crate::setup_error::read_deferred_elevated_setup;
use crate::setup_error::write_deferred_elevated_setup;
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

fn setup_deferred_for_reason(codex_home: &Path, reason: &str) -> Result<bool> {
    let deferred = read_deferred_elevated_setup(codex_home)?;
    Ok(deferred
        .as_ref()
        .is_some_and(|entry| entry.reason == reason))
}

fn reset_deferred_setup_if_reason_changed(
    codex_home: &Path,
    current_reason: Option<&str>,
) -> Result<()> {
    let Some(deferred) = read_deferred_elevated_setup(codex_home)? else {
        return Ok(());
    };
    if current_reason.is_some_and(|reason| reason == deferred.reason) {
        return Ok(());
    }
    clear_deferred_elevated_setup(codex_home)
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
    policy: &SandboxPolicy,
    policy_cwd: &Path,
    command_cwd: &Path,
    env_map: &HashMap<String, String>,
    codex_home: &Path,
    read_roots_override: Option<&[PathBuf]>,
    read_roots_include_platform_defaults: bool,
    write_roots_override: Option<&[PathBuf]>,
    deny_read_paths_override: &[PathBuf],
    deny_write_paths_override: &[PathBuf],
    proxy_enforced: bool,
) -> Result<SandboxCreds> {
    let sandbox_dir = crate::setup::sandbox_dir(codex_home);
    let needed_read = read_roots_override
        .map(<[PathBuf]>::to_vec)
        .unwrap_or_else(|| gather_read_roots(command_cwd, policy, codex_home));
    let needed_write = write_roots_override
        .map(<[PathBuf]>::to_vec)
        .unwrap_or_else(|| gather_write_roots(policy, policy_cwd, command_cwd, env_map));
    let network_identity = SandboxNetworkIdentity::from_policy(policy, proxy_enforced);
    let desired_offline_proxy_settings = offline_proxy_settings_from_env(env_map, network_identity);
    // NOTE: Do not add CODEX_HOME/.sandbox to `needed_write`; it must remain non-writable by the
    // restricted capability token. The setup helper's `lock_sandbox_dir` is responsible for
    // granting the sandbox group access to this directory without granting the capability SID.
    let mut setup_reason: Option<String> = None;

    let mut identity = match load_marker(codex_home)? {
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

    reset_deferred_setup_if_reason_changed(codex_home, setup_reason.as_deref())?;

    if identity.is_none() {
        if let Some(reason) = &setup_reason {
            if setup_deferred_for_reason(codex_home, reason)? {
                log_note(
                    &format!("sandbox setup still deferred after previous cancellation: {reason}"),
                    Some(&sandbox_dir),
                );
                return Err(anyhow!(
                    "Windows sandbox setup requires elevation ({reason}), but the previous setup prompt was canceled; rerun setup with elevation or change the sandbox settings before retrying"
                ));
            }
            log_note(
                &format!("sandbox setup required: {reason}"),
                Some(&sandbox_dir),
            );
        } else {
            log_note("sandbox setup required", Some(&sandbox_dir));
        }
        let setup_result = run_elevated_setup(
            crate::setup::SandboxSetupRequest {
                policy,
                policy_cwd,
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
        );
        match setup_result {
            Ok(()) => clear_deferred_elevated_setup(codex_home)?,
            Err(err) => {
                if extract_setup_failure(&err).is_some_and(|failure| {
                    failure.code == SetupErrorCode::OrchestratorHelperLaunchCanceled
                }) {
                    let deferred_reason = setup_reason
                        .clone()
                        .unwrap_or_else(|| "sandbox setup required".to_string());
                    write_deferred_elevated_setup(
                        codex_home,
                        &DeferredElevatedSetup {
                            reason: deferred_reason.clone(),
                        },
                    )?;
                    log_note(
                        &format!(
                            "sandbox setup deferred after canceled elevation prompt: {deferred_reason}"
                        ),
                        Some(&sandbox_dir),
                    );
                }
                return Err(err);
            }
        }
        identity = select_identity(network_identity, codex_home)?;
    }
    // Always refresh ACLs (non-elevated) for current roots via the setup binary.
    run_setup_refresh_with_overrides(
        crate::setup::SandboxSetupRequest {
            policy,
            policy_cwd,
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
