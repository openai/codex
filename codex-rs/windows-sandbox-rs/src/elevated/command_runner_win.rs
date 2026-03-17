//! Windows command runner used by the **elevated** sandbox path.
//!
//! The CLI launches this binary under the sandbox user when Windows sandbox level is
//! Elevated. It connects to the IPC pipes, reads the framed `SpawnRequest`, derives a
//! restricted token from the sandbox user, and spawns the child process via ConPTY
//! (`tty=true`) or pipes (`tty=false`). It then streams output frames back to the parent,
//! accepts stdin/terminate frames, and emits a final exit frame. The legacy restricted‑token
//! path spawns the child directly and does not use this runner.

#![cfg(target_os = "windows")]

use anyhow::Context;
use anyhow::Result;
use codex_windows_sandbox::allow_null_device;
use codex_windows_sandbox::convert_string_sid_to_sid;
use codex_windows_sandbox::create_readonly_token_with_caps_from;
use codex_windows_sandbox::create_workspace_write_token_with_caps_from;
use codex_windows_sandbox::get_current_token_for_restriction;
use codex_windows_sandbox::hide_current_user_profile_dir;
use codex_windows_sandbox::ipc_framed::decode_bytes;
use codex_windows_sandbox::ipc_framed::encode_bytes;
use codex_windows_sandbox::ipc_framed::read_frame;
use codex_windows_sandbox::ipc_framed::write_frame;
use codex_windows_sandbox::ipc_framed::ErrorPayload;
use codex_windows_sandbox::ipc_framed::ExitPayload;
use codex_windows_sandbox::ipc_framed::FramedMessage;
use codex_windows_sandbox::ipc_framed::Message;
use codex_windows_sandbox::ipc_framed::OutputPayload;
use codex_windows_sandbox::ipc_framed::OutputStream;
use codex_windows_sandbox::log_note;
use codex_windows_sandbox::parse_policy;
use codex_windows_sandbox::read_handle_loop;
use codex_windows_sandbox::spawn_process_with_pipes;
use codex_windows_sandbox::to_wide;
use codex_windows_sandbox::PipeSpawnHandles;
use codex_windows_sandbox::SandboxPolicy;
use codex_windows_sandbox::StderrMode;
use codex_windows_sandbox::StdinMode;
use std::ffi::c_void;
use std::fs::File;
use std::os::windows::io::FromRawHandle;
use std::path::Path;
use std::path::PathBuf;
use std::ptr;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Foundation::HLOCAL;
use windows_sys::Win32::Storage::FileSystem::CreateFileW;
use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_READ;
use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_WRITE;
use windows_sys::Win32::Storage::FileSystem::OPEN_EXISTING;
use windows_sys::Win32::System::Console::ClosePseudoConsole;
use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
use windows_sys::Win32::System::JobObjects::CreateJobObjectW;
use windows_sys::Win32::System::JobObjects::JobObjectExtendedLimitInformation;
use windows_sys::Win32::System::JobObjects::SetInformationJobObject;
use windows_sys::Win32::System::JobObjects::JOBOBJECT_EXTENDED_LIMIT_INFORMATION;
use windows_sys::Win32::System::JobObjects::JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
use windows_sys::Win32::System::Threading::GetExitCodeProcess;
use windows_sys::Win32::System::Threading::GetProcessId;
use windows_sys::Win32::System::Threading::TerminateProcess;
use windows_sys::Win32::System::Threading::WaitForSingleObject;
use windows_sys::Win32::System::Threading::INFINITE;
use windows_sys::Win32::System::Threading::PROCESS_INFORMATION;

#[path = "cwd_junction.rs"]
mod cwd_junction;

#[allow(dead_code)]
#[path = "../read_acl_mutex.rs"]
mod read_acl_mutex;

const WAIT_TIMEOUT: u32 = 0x0000_0102;

struct IpcSpawnedProcess {
    log_dir: PathBuf,
    pi: PROCESS_INFORMATION,
    stdout_handle: HANDLE,
    stderr_handle: HANDLE,
    stdin_handle: Option<HANDLE>,
    hpc_handle: Option<HANDLE>,
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

/// Open a named pipe created by the parent process.
fn open_pipe(name: &str, access: u32) -> Result<HANDLE> {
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
        return Err(anyhow::anyhow!("CreateFileW failed for pipe {name}: {err}"));
    }
    Ok(handle)
}

/// Send an error frame back to the parent process.
fn send_error(writer: &Arc<StdMutex<File>>, code: &str, message: String) -> Result<()> {
    let msg = FramedMessage {
        version: 1,
        message: Message::Error {
            payload: ErrorPayload {
                message,
                code: code.to_string(),
            },
        },
    };
    if let Ok(mut guard) = writer.lock() {
        write_frame(&mut *guard, &msg)?;
    }
    Ok(())
}

/// Read and validate the initial spawn request frame.
fn read_spawn_request(
    reader: &mut File,
) -> Result<codex_windows_sandbox::ipc_framed::SpawnRequest> {
    let Some(msg) = read_frame(reader)? else {
        anyhow::bail!("runner: pipe closed before spawn_request");
    };
    if msg.version != 1 {
        anyhow::bail!("runner: unsupported protocol version {}", msg.version);
    }
    match msg.message {
        Message::SpawnRequest { payload } => Ok(*payload),
        other => anyhow::bail!("runner: expected spawn_request, got {other:?}"),
    }
}

/// Pick an effective CWD, using a junction if the ACL helper is active.
fn effective_cwd(req_cwd: &Path, log_dir: Option<&Path>) -> PathBuf {
    let use_junction = match read_acl_mutex::read_acl_mutex_exists() {
        Ok(exists) => exists,
        Err(err) => {
            log_note(
                &format!(
                    "junction: read_acl_mutex_exists failed: {err}; assuming read ACL helper is running"
                ),
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
        cwd_junction::create_cwd_junction(req_cwd, log_dir).unwrap_or_else(|| req_cwd.to_path_buf())
    } else {
        req_cwd.to_path_buf()
    }
}

fn spawn_ipc_process(
    req: &codex_windows_sandbox::ipc_framed::SpawnRequest,
) -> Result<IpcSpawnedProcess> {
    let log_dir = req.codex_home.clone();
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
    if !policy.has_full_disk_read_access() {
        anyhow::bail!(
            "Restricted read-only access is not yet supported by the Windows sandbox backend"
        );
    }
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

    let base = unsafe { get_current_token_for_restriction()? };
    let token_res: Result<(HANDLE, *mut c_void)> = unsafe {
        match &policy {
            SandboxPolicy::ReadOnly { .. } => {
                create_readonly_token_with_caps_from(base, &cap_psids)
                    .map(|h_token| (h_token, cap_psids[0]))
            }
            SandboxPolicy::WorkspaceWrite { .. } => {
                create_workspace_write_token_with_caps_from(base, &cap_psids)
                    .map(|h_token| (h_token, cap_psids[0]))
            }
            SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. } => {
                unreachable!()
            }
        }
    };
    let (h_token, psid_to_use) = token_res?;
    unsafe {
        CloseHandle(base);
        allow_null_device(psid_to_use);
        for psid in &cap_psids {
            allow_null_device(*psid);
        }
        for psid in cap_psids {
            if !psid.is_null() {
                LocalFree(psid as HLOCAL);
            }
        }
    }

    let effective_cwd = effective_cwd(&req.cwd, Some(log_dir.as_path()));
    log_note(
        &format!(
            "runner: effective cwd={} (requested {})",
            effective_cwd.display(),
            req.cwd.display()
        ),
        Some(log_dir.as_path()),
    );

    let mut hpc_handle: Option<HANDLE> = None;
    let (pi, stdout_handle, stderr_handle, stdin_handle) = if req.tty {
        let (pi, conpty) = codex_windows_sandbox::spawn_conpty_process_as_user(
            h_token,
            &req.command,
            &effective_cwd,
            &req.env,
            req.use_private_desktop,
            Some(log_dir.as_path()),
        )?;
        let (hpc, input_write, output_read) = conpty.into_raw();
        hpc_handle = Some(hpc);
        let stdin_handle = if req.stdin_open {
            Some(input_write)
        } else {
            unsafe {
                CloseHandle(input_write);
            }
            None
        };
        (
            pi,
            output_read,
            windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE,
            stdin_handle,
        )
    } else {
        let stdin_mode = if req.stdin_open {
            StdinMode::Open
        } else {
            StdinMode::Closed
        };
        let pipe_handles: PipeSpawnHandles = spawn_process_with_pipes(
            h_token,
            &req.command,
            &effective_cwd,
            &req.env,
            stdin_mode,
            StderrMode::Separate,
            req.use_private_desktop,
        )?;
        (
            pipe_handles.process,
            pipe_handles.stdout_read,
            pipe_handles
                .stderr_read
                .unwrap_or(windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE),
            pipe_handles.stdin_write,
        )
    };

    unsafe {
        CloseHandle(h_token);
    }

    Ok(IpcSpawnedProcess {
        log_dir,
        pi,
        stdout_handle,
        stderr_handle,
        stdin_handle,
        hpc_handle,
    })
}

/// Stream stdout/stderr from the child into Output frames.
fn spawn_output_reader(
    writer: Arc<StdMutex<File>>,
    handle: HANDLE,
    stream: OutputStream,
    log_dir: Option<PathBuf>,
) -> std::thread::JoinHandle<()> {
    read_handle_loop(handle, move |chunk| {
        let msg = FramedMessage {
            version: 1,
            message: Message::Output {
                payload: OutputPayload {
                    data_b64: encode_bytes(chunk),
                    stream,
                },
            },
        };
        if let Ok(mut guard) = writer.lock() {
            if let Err(err) = write_frame(&mut *guard, &msg) {
                log_note(
                    &format!("runner output write failed: {err}"),
                    log_dir.as_deref(),
                );
            }
        }
    })
}

/// Read stdin/terminate frames and forward to the child process.
fn spawn_input_loop(
    mut reader: File,
    stdin_handle: Option<HANDLE>,
    process_handle: Arc<StdMutex<Option<HANDLE>>>,
    log_dir: Option<PathBuf>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        loop {
            let msg = match read_frame(&mut reader) {
                Ok(Some(v)) => v,
                Ok(None) => break,
                Err(err) => {
                    log_note(
                        &format!("runner input read failed: {err}"),
                        log_dir.as_deref(),
                    );
                    break;
                }
            };
            match msg.message {
                Message::Stdin { payload } => {
                    let Ok(bytes) = decode_bytes(&payload.data_b64) else {
                        continue;
                    };
                    if let Some(handle) = stdin_handle {
                        let mut written: u32 = 0;
                        unsafe {
                            let _ = windows_sys::Win32::Storage::FileSystem::WriteFile(
                                handle,
                                bytes.as_ptr(),
                                bytes.len() as u32,
                                &mut written,
                                ptr::null_mut(),
                            );
                        }
                    }
                }
                Message::Terminate { .. } => {
                    if let Ok(guard) = process_handle.lock() {
                        if let Some(handle) = guard.as_ref() {
                            unsafe {
                                let _ = TerminateProcess(*handle, 1);
                            }
                        }
                    }
                }
                Message::SpawnRequest { .. } => {}
                Message::SpawnReady { .. } => {}
                Message::Output { .. } => {}
                Message::Exit { .. } => {}
                Message::Error { .. } => {}
            }
        }
        if let Some(handle) = stdin_handle {
            unsafe {
                CloseHandle(handle);
            }
        }
    })
}

/// Entry point for the Windows command runner process.
pub fn main() -> Result<()> {
    let mut pipe_in = None;
    let mut pipe_out = None;
    for arg in std::env::args().skip(1) {
        if let Some(rest) = arg.strip_prefix("--pipe-in=") {
            pipe_in = Some(rest.to_string());
        } else if let Some(rest) = arg.strip_prefix("--pipe-out=") {
            pipe_out = Some(rest.to_string());
        }
    }

    let Some(pipe_in) = pipe_in else {
        anyhow::bail!("runner: no pipe-in provided");
    };
    let Some(pipe_out) = pipe_out else {
        anyhow::bail!("runner: no pipe-out provided");
    };

    let h_pipe_in = open_pipe(&pipe_in, FILE_GENERIC_READ)?;
    let h_pipe_out = open_pipe(&pipe_out, FILE_GENERIC_WRITE)?;
    let mut pipe_read = unsafe { File::from_raw_handle(h_pipe_in as _) };
    let pipe_write = Arc::new(StdMutex::new(unsafe {
        File::from_raw_handle(h_pipe_out as _)
    }));

    let req = match read_spawn_request(&mut pipe_read) {
        Ok(v) => v,
        Err(err) => {
            let _ = send_error(&pipe_write, "spawn_failed", err.to_string());
            return Err(err);
        }
    };

    let ipc_spawn = match spawn_ipc_process(&req) {
        Ok(value) => value,
        Err(err) => {
            let _ = send_error(&pipe_write, "spawn_failed", err.to_string());
            return Err(err);
        }
    };
    let log_dir = Some(ipc_spawn.log_dir.as_path());
    let pi = ipc_spawn.pi;
    let stdout_handle = ipc_spawn.stdout_handle;
    let stderr_handle = ipc_spawn.stderr_handle;
    let stdin_handle = ipc_spawn.stdin_handle;
    let hpc_handle = ipc_spawn.hpc_handle;

    let h_job = unsafe { create_job_kill_on_close().ok() };
    if let Some(job) = h_job {
        unsafe {
            let _ = AssignProcessToJobObject(job, pi.hProcess);
        }
    }

    let process_handle = Arc::new(StdMutex::new(Some(pi.hProcess)));

    let msg = FramedMessage {
        version: 1,
        message: Message::SpawnReady {
            payload: codex_windows_sandbox::ipc_framed::SpawnReady {
                process_id: unsafe { GetProcessId(pi.hProcess) },
            },
        },
    };
    if let Err(err) = if let Ok(mut guard) = pipe_write.lock() {
        write_frame(&mut *guard, &msg)
    } else {
        anyhow::bail!("runner spawn_ready write failed: pipe_write lock poisoned");
    } {
        log_note(&format!("runner spawn_ready write failed: {err}"), log_dir);
        let _ = send_error(&pipe_write, "spawn_failed", err.to_string());
        return Err(err);
    }
    let log_dir_owned = log_dir.map(|p| p.to_path_buf());
    let out_thread = spawn_output_reader(
        Arc::clone(&pipe_write),
        stdout_handle,
        OutputStream::Stdout,
        log_dir_owned.clone(),
    );
    let err_thread = if stderr_handle != windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
        Some(spawn_output_reader(
            Arc::clone(&pipe_write),
            stderr_handle,
            OutputStream::Stderr,
            log_dir_owned.clone(),
        ))
    } else {
        None
    };

    let _input_thread = spawn_input_loop(
        pipe_read,
        stdin_handle,
        Arc::clone(&process_handle),
        log_dir_owned,
    );

    let timeout = req.timeout_ms.map(|ms| ms as u32).unwrap_or(INFINITE);
    let wait_res = unsafe { WaitForSingleObject(pi.hProcess, timeout) };
    let timed_out = wait_res == WAIT_TIMEOUT;

    let exit_code: i32;
    unsafe {
        if timed_out {
            let _ = TerminateProcess(pi.hProcess, 1);
            exit_code = 128 + 64;
        } else {
            let mut raw_exit: u32 = 1;
            GetExitCodeProcess(pi.hProcess, &mut raw_exit);
            exit_code = raw_exit as i32;
        }
        if let Some(hpc) = hpc_handle {
            ClosePseudoConsole(hpc);
        }
        if pi.hThread != 0 {
            CloseHandle(pi.hThread);
        }
        if pi.hProcess != 0 {
            CloseHandle(pi.hProcess);
        }
        if let Some(job) = h_job {
            CloseHandle(job);
        }
    }
    let _ = out_thread.join();
    if let Some(err_thread) = err_thread {
        let _ = err_thread.join();
    }
    let exit_msg = FramedMessage {
        version: 1,
        message: Message::Exit {
            payload: ExitPayload {
                exit_code,
                timed_out,
            },
        },
    };
    if let Ok(mut guard) = pipe_write.lock() {
        if let Err(err) = write_frame(&mut *guard, &exit_msg) {
            log_note(&format!("runner exit write failed: {err}"), log_dir);
        }
    }

    std::process::exit(exit_code);
}
