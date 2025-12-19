use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::collections::HashSet;
use std::ffi::c_void;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;

use crate::allow::compute_allow_paths;
use crate::allow::AllowDenyPaths;
use crate::logging::log_note;
use crate::policy::SandboxPolicy;
use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;

use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Security::AllocateAndInitializeSid;
use windows_sys::Win32::Security::CheckTokenMembership;
use windows_sys::Win32::Security::FreeSid;
use windows_sys::Win32::Security::SECURITY_NT_AUTHORITY;

pub const SETUP_VERSION: u32 = 2;
pub const OFFLINE_USERNAME: &str = "CodexSandboxOffline";
pub const ONLINE_USERNAME: &str = "CodexSandboxOnline";
const SECURITY_BUILTIN_DOMAIN_RID: u32 = 0x0000_0020;
const DOMAIN_ALIAS_RID_ADMINS: u32 = 0x0000_0220;

pub fn sandbox_dir(codex_home: &Path) -> PathBuf {
    codex_home.join(".sandbox")
}

pub fn setup_marker_path(codex_home: &Path) -> PathBuf {
    sandbox_dir(codex_home).join("setup_marker.json")
}

pub fn sandbox_users_path(codex_home: &Path) -> PathBuf {
    sandbox_dir(codex_home).join("sandbox_users.json")
}

pub fn run_setup_refresh(
    policy: &SandboxPolicy,
    policy_cwd: &Path,
    command_cwd: &Path,
    env_map: &HashMap<String, String>,
    codex_home: &Path,
) -> Result<()> {
    // Skip in danger-full-access.
    if matches!(
        policy,
        SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. }
    ) {
        return Ok(());
    }
    let (read_roots, write_roots) = build_payload_roots(
        policy,
        policy_cwd,
        command_cwd,
        env_map,
        codex_home,
        None,
        None,
    );
    let payload = ElevationPayload {
        version: SETUP_VERSION,
        offline_username: OFFLINE_USERNAME.to_string(),
        online_username: ONLINE_USERNAME.to_string(),
        codex_home: codex_home.to_path_buf(),
        read_roots,
        write_roots,
        real_user: std::env::var("USERNAME").unwrap_or_else(|_| "Administrators".to_string()),
        refresh_only: true,
    };
    let json = serde_json::to_vec(&payload)?;
    let b64 = BASE64_STANDARD.encode(json);
    let exe = find_setup_exe();
    log_note(
        &format!("setup refresh: invoking {}", exe.display()),
        Some(&sandbox_dir(codex_home)),
    );
    // Refresh should never request elevation; ensure verb isn't set and we don't trigger UAC.
    let mut cmd = Command::new(&exe);
    cmd.arg(&b64).stdout(Stdio::null()).stderr(Stdio::null());
    let cwd = std::env::current_dir().unwrap_or_else(|_| codex_home.to_path_buf());
    log_note(
        &format!(
            "setup refresh: spawning {} (cwd={}, payload_len={})",
            exe.display(),
            cwd.display(),
            b64.len()
        ),
        Some(&sandbox_dir(codex_home)),
    );
    let status = cmd
        .status()
        .map_err(|e| {
            log_note(
                &format!("setup refresh: failed to spawn {}: {e}", exe.display()),
                Some(&sandbox_dir(codex_home)),
            );
            e
        })
        .context("spawn setup refresh")?;
    if !status.success() {
        log_note(
            &format!("setup refresh: exited with status {status:?}"),
            Some(&sandbox_dir(codex_home)),
        );
        return Err(anyhow!("setup refresh failed with status {}", status));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SetupMarker {
    pub version: u32,
    pub offline_username: String,
    pub online_username: String,
    #[serde(default)]
    pub created_at: Option<String>,
}

impl SetupMarker {
    pub fn version_matches(&self) -> bool {
        self.version == SETUP_VERSION
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxUserRecord {
    pub username: String,
    /// DPAPI-encrypted password blob, base64 encoded.
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxUsersFile {
    pub version: u32,
    pub offline: SandboxUserRecord,
    pub online: SandboxUserRecord,
}

impl SandboxUsersFile {
    pub fn version_matches(&self) -> bool {
        self.version == SETUP_VERSION
    }
}

fn is_elevated() -> Result<bool> {
    unsafe {
        let mut administrators_group: *mut c_void = std::ptr::null_mut();
        let ok = AllocateAndInitializeSid(
            &SECURITY_NT_AUTHORITY,
            2,
            SECURITY_BUILTIN_DOMAIN_RID,
            DOMAIN_ALIAS_RID_ADMINS,
            0,
            0,
            0,
            0,
            0,
            0,
            &mut administrators_group,
        );
        if ok == 0 {
            return Err(anyhow!(
                "AllocateAndInitializeSid failed: {}",
                GetLastError()
            ));
        }
        let mut is_member = 0i32;
        let check = CheckTokenMembership(0, administrators_group, &mut is_member as *mut _);
        FreeSid(administrators_group as *mut _);
        if check == 0 {
            return Err(anyhow!("CheckTokenMembership failed: {}", GetLastError()));
        }
        Ok(is_member != 0)
    }
}

fn canonical_existing(paths: &[PathBuf]) -> Vec<PathBuf> {
    paths
        .iter()
        .filter_map(|p| {
            if !p.exists() {
                return None;
            }
            Some(dunce::canonicalize(p).unwrap_or_else(|_| p.clone()))
        })
        .collect()
}

pub(crate) fn gather_read_roots(command_cwd: &Path, policy: &SandboxPolicy) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            roots.push(dir.to_path_buf());
        }
    }
    for p in [
        PathBuf::from(r"C:\Windows"),
        PathBuf::from(r"C:\Program Files"),
        PathBuf::from(r"C:\Program Files (x86)"),
        PathBuf::from(r"C:\ProgramData"),
    ] {
        roots.push(p);
    }
    if let Ok(up) = std::env::var("USERPROFILE") {
        roots.push(PathBuf::from(up));
    }
    roots.push(command_cwd.to_path_buf());
    if let SandboxPolicy::WorkspaceWrite { writable_roots, .. } = policy {
        for root in writable_roots {
            roots.push(root.to_path_buf());
        }
    }
    canonical_existing(&roots)
}

pub(crate) fn gather_write_roots(
    policy: &SandboxPolicy,
    policy_cwd: &Path,
    command_cwd: &Path,
    env_map: &HashMap<String, String>,
) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    // Always include the command CWD for workspace-write.
    if matches!(policy, SandboxPolicy::WorkspaceWrite { .. }) {
        roots.push(command_cwd.to_path_buf());
    }
    let AllowDenyPaths { allow, .. } =
        compute_allow_paths(policy, policy_cwd, command_cwd, env_map);
    roots.extend(allow);
    let mut dedup: HashSet<PathBuf> = HashSet::new();
    let mut out: Vec<PathBuf> = Vec::new();
    for r in canonical_existing(&roots) {
        if dedup.insert(r.clone()) {
            out.push(r);
        }
    }
    out
}

#[derive(Serialize)]
struct ElevationPayload {
    version: u32,
    offline_username: String,
    online_username: String,
    codex_home: PathBuf,
    read_roots: Vec<PathBuf>,
    write_roots: Vec<PathBuf>,
    real_user: String,
    #[serde(default)]
    refresh_only: bool,
}

fn quote_arg(arg: &str) -> String {
    let needs = arg.is_empty()
        || arg
            .chars()
            .any(|c| matches!(c, ' ' | '\t' | '\n' | '\r' | '"'));
    if !needs {
        return arg.to_string();
    }
    let mut out = String::from("\"");
    let mut bs = 0;
    for ch in arg.chars() {
        match ch {
            '\\' => {
                bs += 1;
            }
            '"' => {
                out.push_str(&"\\".repeat(bs * 2 + 1));
                out.push('"');
                bs = 0;
            }
            _ => {
                if bs > 0 {
                    out.push_str(&"\\".repeat(bs));
                    bs = 0;
                }
                out.push(ch);
            }
        }
    }
    if bs > 0 {
        out.push_str(&"\\".repeat(bs * 2));
    }
    out.push('"');
    out
}

fn find_setup_exe() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("codex-windows-sandbox-setup.exe");
            if candidate.exists() {
                return candidate;
            }
        }
    }
    PathBuf::from("codex-windows-sandbox-setup.exe")
}

fn run_setup_exe(payload: &ElevationPayload, needs_elevation: bool) -> Result<()> {
    use windows_sys::Win32::System::Threading::GetExitCodeProcess;
    use windows_sys::Win32::System::Threading::WaitForSingleObject;
    use windows_sys::Win32::System::Threading::INFINITE;
    use windows_sys::Win32::UI::Shell::ShellExecuteExW;
    use windows_sys::Win32::UI::Shell::SEE_MASK_NOCLOSEPROCESS;
    use windows_sys::Win32::UI::Shell::SHELLEXECUTEINFOW;
    let exe = find_setup_exe();
    let payload_json = serde_json::to_string(payload)?;
    let payload_b64 = BASE64_STANDARD.encode(payload_json.as_bytes());

    if !needs_elevation {
        let status = Command::new(&exe)
            .arg(&payload_b64)
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .context("failed to launch setup helper")?;
        if !status.success() {
            return Err(anyhow!(
                "setup helper exited with status {:?}",
                status.code()
            ));
        }
        return Ok(());
    }

    let exe_w = crate::winutil::to_wide(&exe);
    let params = quote_arg(&payload_b64);
    let params_w = crate::winutil::to_wide(params);
    let verb_w = crate::winutil::to_wide("runas");
    let mut sei: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    sei.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    sei.fMask = SEE_MASK_NOCLOSEPROCESS;
    sei.lpVerb = verb_w.as_ptr();
    sei.lpFile = exe_w.as_ptr();
    sei.lpParameters = params_w.as_ptr();
    // Hide the window for the elevated helper.
    sei.nShow = 0; // SW_HIDE
    let ok = unsafe { ShellExecuteExW(&mut sei) };
    if ok == 0 || sei.hProcess == 0 {
        return Err(anyhow!(
            "ShellExecuteExW failed to launch setup helper: {}",
            unsafe { GetLastError() }
        ));
    }
    unsafe {
        WaitForSingleObject(sei.hProcess, INFINITE);
        let mut code: u32 = 1;
        GetExitCodeProcess(sei.hProcess, &mut code);
        CloseHandle(sei.hProcess);
        if code != 0 {
            return Err(anyhow!("setup helper exited with status {}", code));
        }
    }
    Ok(())
}

pub fn run_elevated_setup(
    policy: &SandboxPolicy,
    policy_cwd: &Path,
    command_cwd: &Path,
    env_map: &HashMap<String, String>,
    codex_home: &Path,
    read_roots_override: Option<Vec<PathBuf>>,
    write_roots_override: Option<Vec<PathBuf>>,
) -> Result<()> {
    // Ensure the shared sandbox directory exists before we send it to the elevated helper.
    let sbx_dir = sandbox_dir(codex_home);
    std::fs::create_dir_all(&sbx_dir)?;
    let (read_roots, write_roots) = build_payload_roots(
        policy,
        policy_cwd,
        command_cwd,
        env_map,
        codex_home,
        read_roots_override,
        write_roots_override,
    );
    let payload = ElevationPayload {
        version: SETUP_VERSION,
        offline_username: OFFLINE_USERNAME.to_string(),
        online_username: ONLINE_USERNAME.to_string(),
        codex_home: codex_home.to_path_buf(),
        read_roots,
        write_roots,
        real_user: std::env::var("USERNAME").unwrap_or_else(|_| "Administrators".to_string()),
        refresh_only: false,
    };
    let needs_elevation = !is_elevated()?;
    run_setup_exe(&payload, needs_elevation)
}

fn build_payload_roots(
    policy: &SandboxPolicy,
    policy_cwd: &Path,
    command_cwd: &Path,
    env_map: &HashMap<String, String>,
    codex_home: &Path,
    read_roots_override: Option<Vec<PathBuf>>,
    write_roots_override: Option<Vec<PathBuf>>,
) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let sbx_dir = sandbox_dir(codex_home);
    let mut write_roots = if let Some(roots) = write_roots_override {
        canonical_existing(&roots)
    } else {
        gather_write_roots(policy, policy_cwd, command_cwd, env_map)
    };
    if !write_roots.contains(&sbx_dir) {
        write_roots.push(sbx_dir.clone());
    }
    let mut read_roots = if let Some(roots) = read_roots_override {
        canonical_existing(&roots)
    } else {
        gather_read_roots(command_cwd, policy)
    };
    let write_root_set: HashSet<PathBuf> = write_roots.iter().cloned().collect();
    read_roots.retain(|root| !write_root_set.contains(root));
    (read_roots, write_roots)
}
