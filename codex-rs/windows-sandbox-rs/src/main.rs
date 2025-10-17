mod acl;
mod cap;
mod env;
mod policy;
mod process;
mod token;
mod winutil;

use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use std::collections::HashMap;
use std::ffi::c_void;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::Foundation::HANDLE;

use crate::acl::add_allow_ace;
use crate::acl::allow_null_device;
use crate::acl::revoke_ace;
use crate::cap::cap_sid_file;
use crate::cap::load_or_create_cap_sids;
use crate::env::apply_no_network_to_env;
use crate::env::ensure_non_interactive_pager;
use crate::env::normalize_null_device_env;
use crate::policy::SandboxMode;
use crate::policy::SandboxPolicy;
use crate::process::assign_to_job;
use crate::process::create_job_kill_on_close;
use crate::process::create_process_as_user;
use crate::process::wait_process_and_exitcode;
use crate::token::convert_string_sid_to_sid;
use crate::token::create_readonly_token_with_cap;
use crate::token::create_workspace_write_token_with_cap;
use crate::token::get_current_token_for_restriction;
use crate::token::get_logon_sid_bytes;
use std::io::Write;

fn ensure_dir(p: &Path) -> Result<()> {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d)?;
    }
    Ok(())
}

fn compute_allow_paths(
    policy: &SandboxPolicy,
    policy_cwd: &Path,
    command_cwd: &Path,
    env_map: &HashMap<String, String>,
) -> Vec<PathBuf> {
    let mut allow: Vec<PathBuf> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    // Add declared roots
    if let SandboxMode::WorkspaceWrite { .. } = &policy.0 {
        for w in policy.writable_roots_with_cwd(policy_cwd) {
            let abs = if w.is_absolute() {
                w
            } else {
                command_cwd.join(w)
            };
            if seen.insert(abs.to_string_lossy().to_string()) && abs.exists() {
                allow.push(abs);
            }
        }
    }
    // Ensure cwd if not already covered
    if let SandboxMode::WorkspaceWrite { .. } = &policy.0 {
        let covered = allow.iter().any(|x| command_cwd.starts_with(x));
        if !covered {
            let abs = command_cwd.to_path_buf();
            if seen.insert(abs.to_string_lossy().to_string()) && abs.exists() {
                allow.push(abs);
            }
        }
    }
    // TEMP/TMP
    if !matches!(policy.0, SandboxMode::ReadOnly) {
        for key in ["TEMP", "TMP"] {
            if let Some(v) = env_map.get(key) {
                let abs = PathBuf::from(v);
                if seen.insert(abs.to_string_lossy().to_string()) && abs.exists() {
                    allow.push(abs);
                }
            } else if let Ok(v) = std::env::var(key) {
                let abs = PathBuf::from(v);
                if seen.insert(abs.to_string_lossy().to_string()) && abs.exists() {
                    allow.push(abs);
                }
            }
        }
    }
    allow
}

fn main() -> Result<()> {
    if cfg!(not(windows)) {
        eprintln!("codex-windows-sandbox is only supported on Windows");
        std::process::exit(2);
    }
    let mut args: Vec<String> = std::env::args().collect();
    let mut policy_cwd: Option<PathBuf> = None;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--sandbox-policy-cwd" && i + 1 < args.len() {
            policy_cwd = Some(PathBuf::from(&args[i + 1]));
            args.drain(i..=i + 1);
        } else {
            break;
        }
    }
    if args.len() < 2 {
        eprintln!("No policy specified.");
        std::process::exit(2);
    }
    let policy_str = args[1].clone();
    let policy = SandboxPolicy::parse(&policy_str)?;
    let mut cmd_index = 2;
    if cmd_index < args.len() && args[cmd_index] == "--" {
        cmd_index += 1;
    }
    let command: Vec<String> = args[cmd_index..].to_vec();
    if command.is_empty() {
        eprintln!("No command specified to execute.");
        std::process::exit(2);
    }

    let current_dir = std::env::current_dir().context("failed to get current dir")?;
    let policy_cwd = policy_cwd.unwrap_or_else(|| current_dir.clone());

    let mut env_map: HashMap<String, String> = std::env::vars().collect();
    normalize_null_device_env(&mut env_map);
    ensure_non_interactive_pager(&mut env_map);
    apply_no_network_to_env(&mut env_map)?;

    let (h_token, psid_to_use): (HANDLE, *mut c_void) = unsafe {
        match &policy.0 {
            SandboxMode::ReadOnly => {
                let caps = load_or_create_cap_sids(&policy_cwd);
                ensure_dir(&cap_sid_file(&policy_cwd))?;
                fs::write(cap_sid_file(&policy_cwd), serde_json::to_string(&caps)?)?;
                let psid = convert_string_sid_to_sid(&caps.readonly).unwrap();
                create_readonly_token_with_cap(psid)?
            }
            SandboxMode::WorkspaceWrite { .. } => {
                let caps = load_or_create_cap_sids(&policy_cwd);
                ensure_dir(&cap_sid_file(&policy_cwd))?;
                fs::write(cap_sid_file(&policy_cwd), serde_json::to_string(&caps)?)?;
                let psid = convert_string_sid_to_sid(&caps.workspace).unwrap();
                create_workspace_write_token_with_cap(psid)?
            }
        }
    };

    // Diagnostics parity: allow NUL for current logon sid in WS mode
    unsafe {
        if matches!(policy.0, SandboxMode::WorkspaceWrite { .. }) {
            if let Ok(base) = get_current_token_for_restriction() {
                if let Ok(bytes) = get_logon_sid_bytes(base) {
                    let mut tmp = bytes.clone();
                    let psid2 = tmp.as_mut_ptr() as *mut c_void;
                    allow_null_device(psid2);
                }
                windows_sys::Win32::Foundation::CloseHandle(base);
            }
        }
    }

    // Configure ACLs
    let persist_aces = matches!(policy.0, SandboxMode::WorkspaceWrite { .. });
    let allow = compute_allow_paths(&policy, &policy_cwd, &current_dir, &env_map);
    let mut guards: Vec<(PathBuf, *mut c_void)> = Vec::new();
    unsafe {
        for p in &allow {
            if let Ok(added) = add_allow_ace(p, psid_to_use) {
                if added {
                    if persist_aces {
                        // seed recursively for existing items in workspace
                        if p.is_dir() {
                            // best-effort: recursive seed bounded (omitted here for brevity)
                        }
                    } else {
                        guards.push((p.clone(), psid_to_use));
                    }
                }
            }
        }
        allow_null_device(psid_to_use);
    }

    // Command logging (Rust): append START, then SUCCESS/FAILURE
    const LOG_COMMAND_PREVIEW_LIMIT: usize = 200;
    const LOG_FILE_NAME: &str = "sandbox_commands.rust.log";
    let preview = {
        let j = command.join(" ");
        if j.len() <= LOG_COMMAND_PREVIEW_LIMIT { j } else { j[..LOG_COMMAND_PREVIEW_LIMIT].to_string() }
    };
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(LOG_FILE_NAME) {
        let _ = writeln!(f, "START: {}", preview);
    }

    // Spawn
    let (pi, _si) = match unsafe { create_process_as_user(h_token, &command, &current_dir, &env_map) } {
        Ok(v) => v,
        Err(e) => {
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(LOG_FILE_NAME) {
                let _ = writeln!(f, "FAILURE: {} ({})", preview, format!("spawn failed: {}", e));
            }
            eprintln!("failed to spawn process: {}", e);
            return Err(e);
        }
    };

    let mut code: i32 = 1;
    unsafe {
        match create_job_kill_on_close() {
            Ok(h_job) => {
                let _ = assign_to_job(h_job, pi.hProcess);
                code = wait_process_and_exitcode(&pi)?;
                CloseHandle(h_job);
            }
            Err(_) => {
                code = wait_process_and_exitcode(&pi)?;
            }
        }
    }

    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(LOG_FILE_NAME) {
        if code == 0 { let _ = writeln!(f, "SUCCESS: {}", preview); }
        else { let _ = writeln!(f, "FAILURE: {} (exit code {})", preview, code); }
    }
    if code != 0 { eprintln!("sandboxed command failed with exit code {}: {}", code, command.join(" ")); }

    unsafe {
        if pi.hThread != 0 {
            CloseHandle(pi.hThread);
        }
        if pi.hProcess != 0 {
            CloseHandle(pi.hProcess);
        }
    }

    if !persist_aces {
        unsafe {
            for (p, sid) in guards {
                revoke_ace(&p, sid);
            }
        }
    }

    unsafe {
        CloseHandle(h_token);
    }
    std::process::exit(code);
}
