#![cfg(target_os = "windows")]

use anyhow::Context;
use anyhow::Result;
use codex_protocol::protocol::SandboxPolicy;
use codex_windows_sandbox::run_elevated_setup_with_identity_overrides;
use codex_windows_sandbox::sandbox_setup_is_complete;
use codex_windows_sandbox::SetupIdentityOverrides;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

struct CleanupGuard {
    usernames: Vec<String>,
    firewall_rule_name: String,
}

impl Drop for CleanupGuard {
    fn drop(&mut self) {
        cleanup_global_sandbox_artifacts(&self.usernames, &self.firewall_rule_name);
    }
}

#[derive(Debug, Deserialize)]
struct SandboxUsersFile {
    offline: SandboxUserRecord,
    online: SandboxUserRecord,
}

#[derive(Debug, Deserialize)]
struct SandboxUserRecord {
    username: String,
}

#[test]
fn elevated_setup_creates_both_sandbox_users() -> Result<()> {
    let suffix = unique_suffix();
    let offline_username = format!("CdxSbxOf{suffix}");
    let online_username = format!("CdxSbxOn{suffix}");
    let firewall_rule_name = format!("codex_sbx_test_{suffix}");

    let pre_users = vec![offline_username.clone(), online_username.clone()];
    cleanup_global_sandbox_artifacts(&pre_users, &firewall_rule_name);

    let mut guard = CleanupGuard {
        usernames: Vec::new(),
        firewall_rule_name: firewall_rule_name.clone(),
    };

    ensure_setup_helper_on_path()?;

    let temp = tempfile::tempdir().context("create temp dir")?;
    let codex_home = temp.path().join("codex-home");
    let workspace = temp.path().join("workspace");
    std::fs::create_dir_all(&codex_home).context("create codex_home")?;
    std::fs::create_dir_all(&workspace).context("create workspace")?;

    let policy = SandboxPolicy::ReadOnly;
    let env_map = HashMap::new();
    let overrides = SetupIdentityOverrides {
        offline_username: Some(offline_username),
        online_username: Some(online_username),
        offline_block_rule_name: Some(firewall_rule_name),
    };
    run_elevated_setup_with_identity_overrides(
        &policy,
        workspace.as_path(),
        workspace.as_path(),
        &env_map,
        codex_home.as_path(),
        None,
        None,
        Some(&overrides),
    )?;

    assert!(
        sandbox_setup_is_complete(codex_home.as_path()),
        "sandbox setup should be complete after elevated setup runs"
    );

    let users = read_sandbox_users_file(codex_home.as_path())?;
    guard.usernames = vec![users.offline.username.clone(), users.online.username.clone()];
    assert_local_user_exists(&users.offline.username)?;
    assert_local_user_exists(&users.online.username)?;

    Ok(())
}

fn read_sandbox_users_file(codex_home: &Path) -> Result<SandboxUsersFile> {
    let path = codex_home
        .join(".sandbox-secrets")
        .join("sandbox_users.json");
    let bytes = std::fs::read(&path).with_context(|| format!("read {}", path.display()))?;
    let users: SandboxUsersFile = serde_json::from_slice(&bytes)
        .with_context(|| format!("parse {}", path.display()))?;
    Ok(users)
}

fn assert_local_user_exists(username: &str) -> Result<()> {
    let status = Command::new("net")
        .args(["user", username])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("query local user {username}"))?;
    if status.success() {
        return Ok(());
    }
    anyhow::bail!("expected local user {username} to exist")
}

fn ensure_setup_helper_on_path() -> Result<()> {
    if let Some(helper) = helper_from_env() {
        let helper_dir = Path::new(&helper)
            .parent()
            .context("setup helper path has no parent dir")?;
        prepend_to_path(helper_dir)?;
        return Ok(());
    }

    if let Some(helper_dir) = helper_dir_from_target_layout() {
        prepend_to_path(&helper_dir)?;
        return Ok(());
    }

    anyhow::bail!(
        "setup helper path was not found via CARGO_BIN_EXE_* or target/debug layout"
    )
}

fn helper_from_env() -> Option<String> {
    for key in [
        "CARGO_BIN_EXE_codex-windows-sandbox-setup",
        "CARGO_BIN_EXE_codex_windows_sandbox_setup",
    ] {
        if let Ok(path) = std::env::var(key) {
            return Some(path);
        }
    }
    None
}

fn helper_dir_from_target_layout() -> Option<PathBuf> {
    let current_exe = std::env::current_exe().ok()?;
    let deps_dir = current_exe.parent()?;
    let target_debug_dir = deps_dir.parent()?;
    let helper = target_debug_dir.join("codex-windows-sandbox-setup.exe");
    if helper.is_file() {
        return Some(target_debug_dir.to_path_buf());
    }
    None
}

fn prepend_to_path(path: &Path) -> Result<()> {
    let existing = std::env::var("PATH").unwrap_or_default();
    let mut parts = vec![path.display().to_string()];
    if !existing.is_empty() {
        parts.push(existing);
    }
    let joined = parts.join(";");
    std::env::set_var("PATH", joined);
    Ok(())
}

fn cleanup_global_sandbox_artifacts(usernames: &[String], firewall_rule_name: &str) {
    for user in usernames {
        let _ = Command::new("net")
            .args(["user", user, "/delete"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let cmd = format!(
        "Remove-NetFirewallRule -Name '{firewall_rule_name}' -ErrorAction SilentlyContinue"
    );
    let _ = Command::new("powershell")
        .args(["-NoLogo", "-NoProfile", "-Command", &cmd])
        .status();
}

fn unique_suffix() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id() as u128;
    format!("{:08x}", ((nanos ^ pid) & 0xffff_ffff) as u32)
}
