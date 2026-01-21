use crate::dpapi;
use crate::logging::debug_log;
use crate::policy::SandboxPolicy;
use crate::setup::gather_read_roots;
use crate::setup::gather_write_roots;
use crate::setup::run_elevated_setup;
use crate::setup::sandbox_users_path;
use crate::setup::setup_marker_path;
use crate::setup::SandboxUserRecord;
use crate::setup::SandboxUsersFile;
use crate::setup::SetupMarker;
use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

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
/// This reuses the same marker/users validation used by `require_logon_sandbox_creds`.
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

fn select_identity(policy: &SandboxPolicy, codex_home: &Path) -> Result<Option<SandboxIdentity>> {
    let _marker = match load_marker(codex_home)? {
        Some(m) if m.version_matches() => m,
        _ => return Ok(None),
    };
    let users = match load_users(codex_home)? {
        Some(u) if u.version_matches() => u,
        _ => return Ok(None),
    };
    let chosen = if !policy.has_full_network_access() {
        users.offline
    } else {
        users.online
    };
    let password = decode_password(&chosen)?;
    Ok(Some(SandboxIdentity {
        username: chosen.username.clone(),
        password,
    }))
}

pub fn require_logon_sandbox_creds(
    policy: &SandboxPolicy,
    policy_cwd: &Path,
    command_cwd: &Path,
    env_map: &HashMap<String, String>,
    codex_home: &Path,
) -> Result<SandboxCreds> {
    let sandbox_dir = crate::setup::sandbox_dir(codex_home);
    let needed_read = gather_read_roots(command_cwd, policy);
    let needed_write = gather_write_roots(policy, policy_cwd, command_cwd, env_map);
    // NOTE: Do not add CODEX_HOME/.sandbox to `needed_write`; it must remain non-writable by the
    // restricted capability token. The setup helper's `lock_sandbox_dir` is responsible for
    // granting the sandbox group access to this directory without granting the capability SID.
    let mut setup_reason: Option<String> = None;
    let mut _existing_marker: Option<SetupMarker> = None;

    let mut identity = match load_marker(codex_home)? {
        Some(marker) if marker.version_matches() => {
            _existing_marker = Some(marker.clone());
            let selected = select_identity(policy, codex_home)?;
            if selected.is_none() {
                setup_reason =
                    Some("sandbox users missing or incompatible with marker version".to_string());
            }
            selected
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
        run_elevated_setup(
            policy,
            policy_cwd,
            command_cwd,
            env_map,
            codex_home,
            Some(needed_read.clone()),
            Some(needed_write.clone()),
        )?;
        identity = select_identity(policy, codex_home)?;
    }
    // Always refresh ACLs (non-elevated) for current roots via the setup binary.
    crate::setup::run_setup_refresh(policy, policy_cwd, command_cwd, env_map, codex_home)?;
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
