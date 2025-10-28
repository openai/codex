mod acl;
mod allow;
mod audit;
mod cap;
mod env;
mod logging;
mod policy;
mod token;
mod winutil;

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
use crate::token::convert_string_sid_to_sid;
use crate::winutil::format_last_error;
use crate::winutil::to_wide;
use anyhow::Result;
use std::collections::HashMap;
use std::ffi::c_void;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::ptr;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Foundation::SetHandleInformation;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Foundation::HANDLE_FLAG_INHERIT;
use windows_sys::Win32::System::Pipes::CreatePipe;
use windows_sys::Win32::System::Threading::CreateProcessAsUserW;
use windows_sys::Win32::System::Threading::GetExitCodeProcess;
use windows_sys::Win32::System::Threading::WaitForSingleObject;
use windows_sys::Win32::System::Threading::CREATE_UNICODE_ENVIRONMENT;
use windows_sys::Win32::System::Threading::INFINITE;
use windows_sys::Win32::System::Threading::PROCESS_INFORMATION;
use windows_sys::Win32::System::Threading::STARTF_USESTDHANDLES;
use windows_sys::Win32::System::Threading::STARTUPINFOW;

fn ensure_dir(p: &Path) -> Result<()> {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d)?;
    }
    Ok(())
}

fn make_env_block(env: &HashMap<String, String>) -> Vec<u16> {
    let mut items: Vec<(String, String)> =
        env.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    items.sort_by(|a, b| {
        a.0.to_uppercase()
            .cmp(&b.0.to_uppercase())
            .then(a.0.cmp(&b.0))
    });
    let mut w: Vec<u16> = Vec::new();
    for (k, v) in items {
        let mut s = to_wide(format!("{}={}", k, v));
        s.pop();
        w.extend_from_slice(&s);
        w.push(0);
    }
    w.push(0);
    w
}

// Quote a single Windows command-line argument following the rules used by
// CommandLineToArgvW/CRT so that spaces, quotes, and backslashes are preserved.
// Reference behavior matches Rust std::process::Command on Windows.
fn quote_windows_arg(arg: &str) -> String {
    // Empty or contains special characters: must quote
    let needs_quotes = arg.is_empty()
        || arg
            .chars()
            .any(|c| matches!(c, ' ' | '\t' | '\n' | '\r' | '"'));
    if !needs_quotes {
        return arg.to_string();
    }

    let mut quoted = String::with_capacity(arg.len() + 2);
    quoted.push('"');
    let mut backslashes = 0;
    for ch in arg.chars() {
        match ch {
            '\\' => {
                backslashes += 1;
            }
            '"' => {
                // Escape all backslashes before a quote
                quoted.extend(std::iter::repeat('\\').take(backslashes * 2 + 1));
                quoted.push('"');
                backslashes = 0;
            }
            _ => {
                if backslashes > 0 {
                    quoted.extend(std::iter::repeat('\\').take(backslashes));
                    backslashes = 0;
                }
                quoted.push(ch);
            }
        }
    }
    // Escape trailing backslashes
    if backslashes > 0 {
        quoted.extend(std::iter::repeat('\\').take(backslashes * 2));
    }
    quoted.push('"');
    quoted
}

unsafe fn setup_stdio_pipes() -> io::Result<((HANDLE, HANDLE), (HANDLE, HANDLE), (HANDLE, HANDLE))>
{
    // Returns (stdin_read, stdin_write), (stdout_read, stdout_write), (stderr_read, stderr_write)
    let mut in_r: HANDLE = 0;
    let mut in_w: HANDLE = 0;
    let mut out_r: HANDLE = 0;
    let mut out_w: HANDLE = 0;
    let mut err_r: HANDLE = 0;
    let mut err_w: HANDLE = 0;
    if CreatePipe(&mut in_r, &mut in_w, ptr::null_mut(), 0) == 0 {
        return Err(io::Error::from_raw_os_error(GetLastError() as i32));
    }
    if CreatePipe(&mut out_r, &mut out_w, ptr::null_mut(), 0) == 0 {
        return Err(io::Error::from_raw_os_error(GetLastError() as i32));
    }
    if CreatePipe(&mut err_r, &mut err_w, ptr::null_mut(), 0) == 0 {
        return Err(io::Error::from_raw_os_error(GetLastError() as i32));
    }
    // Make child ends (in_r for stdin, out_w for stdout, err_w for stderr) inheritable
    if SetHandleInformation(in_r, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT) == 0 {
        return Err(io::Error::from_raw_os_error(GetLastError() as i32));
    }
    if SetHandleInformation(out_w, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT) == 0 {
        return Err(io::Error::from_raw_os_error(GetLastError() as i32));
    }
    if SetHandleInformation(err_w, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT) == 0 {
        return Err(io::Error::from_raw_os_error(GetLastError() as i32));
    }
    Ok(((in_r, in_w), (out_r, out_w), (err_r, err_w)))
}

pub struct CaptureResult {
    pub exit_code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub timed_out: bool,
}

// Expose a reusable preflight audit entry so other codepaths can trigger it
pub fn preflight_audit_everyone_writable(
    cwd: &Path,
    env_map: &HashMap<String, String>,
) -> Result<()> {
    audit::audit_everyone_writable(cwd, env_map)
}

pub fn run_windows_sandbox_capture(
    policy_json_or_preset: &str,
    sandbox_policy_cwd: &Path,
    command: Vec<String>,
    cwd: &Path,
    mut env_map: HashMap<String, String>,
    timeout_ms: Option<u64>,
) -> Result<CaptureResult> {
    let policy = SandboxPolicy::parse(policy_json_or_preset)?;
    normalize_null_device_env(&mut env_map);
    ensure_non_interactive_pager(&mut env_map);
    apply_no_network_to_env(&mut env_map)?;

    let current_dir = cwd.to_path_buf();
    // Preflight: audit common directories for Everyone-writeable risk and fail-closed.
    // This is intentionally callable independently so other codepaths can trigger it.
    audit::audit_everyone_writable(&current_dir, &env_map)?;
    // Log start consistently with the CLI binary
    log_start(&command);
    let (h_token, psid_to_use): (HANDLE, *mut c_void) = unsafe {
        match &policy.0 {
            SandboxMode::ReadOnly => {
                let caps = load_or_create_cap_sids(sandbox_policy_cwd);
                ensure_dir(&cap_sid_file(sandbox_policy_cwd))?;
                fs::write(
                    cap_sid_file(sandbox_policy_cwd),
                    serde_json::to_string(&caps)?,
                )?;
                let psid = convert_string_sid_to_sid(&caps.readonly).unwrap();
                token::create_readonly_token_with_cap(psid)?
            }
            SandboxMode::WorkspaceWrite => {
                let caps = load_or_create_cap_sids(sandbox_policy_cwd);
                ensure_dir(&cap_sid_file(sandbox_policy_cwd))?;
                fs::write(
                    cap_sid_file(sandbox_policy_cwd),
                    serde_json::to_string(&caps)?,
                )?;
                let psid = convert_string_sid_to_sid(&caps.workspace).unwrap();
                token::create_workspace_write_token_with_cap(psid)?
            }
        }
    };

    // Configure write/exec ACLs using shared computation
    let persist = matches!(policy.0, SandboxMode::WorkspaceWrite);
    let mut allow_paths = compute_allow_paths(&policy, sandbox_policy_cwd, &current_dir, &env_map);

    // Create-time: locate apply_patch(.bat/.cmd) shim inserted by arg0 into PATH (Temp dir)
    // and include it (and its parent) in allow paths so the capability SID can execute it.
    fn find_apply_patch_in_path(env_map: &HashMap<String, String>) -> Option<PathBuf> {
        let path_val = env_map.get("PATH").cloned().unwrap_or_default();
        if path_val.is_empty() {
            return None;
        }
        let entries: Vec<String> = path_val.split(';').map(|s| s.trim().to_string()).collect();
        let candidates = [
            "apply_patch.bat",
            "applypatch.bat",
            "apply_patch.cmd",
            "applypatch.cmd",
        ];
        for dir in entries {
            if dir.is_empty() {
                continue;
            }
            let base = PathBuf::from(&dir);
            for name in &candidates {
                let cand = base.join(name);
                if cand.exists() {
                    return Some(cand);
                }
            }
        }
        None
    }

    if let Some(shim) = find_apply_patch_in_path(&env_map) {
        /*logging::debug_log(&format!(
            "apply_patch shim detected in PATH; granting allow ACEs: {}",
            shim.display()
        ));*/
        allow_paths.push(shim.clone());
        if let Some(parent) = shim.parent() {
            allow_paths.push(parent.to_path_buf());
        }
    }
    unsafe {
        for p in &allow_paths {
            let _ = add_allow_ace(p, psid_to_use);
        }
        allow_null_device(psid_to_use);
    }

    // Set up pipes and spawn
    let ((in_r, in_w), (out_r, out_w), (err_r, err_w)) = unsafe { setup_stdio_pipes()? };
    // Close our stdin write end immediately (no input)
    unsafe {
        CloseHandle(in_w);
    }
    let mut pi: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
    let mut si: STARTUPINFOW = unsafe { std::mem::zeroed() };
    si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
    si.dwFlags |= STARTF_USESTDHANDLES;
    si.hStdInput = in_r;
    si.hStdOutput = out_w;
    si.hStdError = err_w;
    // Ensure a valid interactive desktop when launching with a restricted token
    let desktop = to_wide("Winsta0\\Default");
    si.lpDesktop = desktop.as_ptr() as *mut u16;

    // Build a properly quoted Windows command line so the patch argument is not split.
    let cmdline_str = command
        .iter()
        .map(|s| quote_windows_arg(s))
        .collect::<Vec<_>>()
        .join(" ");
    let mut cmdline: Vec<u16> = to_wide(&cmdline_str);
    let env_block = make_env_block(&env_map);

    // Pre-spawn Temp grant removed; create-time PATH scan handles apply_patch shim.
    let ok = unsafe {
        CreateProcessAsUserW(
            h_token,
            std::ptr::null(),
            cmdline.as_mut_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            1,
            CREATE_UNICODE_ENVIRONMENT,
            env_block.as_ptr() as *mut c_void,
            to_wide(&current_dir).as_ptr(),
            &mut si,
            &mut pi,
        )
    };
    if ok == 0 {
        // Spawn failed: log and clean up
        let err = unsafe { GetLastError() } as i32;
        log_failure(&command, &format!("CreateProcessAsUserW failed: {}", err));
        let dbg = format!(
            "CreateProcessAsUserW failed: {} ({}) | cwd={} | cmd={} | env_u16_len={} | si_flags={}",
            err,
            format_last_error(err),
            current_dir.display(),
            cmdline_str,
            env_block.len(),
            si.dwFlags,
        );
        logging::debug_log(&dbg);
        unsafe {
            CloseHandle(in_r);
            CloseHandle(out_r);
            CloseHandle(out_w);
            CloseHandle(err_r);
            CloseHandle(err_w);
            CloseHandle(h_token);
        }
        return Err(anyhow::anyhow!("CreateProcessAsUserW failed: {}", err));
    }
    // Parent: close child-ends we don't need
    unsafe {
        CloseHandle(in_r);
        CloseHandle(out_w);
        CloseHandle(err_w);
    }

    // Read stdout/stderr concurrently in threads to avoid deadlocks
    let (tx_out, rx_out) = std::sync::mpsc::channel::<Vec<u8>>();
    let (tx_err, rx_err) = std::sync::mpsc::channel::<Vec<u8>>();
    let t_out = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let mut tmp = [0u8; 8192];
        loop {
            let mut read_bytes: u32 = 0;
            let ok = unsafe {
                windows_sys::Win32::Storage::FileSystem::ReadFile(
                    out_r,
                    tmp.as_mut_ptr(),
                    tmp.len() as u32,
                    &mut read_bytes,
                    std::ptr::null_mut(),
                )
            };
            if ok == 0 || read_bytes == 0 {
                break;
            }
            buf.extend_from_slice(&tmp[..read_bytes as usize]);
        }
        let _ = tx_out.send(buf);
    });
    let t_err = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let mut tmp = [0u8; 8192];
        loop {
            let mut read_bytes: u32 = 0;
            let ok = unsafe {
                windows_sys::Win32::Storage::FileSystem::ReadFile(
                    err_r,
                    tmp.as_mut_ptr(),
                    tmp.len() as u32,
                    &mut read_bytes,
                    std::ptr::null_mut(),
                )
            };
            if ok == 0 || read_bytes == 0 {
                break;
            }
            buf.extend_from_slice(&tmp[..read_bytes as usize]);
        }
        let _ = tx_err.send(buf);
    });

    // Wait with timeout
    let timeout = timeout_ms.map(|ms| ms as u32).unwrap_or(INFINITE);
    let res = unsafe { WaitForSingleObject(pi.hProcess, timeout) };
    let timed_out = res == 0x00000102; // WAIT_TIMEOUT
    let mut exit_code_u32: u32 = 1;
    if !timed_out {
        unsafe {
            GetExitCodeProcess(pi.hProcess, &mut exit_code_u32);
        }
    } else {
        unsafe {
            windows_sys::Win32::System::Threading::TerminateProcess(pi.hProcess, 1);
        }
    }
    // Close process handles
    unsafe {
        if pi.hThread != 0 {
            CloseHandle(pi.hThread);
        }
        if pi.hProcess != 0 {
            CloseHandle(pi.hProcess);
        }
        CloseHandle(h_token);
    }
    let _ = t_out.join();
    let _ = t_err.join();
    let stdout = rx_out.recv().unwrap_or_default();
    let stderr = rx_err.recv().unwrap_or_default();
    let exit_code = if timed_out {
        128 + 64
    } else {
        exit_code_u32 as i32
    };

    // Log completion
    if exit_code == 0 {
        log_success(&command);
    } else {
        log_failure(&command, &format!("exit code {}", exit_code));
    }

    // Best-effort cleanup for non-persistent ACLs (revoke what we added)
    if !persist {
        unsafe {
            for p in &allow_paths {
                revoke_ace(p, psid_to_use);
            }
        }
    }

    Ok(CaptureResult {
        exit_code,
        stdout,
        stderr,
        timed_out,
    })
}
