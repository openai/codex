#![cfg(target_os = "windows")]

use anyhow::Context;
use anyhow::Result;
use codex_windows_sandbox::allow_null_device;
use codex_windows_sandbox::convert_string_sid_to_sid;
use codex_windows_sandbox::create_process_as_user;
use codex_windows_sandbox::create_readonly_token_with_caps_from;
use codex_windows_sandbox::create_workspace_write_token_with_caps_from;
use codex_windows_sandbox::get_current_token_for_restriction;
use codex_windows_sandbox::hide_current_user_profile_dir;
use codex_windows_sandbox::log_note;
use codex_windows_sandbox::parse_policy;
use codex_windows_sandbox::to_wide;
use codex_windows_sandbox::SandboxPolicy;
use serde::Deserialize;
use std::collections::HashMap;
use std::ffi::c_void;
use std::path::Path;
use std::path::PathBuf;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Foundation::HLOCAL;
use windows_sys::Win32::Storage::FileSystem::CreateFileW;
use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_READ;
use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_WRITE;
use windows_sys::Win32::Storage::FileSystem::OPEN_EXISTING;
use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
use windows_sys::Win32::System::JobObjects::CreateJobObjectW;
use windows_sys::Win32::System::JobObjects::JobObjectExtendedLimitInformation;
use windows_sys::Win32::System::JobObjects::SetInformationJobObject;
use windows_sys::Win32::System::JobObjects::JOBOBJECT_EXTENDED_LIMIT_INFORMATION;
use windows_sys::Win32::System::JobObjects::JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
use windows_sys::Win32::System::Threading::TerminateProcess;
use windows_sys::Win32::System::Threading::WaitForSingleObject;
use windows_sys::Win32::System::Threading::INFINITE;

#[path = "cwd_junction.rs"]
mod cwd_junction;

#[allow(dead_code)]
mod read_acl_mutex;

#[derive(Debug, Deserialize)]
struct RunnerRequest {
    policy_json_or_preset: String,
    // Writable location for logs (sandbox user's .codex).
    codex_home: PathBuf,
    // Real user's CODEX_HOME for shared data (caps, config).
    real_codex_home: PathBuf,
    cap_sids: Vec<String>,
    command: Vec<String>,
    cwd: PathBuf,
    env_map: HashMap<String, String>,
    timeout_ms: Option<u64>,
    stdin_pipe: String,
    stdout_pipe: String,
    stderr_pipe: String,
}

const WAIT_TIMEOUT: u32 = 0x0000_0102;

unsafe fn create_job_kill_on_close() -> Result<HANDLE> {
    let h = CreateJobObjectW(std::ptr::null_mut(), std::ptr::null());
    if h == 0 {
        return Err(anyhow::anyhow!("CreateJobObjectW failed"));
    }
    let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    let ok = SetInformationJobObject(
        h,
        JobObjectExtendedLimitInformation,
        &mut limits as *mut _ as *mut _,
        std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
    );
    if ok == 0 {
        return Err(anyhow::anyhow!("SetInformationJobObject failed"));
    }
    Ok(h)
}

fn read_request_file(req_path: &Path) -> Result<String> {
    let content = std::fs::read_to_string(req_path)
        .with_context(|| format!("read request file {}", req_path.display()));
    let _ = std::fs::remove_file(req_path);
    content
}

pub fn main() -> Result<()> {
    let mut input = String::new();
    let mut args = std::env::args().skip(1);
    if let Some(first) = args.next() {
        if let Some(rest) = first.strip_prefix("--request-file=") {
            let req_path = PathBuf::from(rest);
            input = read_request_file(&req_path)?;
        }
    }
    if input.is_empty() {
        anyhow::bail!("runner: no request-file provided");
    }
    let req: RunnerRequest = serde_json::from_str(&input).context("parse runner request json")?;
    let log_dir = Some(req.codex_home.as_path());
    hide_current_user_profile_dir(req.codex_home.as_path());
    log_note(
        &format!(
            "runner start cwd={} cmd={:?} real_codex_home={}",
            req.cwd.display(),
            req.command,
            req.real_codex_home.display()
        ),
        Some(&req.codex_home),
    );

    let policy = parse_policy(&req.policy_json_or_preset).context("parse policy_json_or_preset")?;
    let mut cap_psids: Vec<*mut c_void> = Vec::new();
    for sid in &req.cap_sids {
        let Some(psid) = (unsafe { convert_string_sid_to_sid(sid) }) else {
            anyhow::bail!("ConvertStringSidToSidW failed for capability SID");
        };
        cap_psids.push(psid);
    }
    if cap_psids.is_empty() {
        anyhow::bail!("runner: empty capability SID list");
    }

    // Create restricted token from current process token.
    let base = unsafe { get_current_token_for_restriction()? };
    let token_res: Result<HANDLE> = unsafe {
        match &policy {
            SandboxPolicy::ReadOnly => create_readonly_token_with_caps_from(base, &cap_psids),
            SandboxPolicy::WorkspaceWrite { .. } => {
                create_workspace_write_token_with_caps_from(base, &cap_psids)
            }
            SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. } => {
                unreachable!()
            }
        }
    };
    let h_token = token_res?;
    unsafe {
        CloseHandle(base);
    }
    unsafe {
        for psid in &cap_psids {
            allow_null_device(*psid);
        }
        for psid in cap_psids {
            if !psid.is_null() {
                LocalFree(psid as HLOCAL);
            }
        }
    }

    // Open named pipes for stdio.
    let open_pipe = |name: &str, access: u32| -> Result<HANDLE> {
        let path = to_wide(name);
        let handle = unsafe {
            CreateFileW(
                path.as_ptr(),
                access,
                0,
                std::ptr::null_mut(),
                OPEN_EXISTING,
                0,
                0,
            )
        };
        if handle == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
            let err = unsafe { GetLastError() };
            log_note(
                &format!("CreateFileW failed for pipe {name}: {err}"),
                Some(&req.codex_home),
            );
            return Err(anyhow::anyhow!("CreateFileW failed for pipe {name}: {err}"));
        }
        Ok(handle)
    };
    let h_stdin = open_pipe(&req.stdin_pipe, FILE_GENERIC_READ)?;
    let h_stdout = open_pipe(&req.stdout_pipe, FILE_GENERIC_WRITE)?;
    let h_stderr = open_pipe(&req.stderr_pipe, FILE_GENERIC_WRITE)?;
    let stdio = Some((h_stdin, h_stdout, h_stderr));

    // While the read-ACL helper is running, PowerShell can fail to start in the requested CWD due
    // to unreadable ancestors. Use a junction CWD for that window; once the helper finishes, go
    // back to using the real requested CWD (no probing, no extra state).
    let use_junction = match read_acl_mutex::read_acl_mutex_exists() {
        Ok(exists) => exists,
        Err(err) => {
            // Fail-safe: if we can't determine the state, assume the helper might be running and
            // use the junction path to avoid CWD failures on unreadable ancestors.
            log_note(
                &format!("junction: read_acl_mutex_exists failed: {err}; assuming read ACL helper is running"),
                log_dir,
            );
            true
        }
    };
    if use_junction {
        log_note(
            "junction: read ACL helper running; using junction CWD",
            log_dir,
        );
    }
    let effective_cwd = if use_junction {
        cwd_junction::create_cwd_junction(&req.cwd, log_dir).unwrap_or_else(|| req.cwd.clone())
    } else {
        req.cwd.clone()
    };
    log_note(
        &format!(
            "runner: effective cwd={} (requested {})",
            effective_cwd.display(),
            req.cwd.display()
        ),
        log_dir,
    );

    // Build command and env, spawn with CreateProcessAsUserW.
    let spawn_result = unsafe {
        create_process_as_user(
            h_token,
            &req.command,
            &effective_cwd,
            &req.env_map,
            Some(&req.codex_home),
            stdio,
        )
    };
    let (proc_info, _si) = match spawn_result {
        Ok(v) => v,
        Err(e) => {
            log_note(&format!("runner: spawn failed: {e:?}"), log_dir);
            unsafe {
                CloseHandle(h_stdin);
                CloseHandle(h_stdout);
                CloseHandle(h_stderr);
                CloseHandle(h_token);
            }
            return Err(e);
        }
    };

    // Optional job kill on close.
    let h_job = unsafe { create_job_kill_on_close().ok() };
    if let Some(job) = h_job {
        unsafe {
            let _ = AssignProcessToJobObject(job, proc_info.hProcess);
        }
    }

    // Wait for process.
    let wait_res = unsafe {
        WaitForSingleObject(
            proc_info.hProcess,
            req.timeout_ms.map(|ms| ms as u32).unwrap_or(INFINITE),
        )
    };
    let timed_out = wait_res == WAIT_TIMEOUT;

    let exit_code: i32;
    unsafe {
        if timed_out {
            let _ = TerminateProcess(proc_info.hProcess, 1);
            exit_code = 128 + 64;
        } else {
            let mut raw_exit: u32 = 1;
            windows_sys::Win32::System::Threading::GetExitCodeProcess(
                proc_info.hProcess,
                &mut raw_exit,
            );
            exit_code = raw_exit as i32;
        }
        if proc_info.hThread != 0 {
            CloseHandle(proc_info.hThread);
        }
        if proc_info.hProcess != 0 {
            CloseHandle(proc_info.hProcess);
        }
        CloseHandle(h_stdin);
        CloseHandle(h_stdout);
        CloseHandle(h_stderr);
        CloseHandle(h_token);
        if let Some(job) = h_job {
            CloseHandle(job);
        }
    }
    if exit_code != 0 {
        eprintln!("runner child exited with code {}", exit_code);
    }
    std::process::exit(exit_code);
}

#[cfg(test)]
mod tests {
    use super::read_request_file;
    use pretty_assertions::assert_eq;
    use std::fs;

    #[test]
    fn removes_request_file_after_read() {
        let dir = tempfile::tempdir().expect("tempdir");
        let req_path = dir.path().join("request.json");
        fs::write(&req_path, "{\"ok\":true}").expect("write request");

        let content = read_request_file(&req_path).expect("read request");
        assert_eq!(content, "{\"ok\":true}");
        assert!(!req_path.exists(), "request file should be removed");
    }
}
