use super::windows_common::finish_driver_spawn;
use super::windows_common::normalize_windows_tty_input;
use crate::acl::revoke_ace;
use crate::conpty::spawn_conpty_process_as_user;
use crate::desktop::LaunchDesktop;
use crate::logging::log_failure;
use crate::logging::log_success;
use crate::process::StderrMode;
use crate::process::StdinMode;
use crate::process::read_handle_loop;
use crate::process::spawn_process_with_pipes;
use crate::spawn_prep::LocalSid;
use crate::spawn_prep::allow_null_device_for_workspace_write;
use crate::spawn_prep::apply_legacy_session_acl_rules;
use crate::spawn_prep::prepare_legacy_session_security;
use crate::spawn_prep::prepare_legacy_spawn_context;
use anyhow::Result;
use codex_utils_pty::ProcessDriver;
use codex_utils_pty::SpawnedProcess;
use codex_utils_pty::TerminalSize;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::ptr;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::Storage::FileSystem::WriteFile;
use windows_sys::Win32::System::Console::COORD;
use windows_sys::Win32::System::Console::ClosePseudoConsole;
use windows_sys::Win32::System::Console::ResizePseudoConsole;
use windows_sys::Win32::System::Threading::GetExitCodeProcess;
use windows_sys::Win32::System::Threading::INFINITE;
use windows_sys::Win32::System::Threading::PROCESS_INFORMATION;
use windows_sys::Win32::System::Threading::TerminateProcess;
use windows_sys::Win32::System::Threading::WaitForSingleObject;

const WAIT_TIMEOUT: u32 = 0x0000_0102;

struct LegacyProcessHandles {
    process: PROCESS_INFORMATION,
    output_join: std::thread::JoinHandle<()>,
    writer_handle: tokio::task::JoinHandle<()>,
    hpc: Option<HANDLE>,
    token_handle: HANDLE,
    desktop: Option<LaunchDesktop>,
}

#[allow(clippy::too_many_arguments)]
fn spawn_legacy_process(
    h_token: HANDLE,
    command: &[String],
    cwd: &Path,
    env_map: &HashMap<String, String>,
    use_private_desktop: bool,
    tty: bool,
    stdin_open: bool,
    stdout_tx: broadcast::Sender<Vec<u8>>,
    stderr_tx: Option<broadcast::Sender<Vec<u8>>>,
    writer_rx: mpsc::Receiver<Vec<u8>>,
    logs_base_dir: Option<&Path>,
) -> Result<LegacyProcessHandles> {
    let (pi, output_join, writer_handle, hpc, desktop) = if tty {
        let (pi, conpty) = spawn_conpty_process_as_user(
            h_token,
            command,
            cwd,
            env_map,
            use_private_desktop,
            logs_base_dir,
        )?;
        let (hpc, input_write, output_read, desktop) = conpty.into_raw();
        let output_join = spawn_output_reader(output_read, stdout_tx);
        let writer_handle = spawn_input_writer(
            Some(input_write),
            writer_rx,
            /*normalize_newlines*/ true,
        );
        (pi, output_join, writer_handle, Some(hpc), desktop)
    } else {
        let pipe_handles = spawn_process_with_pipes(
            h_token,
            command,
            cwd,
            env_map,
            if stdin_open {
                StdinMode::Open
            } else {
                StdinMode::Closed
            },
            StderrMode::Separate,
            use_private_desktop,
            logs_base_dir,
        )?;
        let stdout_join = spawn_output_reader(pipe_handles.stdout_read, stdout_tx);
        let stderr_join = spawn_output_reader(
            pipe_handles
                .stderr_read
                .expect("separate stderr handle should be present"),
            stderr_tx.expect("separate stderr channel should be present"),
        );
        let output_join = std::thread::spawn(move || {
            let _ = stdout_join.join();
            let _ = stderr_join.join();
        });
        let writer_handle = spawn_input_writer(
            pipe_handles.stdin_write,
            writer_rx,
            /*normalize_newlines*/ false,
        );
        (
            pipe_handles.process,
            output_join,
            writer_handle,
            None,
            Some(pipe_handles.desktop),
        )
    };
    Ok(LegacyProcessHandles {
        process: pi,
        output_join,
        writer_handle,
        hpc,
        token_handle: h_token,
        desktop,
    })
}

fn spawn_output_reader(
    output_read: HANDLE,
    output_tx: broadcast::Sender<Vec<u8>>,
) -> std::thread::JoinHandle<()> {
    read_handle_loop(output_read, move |chunk| {
        let _ = output_tx.send(chunk.to_vec());
    })
}

fn spawn_input_writer(
    input_write: Option<HANDLE>,
    mut writer_rx: mpsc::Receiver<Vec<u8>>,
    normalize_newlines: bool,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn_blocking(move || {
        let mut previous_was_cr = false;
        while let Some(bytes) = writer_rx.blocking_recv() {
            let Some(handle) = input_write else {
                continue;
            };
            let bytes = if normalize_newlines {
                normalize_windows_tty_input(&bytes, &mut previous_was_cr)
            } else {
                bytes
            };
            if write_all_handle(handle, &bytes).is_err() {
                break;
            }
        }
        if let Some(handle) = input_write {
            unsafe {
                CloseHandle(handle);
            }
        }
    })
}

fn write_all_handle(handle: HANDLE, mut bytes: &[u8]) -> Result<()> {
    while !bytes.is_empty() {
        let mut written = 0u32;
        let ok = unsafe {
            WriteFile(
                handle,
                bytes.as_ptr() as *const _,
                bytes.len() as u32,
                &mut written,
                ptr::null_mut(),
            )
        };
        if ok == 0 {
            let err = unsafe { GetLastError() } as i32;
            return Err(anyhow::anyhow!("WriteFile failed: {err}"));
        }
        if written == 0 {
            anyhow::bail!("WriteFile returned success but wrote 0 bytes");
        }
        bytes = &bytes[written as usize..];
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn finalize_exit(
    exit_tx: oneshot::Sender<i32>,
    process_handle: Arc<StdMutex<Option<HANDLE>>>,
    thread_handle: HANDLE,
    output_join: std::thread::JoinHandle<()>,
    guards: Vec<PathBuf>,
    cap_sid: Option<String>,
    logs_base_dir: Option<&Path>,
    command: Vec<String>,
) {
    let exit_code = {
        let mut raw_exit = 1u32;
        if let Ok(guard) = process_handle.lock() {
            if let Some(handle) = guard.as_ref() {
                unsafe {
                    WaitForSingleObject(*handle, INFINITE);
                    GetExitCodeProcess(*handle, &mut raw_exit);
                }
            }
        }
        raw_exit as i32
    };

    let _ = output_join.join();
    let _ = exit_tx.send(exit_code);

    unsafe {
        if thread_handle != 0 && thread_handle != INVALID_HANDLE_VALUE {
            CloseHandle(thread_handle);
        }
        if let Ok(mut guard) = process_handle.lock() {
            if let Some(handle) = guard.take() {
                CloseHandle(handle);
            }
        }
    }

    if exit_code == 0 {
        log_success(&command, logs_base_dir);
    } else {
        log_failure(&command, &format!("exit code {}", exit_code), logs_base_dir);
    }

    if let Some(cap_sid) = cap_sid {
        if let Ok(sid) = LocalSid::from_string(&cap_sid) {
            unsafe {
                for path in guards {
                    revoke_ace(&path, sid.as_ptr());
                }
            }
        }
    }
}

fn resize_conpty_handle(hpc: &Arc<StdMutex<Option<HANDLE>>>, size: TerminalSize) -> Result<()> {
    let guard = hpc
        .lock()
        .map_err(|_| anyhow::anyhow!("failed to lock ConPTY handle"))?;
    let hpc = guard
        .as_ref()
        .copied()
        .ok_or_else(|| anyhow::anyhow!("process is not attached to a PTY"))?;
    let result = unsafe {
        ResizePseudoConsole(
            hpc,
            COORD {
                X: size.cols as i16,
                Y: size.rows as i16,
            },
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "failed to resize console: HRESULT {result}"
        ))
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn spawn_windows_sandbox_session_legacy(
    policy_json_or_preset: &str,
    sandbox_policy_cwd: &Path,
    codex_home: &Path,
    command: Vec<String>,
    cwd: &Path,
    mut env_map: HashMap<String, String>,
    timeout_ms: Option<u64>,
    tty: bool,
    stdin_open: bool,
    use_private_desktop: bool,
) -> Result<SpawnedProcess> {
    let common = prepare_legacy_spawn_context(
        policy_json_or_preset,
        codex_home,
        cwd,
        &mut env_map,
        &command,
        false,
        false,
    )?;
    if !common.policy.has_full_disk_read_access() {
        anyhow::bail!("Restricted read-only access requires the elevated Windows sandbox backend");
    }
    let security = prepare_legacy_session_security(&common.policy, codex_home, cwd)?;
    allow_null_device_for_workspace_write(common.is_workspace_write);

    let persist_aces = common.is_workspace_write;
    let guards = apply_legacy_session_acl_rules(
        &common.policy,
        sandbox_policy_cwd,
        &common.current_dir,
        &env_map,
        &security.psid_generic,
        security.psid_workspace.as_ref(),
        persist_aces,
    );

    let (writer_tx, writer_rx) = mpsc::channel::<Vec<u8>>(128);
    let (stdout_tx, stdout_rx) = broadcast::channel::<Vec<u8>>(256);
    let stderr_rx = if tty {
        None
    } else {
        Some(broadcast::channel::<Vec<u8>>(256))
    };
    let (exit_tx, exit_rx) = oneshot::channel::<i32>();

    let LegacyProcessHandles {
        process: pi,
        output_join,
        writer_handle,
        hpc,
        token_handle,
        desktop,
    } = match spawn_legacy_process(
        security.h_token,
        &command,
        cwd,
        &env_map,
        use_private_desktop,
        tty,
        stdin_open,
        stdout_tx.clone(),
        stderr_rx.as_ref().map(|(tx, _rx)| tx.clone()),
        writer_rx,
        common.logs_base_dir.as_deref(),
    ) {
        Ok(handles) => handles,
        Err(err) => {
            unsafe {
                if !persist_aces && !guards.is_empty() {
                    if let Ok(sid) = LocalSid::from_string(&security.cap_sid_str) {
                        for path in &guards {
                            revoke_ace(path, sid.as_ptr());
                        }
                    }
                }
                CloseHandle(security.h_token);
            }
            return Err(err);
        }
    };
    let hpc_handle = hpc.map(|hpc| Arc::new(StdMutex::new(Some(hpc))));

    let process_handle = Arc::new(StdMutex::new(Some(pi.hProcess)));
    let wait_handle = Arc::clone(&process_handle);
    let command_for_wait = command.clone();
    let guards_for_wait = if persist_aces { Vec::new() } else { guards };
    let cap_sid_for_wait = if guards_for_wait.is_empty() {
        None
    } else {
        Some(security.cap_sid_str.clone())
    };
    let hpc_for_wait = hpc_handle.clone();
    std::thread::spawn(move || {
        let _desktop = desktop;
        let timeout = timeout_ms.map(|ms| ms as u32).unwrap_or(INFINITE);
        let wait_res = unsafe { WaitForSingleObject(pi.hProcess, timeout) };
        if wait_res == WAIT_TIMEOUT {
            unsafe {
                if let Ok(guard) = wait_handle.lock() {
                    if let Some(handle) = guard.as_ref() {
                        let _ = TerminateProcess(*handle, 1);
                    }
                }
            }
        }
        if let Some(hpc) = hpc_for_wait {
            if let Ok(mut guard) = hpc.lock() {
                if let Some(hpc) = guard.take() {
                    unsafe {
                        ClosePseudoConsole(hpc);
                    }
                }
            }
        }
        unsafe {
            if token_handle != 0 && token_handle != INVALID_HANDLE_VALUE {
                CloseHandle(token_handle);
            }
        }
        finalize_exit(
            exit_tx,
            wait_handle,
            pi.hThread,
            output_join,
            guards_for_wait,
            cap_sid_for_wait,
            common.logs_base_dir.as_deref(),
            command_for_wait,
        );
    });

    let terminator = {
        let process_handle = Arc::clone(&process_handle);
        Some(Box::new(move || {
            if let Ok(guard) = process_handle.lock() {
                if let Some(handle) = guard.as_ref() {
                    unsafe {
                        let _ = TerminateProcess(*handle, 1);
                    }
                }
            }
        }) as Box<dyn FnMut() + Send + Sync>)
    };

    let driver = ProcessDriver {
        writer_tx,
        stdout_rx,
        stderr_rx: stderr_rx.map(|(_tx, rx)| rx),
        exit_rx,
        terminator,
        writer_handle: Some(writer_handle),
        resizer: hpc_handle.map(|hpc| {
            Box::new(move |size| resize_conpty_handle(&hpc, size))
                as Box<dyn FnMut(TerminalSize) -> Result<()> + Send>
        }),
    };

    Ok(finish_driver_spawn(driver, stdin_open))
}
