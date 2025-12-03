use crate::dpapi;
use crate::logging::debug_log;
use crate::policy::SandboxPolicy;
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
    #[allow(dead_code)]
    offline: bool,
}

#[derive(Debug, Clone)]
pub struct SandboxCreds {
    pub username: String,
    pub password: String,
}

fn load_marker(codex_home: &Path) -> Result<Option<SetupMarker>> {
    let path = setup_marker_path(codex_home);
    let marker = match fs::read_to_string(&path) {
        Ok(contents) => match serde_json::from_str::<SetupMarker>(&contents) {
            Ok(m) => Some(m),
            Err(err) => {
                debug_log(
                    &format!("sandbox setup marker parse failed: {}", err),
                    Some(codex_home),
                );
                None
            }
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => {
            debug_log(
                &format!("sandbox setup marker read failed: {}", err),
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
                &format!("sandbox users read failed: {}", err),
                Some(codex_home),
            );
            return Ok(None);
        }
    };
    match serde_json::from_str::<SandboxUsersFile>(&file) {
        Ok(users) => Ok(Some(users)),
        Err(err) => {
            debug_log(
                &format!("sandbox users parse failed: {}", err),
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
    let offline = !policy.has_full_network_access();
    let chosen = if offline {
        users.offline
    } else {
        users.online
    };
    let password = decode_password(&chosen)?;
    Ok(Some(SandboxIdentity {
        username: chosen.username.clone(),
        password,
        offline,
    }))
}

pub fn require_logon_sandbox_creds(
    policy: &SandboxPolicy,
    policy_cwd: &Path,
    command_cwd: &Path,
    env_map: &HashMap<String, String>,
    codex_home: &Path,
) -> Result<SandboxCreds> {
    let mut identity = select_identity(policy, codex_home)?;
    if identity.is_none() {
        run_elevated_setup(policy, policy_cwd, command_cwd, env_map, codex_home)?;
        identity = select_identity(policy, codex_home)?;
    }
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
