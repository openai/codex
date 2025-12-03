use anyhow::{Context, Result};
use codex_windows_sandbox::{
    allow_null_device, cap_sid_file, convert_string_sid_to_sid, create_process_as_user,
    create_readonly_token_with_cap_from, create_workspace_write_token_with_cap_from,
    get_current_token_for_restriction, load_or_create_cap_sids, log_note, parse_policy, to_wide,
    SandboxPolicy,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::ffi::c_void;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_GENERIC_READ, FILE_GENERIC_WRITE, OPEN_EXISTING,
};
use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
use windows_sys::Win32::System::JobObjects::CreateJobObjectW;
use windows_sys::Win32::System::JobObjects::JobObjectExtendedLimitInformation;
use windows_sys::Win32::System::JobObjects::SetInformationJobObject;
use windows_sys::Win32::System::JobObjects::JOBOBJECT_EXTENDED_LIMIT_INFORMATION;
use windows_sys::Win32::System::JobObjects::JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
use windows_sys::Win32::System::Threading::WaitForSingleObject;
use windows_sys::Win32::System::Threading::INFINITE;

#[derive(Debug, Deserialize)]
struct RunnerRequest {
    policy_json_or_preset: String,
    #[allow(dead_code)]
    sandbox_policy_cwd: PathBuf,
    codex_home: PathBuf,
    command: Vec<String>,
    cwd: PathBuf,
    env_map: HashMap<String, String>,
    timeout_ms: Option<u64>,
    stdin_pipe: String,
    stdout_pipe: String,
    stderr_pipe: String,
}

// Best-effort early marker to detect image load before main.
#[used]
#[allow(dead_code)]
static LOAD_MARKER: fn() = load_marker;

#[allow(dead_code)]
const fn load_marker() {
    // const fn placeholder; actual work is in write_load_marker, invoked at start of main.
}

fn write_load_marker() {
    if let Some(mut p) = dirs_next::home_dir() {
        p.push(".codex");
        let _ = std::fs::create_dir_all(&p);
        p.push("runner_load_marker.txt");
        let _ = std::fs::write(&p, "loaded");
    }
}

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

fn main() -> Result<()> {
    write_load_marker();
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .context("read request")?;
    let req: RunnerRequest =
        serde_json::from_str(&input).context("parse runner request json from stdin")?;
    log_note(
        &format!(
            "runner start cwd={} cmd={:?}",
            req.cwd.display(),
            req.command
        ),
        Some(&req.codex_home),
    );
    log_note(
        &format!(
            "stdin_pipe={} stdout_pipe={} stderr_pipe={}",
            req.stdin_pipe, req.stdout_pipe, req.stderr_pipe
        ),
        Some(&req.codex_home),
    );

    let policy = parse_policy(&req.policy_json_or_preset).context("parse policy_json_or_preset")?;
    // Ensure cap SIDs exist.
    let caps = load_or_create_cap_sids(&req.codex_home);
    let cap_sid_path = cap_sid_file(&req.codex_home);
    fs::write(&cap_sid_path, serde_json::to_string(&caps)?).context("write cap sid file")?;

    let psid_cap: *mut c_void = match &policy {
        SandboxPolicy::ReadOnly => unsafe { convert_string_sid_to_sid(&caps.readonly).unwrap() },
        SandboxPolicy::WorkspaceWrite { .. } => unsafe {
            convert_string_sid_to_sid(&caps.workspace).unwrap()
        },
        SandboxPolicy::DangerFullAccess => {
            anyhow::bail!("DangerFullAccess is not supported for runner")
        }
    };

    // Create restricted token from current process token.
    let base = unsafe { get_current_token_for_restriction()? };
    let token_res: Result<(HANDLE, *mut c_void)> = unsafe {
        match &policy {
            SandboxPolicy::ReadOnly => create_readonly_token_with_cap_from(base, psid_cap),
            SandboxPolicy::WorkspaceWrite { .. } => {
                create_workspace_write_token_with_cap_from(base, psid_cap)
            }
            SandboxPolicy::DangerFullAccess => unreachable!(),
        }
    };
    let (h_token, psid_to_use) = token_res?;
    unsafe {
        CloseHandle(base);
    }

    unsafe {
        allow_null_device(psid_to_use);
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
    log_note("pipes opened", Some(&req.codex_home));

    // Build command and env, spawn with CreateProcessWithTokenW.
    let (proc_info, _si) = unsafe {
        create_process_as_user(
            h_token,
            &req.command,
            &req.cwd,
            &req.env_map,
            Some(&req.codex_home),
            Some((h_stdin, h_stdout, h_stderr)),
        )?
    };
    log_note("spawned child process", Some(&req.codex_home));

    // Optional job kill on close.
    let h_job = unsafe { create_job_kill_on_close().ok() };
    if let Some(job) = h_job {
        unsafe {
            let _ = AssignProcessToJobObject(job, proc_info.hProcess);
        }
    }

    // Wait for process.
    let _ = unsafe {
        WaitForSingleObject(
            proc_info.hProcess,
            req.timeout_ms.map(|ms| ms as u32).unwrap_or(INFINITE),
        )
    };
    let mut exit_code: u32 = 1;
    unsafe {
        windows_sys::Win32::System::Threading::GetExitCodeProcess(
            proc_info.hProcess,
            &mut exit_code,
        );
        if proc_info.hThread != 0 {
            CloseHandle(proc_info.hThread);
        }
        if proc_info.hProcess != 0 {
            CloseHandle(proc_info.hProcess);
        }
        CloseHandle(h_token);
        if let Some(job) = h_job {
            CloseHandle(job);
        }
    }
    log_note(
        &format!("runner exiting with code {}", exit_code),
        Some(&req.codex_home),
    );
    std::process::exit(exit_code as i32);
}
