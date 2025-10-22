mod acl;
mod allow;
mod cap;
mod env;
mod logging;
mod policy;
mod process;
mod token;
mod winutil;

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
use crate::allow::compute_allow_paths;
use crate::cap::cap_sid_file;
use crate::cap::load_or_create_cap_sids;
use crate::env::apply_no_network_to_env;
use crate::env::ensure_non_interactive_pager;
use crate::env::normalize_null_device_env;
use crate::logging::log_failure;
use crate::logging::log_start;
use crate::logging::log_success;
use crate::policy::SandboxMode;
use crate::policy::SandboxPolicy;
use crate::process::assign_to_job;
use crate::process::create_job_kill_on_close;
use crate::process::create_process_as_user;
use crate::process::wait_process_and_exitcode;
use crate::token::convert_string_sid_to_sid;
use crate::token::create_readonly_token_with_cap;
use crate::token::create_workspace_write_token_with_cap;
use crate::token::create_write_restricted_token_compat;
use crate::token::get_current_token_for_restriction;
use crate::token::get_logon_sid_bytes;

fn ensure_dir(p: &Path) -> Result<()> {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d)?;
    }
    Ok(())
}

// allow::compute_allow_paths now provides the shared logic

fn main() -> Result<()> {
    if cfg!(not(windows)) {
        eprintln!("codex-windows-sandbox is only supported on Windows");
        std::process::exit(2);
    }
    let mut args: Vec<String> = std::env::args().collect();
    let mut policy_cwd: Option<PathBuf> = None;
    if args.len() > 2 && args[1] == "--sandbox-policy-cwd" {
        policy_cwd = Some(PathBuf::from(&args[2]));
        args.drain(1..=2);
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
            SandboxMode::WorkspaceWrite => {
                let caps = load_or_create_cap_sids(&policy_cwd);
                ensure_dir(&cap_sid_file(&policy_cwd))?;
                fs::write(cap_sid_file(&policy_cwd), serde_json::to_string(&caps)?)?;
                if std::env::var("SBX_USE_COMPAT").ok().as_deref() == Some("1") {
                    create_write_restricted_token_compat()?
                } else {
                    let psid = convert_string_sid_to_sid(&caps.workspace).unwrap();
                    create_workspace_write_token_with_cap(psid)?
                }
            }
        }
    };

    // Diagnostics parity: allow NUL for current logon sid in WS mode
    unsafe {
        if matches!(policy.0, SandboxMode::WorkspaceWrite) {
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
    let persist_aces = matches!(policy.0, SandboxMode::WorkspaceWrite);
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

    // Command logging (shared)
    log_start(&command);

    // Spawn
    let (pi, _si) =
        match unsafe { create_process_as_user(h_token, &command, &current_dir, &env_map) } {
            Ok(v) => v,
            Err(e) => {
                log_failure(&command, &format!("spawn failed: {}", e));
                eprintln!("failed to spawn process: {}", e);
                return Err(e);
            }
        };

    let code: i32 = unsafe {
        match create_job_kill_on_close() {
            Ok(h_job) => {
                let _ = assign_to_job(h_job, pi.hProcess);
                let c = wait_process_and_exitcode(&pi)?;
                CloseHandle(h_job);
                c
            }
            Err(_) => wait_process_and_exitcode(&pi)?,
        }
    };

    if code == 0 {
        log_success(&command);
    } else {
        log_failure(&command, &format!("exit code {}", code));
    }
    if code != 0 {
        eprintln!(
            "sandboxed command failed with exit code {}: {}",
            code,
            command.join(" ")
        );
    }

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
