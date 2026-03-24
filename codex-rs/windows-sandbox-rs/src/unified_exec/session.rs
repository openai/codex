//! Unified exec session spawner for Windows sandboxing.
//!
//! This module implements the **unified_exec session** paths for Windows by returning a
//! long‑lived `SpawnedProcess` wired for stdin/out/exit. It covers both the legacy
//! restricted‑token path (direct spawn under a restricted token) and the elevated path
//! (spawn via the command runner IPC). It is not used for non‑unified exec capture flows,
//! which continue to use the one‑shot capture APIs.

use crate::acl::revoke_ace;
use crate::conpty::spawn_conpty_process_as_user;
use crate::desktop::LaunchDesktop;
use crate::identity::SandboxCreds;
use crate::ipc_framed::decode_bytes;
use crate::ipc_framed::encode_bytes;
use crate::ipc_framed::read_frame;
use crate::ipc_framed::write_frame;
use crate::ipc_framed::EmptyPayload;
use crate::ipc_framed::FramedMessage;
use crate::ipc_framed::Message;
use crate::ipc_framed::OutputStream;
use crate::ipc_framed::ResizePayload;
use crate::ipc_framed::SpawnRequest;
use crate::ipc_framed::StdinPayload;
use crate::logging::log_failure;
use crate::logging::log_success;
use crate::process::read_handle_loop;
use crate::process::spawn_process_with_pipes;
use crate::process::StderrMode;
use crate::process::StdinMode;
use crate::runner_pipe::connect_pipe;
use crate::runner_pipe::create_named_pipe;
use crate::runner_pipe::find_runner_exe;
use crate::runner_pipe::pipe_pair;
use crate::runner_pipe::PIPE_ACCESS_INBOUND;
use crate::runner_pipe::PIPE_ACCESS_OUTBOUND;
use crate::spawn_prep::allow_null_device_for_workspace_write;
use crate::spawn_prep::apply_legacy_session_acl_rules;
use crate::spawn_prep::prepare_elevated_spawn_context;
use crate::spawn_prep::prepare_legacy_session_security;
use crate::spawn_prep::prepare_legacy_spawn_context;
use crate::token::convert_string_sid_to_sid;
use crate::winutil::quote_windows_arg;
use crate::winutil::to_wide;
use anyhow::Result;
use codex_utils_pty::spawn_from_driver;
use codex_utils_pty::ProcessDriver;
use codex_utils_pty::SpawnedProcess;
use codex_utils_pty::TerminalSize;
use std::collections::HashMap;
use std::ffi::c_void;
use std::fs::File;
use std::os::windows::io::FromRawHandle;
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
use windows_sys::Win32::System::Console::ClosePseudoConsole;
use windows_sys::Win32::System::Console::ResizePseudoConsole;
use windows_sys::Win32::System::Console::COORD;
use windows_sys::Win32::System::Diagnostics::Debug::SetErrorMode;
use windows_sys::Win32::System::Threading::CreateProcessWithLogonW;
use windows_sys::Win32::System::Threading::GetExitCodeProcess;
use windows_sys::Win32::System::Threading::TerminateProcess;
use windows_sys::Win32::System::Threading::WaitForSingleObject;
use windows_sys::Win32::System::Threading::INFINITE;
use windows_sys::Win32::System::Threading::LOGON_WITH_PROFILE;
use windows_sys::Win32::System::Threading::PROCESS_INFORMATION;
use windows_sys::Win32::System::Threading::STARTUPINFOW;

const WAIT_TIMEOUT: u32 = 0x0000_0102;

struct LegacyProcessHandles {
    process: PROCESS_INFORMATION,
    output_join: std::thread::JoinHandle<()>,
    writer_handle: tokio::task::JoinHandle<()>,
    hpc: Option<HANDLE>,
    token_handle: HANDLE,
    desktop: Option<LaunchDesktop>,
}

/// Spawn the restricted-token child directly and attach either pipe or ConPTY I/O.
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

/// Read process output and forward chunks into a broadcast channel.
fn spawn_output_reader(
    output_read: HANDLE,
    output_tx: broadcast::Sender<Vec<u8>>,
) -> std::thread::JoinHandle<()> {
    read_handle_loop(output_read, move |chunk| {
        let _ = output_tx.send(chunk.to_vec());
    })
}

fn normalize_windows_tty_input(bytes: &[u8], previous_was_cr: &mut bool) -> Vec<u8> {
    let mut normalized = Vec::with_capacity(bytes.len());
    for &byte in bytes {
        if byte == b'\n' {
            if !*previous_was_cr {
                normalized.push(b'\r');
            }
            normalized.push(b'\n');
            *previous_was_cr = false;
        } else {
            normalized.push(byte);
            *previous_was_cr = byte == b'\r';
        }
    }
    normalized
}

/// Write stdin chunks from a channel into the child process input handle.
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
            let mut written: u32 = 0;
            unsafe {
                let _ = WriteFile(
                    handle,
                    bytes.as_ptr(),
                    bytes.len() as u32,
                    &mut written,
                    ptr::null_mut(),
                );
            }
        }
        if let Some(handle) = input_write {
            unsafe {
                CloseHandle(handle);
            }
        }
    })
}

/// Start the elevated runner under the sandbox user and connect the parent-side IPC pipes.
fn launch_runner_pipes(
    codex_home: &Path,
    cwd: &Path,
    sandbox_creds: &SandboxCreds,
    pipe_in: String,
    pipe_out: String,
) -> Result<(File, File)> {
    let h_pipe_in = create_named_pipe(&pipe_in, PIPE_ACCESS_OUTBOUND, &sandbox_creds.username)?;
    let h_pipe_out = create_named_pipe(&pipe_out, PIPE_ACCESS_INBOUND, &sandbox_creds.username)?;
    let runner_exe = find_runner_exe(codex_home, None);
    let runner_cmdline = runner_exe
        .to_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "codex-command-runner.exe".to_string());
    let runner_full_cmd = format!(
        "{} {} {}",
        quote_windows_arg(&runner_cmdline),
        quote_windows_arg(&format!("--pipe-in={pipe_in}")),
        quote_windows_arg(&format!("--pipe-out={pipe_out}"))
    );
    let mut cmdline_vec: Vec<u16> = to_wide(&runner_full_cmd);
    let exe_w: Vec<u16> = to_wide(&runner_cmdline);
    let cwd_w: Vec<u16> = to_wide(cwd);
    let env_block: Option<Vec<u16>> = None;
    let mut si: STARTUPINFOW = unsafe { std::mem::zeroed() };
    si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
    let mut pi: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
    let user_w = to_wide(&sandbox_creds.username);
    let domain_w = to_wide(".");
    let password_w = to_wide(&sandbox_creds.password);
    let _ = unsafe { SetErrorMode(0x0001 | 0x0002) };
    let spawn_res = unsafe {
        CreateProcessWithLogonW(
            user_w.as_ptr(),
            domain_w.as_ptr(),
            password_w.as_ptr(),
            LOGON_WITH_PROFILE,
            exe_w.as_ptr(),
            cmdline_vec.as_mut_ptr(),
            windows_sys::Win32::System::Threading::CREATE_NO_WINDOW
                | windows_sys::Win32::System::Threading::CREATE_UNICODE_ENVIRONMENT,
            env_block
                .as_ref()
                .map(|b| b.as_ptr() as *const c_void)
                .unwrap_or(ptr::null()),
            cwd_w.as_ptr(),
            &si,
            &mut pi,
        )
    };
    if spawn_res == 0 {
        let err = unsafe { GetLastError() } as i32;
        return Err(anyhow::anyhow!("CreateProcessWithLogonW failed: {}", err));
    }
    if let Err(err) = connect_pipe(h_pipe_in) {
        unsafe {
            CloseHandle(h_pipe_in);
            CloseHandle(h_pipe_out);
            if pi.hThread != 0 {
                CloseHandle(pi.hThread);
            }
            if pi.hProcess != 0 {
                CloseHandle(pi.hProcess);
            }
        }
        return Err(err.into());
    }
    if let Err(err) = connect_pipe(h_pipe_out) {
        unsafe {
            CloseHandle(h_pipe_in);
            CloseHandle(h_pipe_out);
            if pi.hThread != 0 {
                CloseHandle(pi.hThread);
            }
            if pi.hProcess != 0 {
                CloseHandle(pi.hProcess);
            }
        }
        return Err(err.into());
    }
    unsafe {
        if pi.hThread != 0 {
            CloseHandle(pi.hThread);
        }
        if pi.hProcess != 0 {
            CloseHandle(pi.hProcess);
        }
    }
    let pipe_write = unsafe { File::from_raw_handle(h_pipe_in as _) };
    let pipe_read = unsafe { File::from_raw_handle(h_pipe_out as _) };
    Ok((pipe_write, pipe_read))
}

/// Send the initial spawn request that tells the elevated runner what child to create.
fn send_spawn_request(pipe_write: &mut File, request: SpawnRequest) -> Result<()> {
    let spawn_request = FramedMessage {
        version: 1,
        message: Message::SpawnRequest {
            payload: Box::new(request),
        },
    };
    write_frame(pipe_write, &spawn_request)?;
    Ok(())
}

/// Wait for the runner to acknowledge spawn success or return a structured startup error.
fn read_spawn_ready(pipe_read: &mut File) -> Result<()> {
    let first = read_frame(pipe_read)?
        .ok_or_else(|| anyhow::anyhow!("runner pipe closed before spawn_ready"))?;
    match first.message {
        Message::SpawnReady { .. } => Ok(()),
        Message::Error { payload } => Err(anyhow::anyhow!("runner error: {}", payload.message)),
        other => Err(anyhow::anyhow!("unexpected runner message: {other:?}")),
    }
}

fn start_runner_pipe_writer(mut pipe_write: File) -> std::sync::mpsc::Sender<FramedMessage> {
    let (outbound_tx, outbound_rx) = std::sync::mpsc::channel::<FramedMessage>();
    tokio::task::spawn_blocking(move || {
        while let Ok(msg) = outbound_rx.recv() {
            if write_frame(&mut pipe_write, &msg).is_err() {
                break;
            }
        }
    });
    outbound_tx
}

/// Forward stdin chunks from the process driver into framed IPC messages for the runner.
fn start_runner_stdin_writer(
    mut writer_rx: mpsc::Receiver<Vec<u8>>,
    outbound_tx: std::sync::mpsc::Sender<FramedMessage>,
    normalize_newlines: bool,
    stdin_open: bool,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn_blocking(move || {
        let mut previous_was_cr = false;
        while let Some(bytes) = writer_rx.blocking_recv() {
            let bytes = if normalize_newlines {
                normalize_windows_tty_input(&bytes, &mut previous_was_cr)
            } else {
                bytes
            };
            let msg = FramedMessage {
                version: 1,
                message: Message::Stdin {
                    payload: StdinPayload {
                        data_b64: encode_bytes(&bytes),
                    },
                },
            };
            if outbound_tx.send(msg).is_err() {
                break;
            }
        }
        if stdin_open {
            let msg = FramedMessage {
                version: 1,
                message: Message::CloseStdin {
                    payload: EmptyPayload::default(),
                },
            };
            let _ = outbound_tx.send(msg);
        }
    })
}

/// Translate framed runner output and exit messages back into the process driver channels.
fn start_runner_stdout_reader(
    mut pipe_read: File,
    stdout_tx: broadcast::Sender<Vec<u8>>,
    stderr_tx: Option<broadcast::Sender<Vec<u8>>>,
    exit_tx: oneshot::Sender<i32>,
) {
    std::thread::spawn(move || loop {
        let msg = match read_frame(&mut pipe_read) {
            Ok(Some(v)) => v,
            Ok(None) => {
                let _ = exit_tx.send(-1);
                break;
            }
            Err(_err) => {
                let _ = exit_tx.send(-1);
                break;
            }
        };

        match msg.message {
            Message::Output { payload } => {
                if let Ok(data) = decode_bytes(&payload.data_b64) {
                    match payload.stream {
                        OutputStream::Stdout => {
                            let _ = stdout_tx.send(data);
                        }
                        OutputStream::Stderr => {
                            if let Some(stderr_tx) = stderr_tx.as_ref() {
                                let _ = stderr_tx.send(data);
                            } else {
                                let _ = stdout_tx.send(data);
                            }
                        }
                    }
                }
            }
            Message::Exit { payload } => {
                let _ = exit_tx.send(payload.exit_code);
                break;
            }
            Message::Error { payload: _ } => {
                let _ = exit_tx.send(-1);
                break;
            }
            Message::SpawnReady { .. } => {}
            Message::Stdin { .. } => {}
            Message::CloseStdin { .. } => {}
            Message::Resize { .. } => {}
            Message::SpawnRequest { .. } => {}
            Message::Terminate { .. } => {}
        }
    });
}

/// Finalize process exit, emit exit code, and cleanup handles/ACLs.
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
        let mut raw_exit: u32 = 1;
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

    if !guards.is_empty() {
        if let Some(cap_sid) = cap_sid {
            if let Some(sid) = unsafe { convert_string_sid_to_sid(&cap_sid) } {
                unsafe {
                    for p in guards {
                        revoke_ace(&p, sid);
                    }
                }
            }
        }
    }

    // exit_tx already sent above.
}

fn finish_driver_spawn(driver: ProcessDriver, stdin_open: bool) -> SpawnedProcess {
    let spawned = spawn_from_driver(driver);
    if !stdin_open {
        spawned.session.close_stdin();
    }
    spawned
}

fn resize_conpty_handle(
    hpc: &Arc<StdMutex<Option<HANDLE>>>,
    size: TerminalSize,
) -> anyhow::Result<()> {
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

fn make_runner_resizer(
    outbound_tx: std::sync::mpsc::Sender<FramedMessage>,
) -> Box<dyn FnMut(TerminalSize) -> anyhow::Result<()> + Send> {
    Box::new(move |size: TerminalSize| {
        let msg = FramedMessage {
            version: 1,
            message: Message::Resize {
                payload: ResizePayload {
                    rows: size.rows,
                    cols: size.cols,
                },
            },
        };
        outbound_tx
            .send(msg)
            .map_err(|_| anyhow::anyhow!("runner resize pipe closed"))
    })
}

#[allow(clippy::too_many_arguments)]
/// Spawn a sandboxed process under a restricted token and return a live session.
pub async fn spawn_windows_sandbox_session_legacy(
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
        security.psid_generic,
        security.psid_workspace,
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
                    if let Some(sid) = convert_string_sid_to_sid(&security.cap_sid_str) {
                        for path in &guards {
                            revoke_ace(path, sid);
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
                as Box<dyn FnMut(TerminalSize) -> anyhow::Result<()> + Send>
        }),
    };

    Ok(finish_driver_spawn(driver, stdin_open))
}

#[allow(clippy::too_many_arguments)]
/// Spawn a sandboxed process via the elevated runner IPC path and return a live session.
pub async fn spawn_windows_sandbox_session_elevated(
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
    let _ = timeout_ms;
    let elevated = prepare_elevated_spawn_context(
        policy_json_or_preset,
        sandbox_policy_cwd,
        codex_home,
        cwd,
        &mut env_map,
        &command,
    )?;

    let (pipe_in, pipe_out) = pipe_pair();
    let (mut pipe_write, mut pipe_read) =
        launch_runner_pipes(codex_home, cwd, &elevated.sandbox_creds, pipe_in, pipe_out)?;

    let spawn_request = SpawnRequest {
        command: command.clone(),
        cwd: cwd.to_path_buf(),
        env: env_map.clone(),
        policy_json_or_preset: policy_json_or_preset.to_string(),
        sandbox_policy_cwd: sandbox_policy_cwd.to_path_buf(),
        codex_home: elevated.common.sandbox_base.clone(),
        real_codex_home: codex_home.to_path_buf(),
        cap_sids: elevated.cap_sids.clone(),
        timeout_ms,
        tty,
        stdin_open,
        use_private_desktop,
    };
    send_spawn_request(&mut pipe_write, spawn_request)?;
    read_spawn_ready(&mut pipe_read)?;

    let (writer_tx, writer_rx) = mpsc::channel::<Vec<u8>>(128);
    let (stdout_tx, stdout_rx) = broadcast::channel::<Vec<u8>>(256);
    let stderr_rx = if tty {
        None
    } else {
        Some(broadcast::channel::<Vec<u8>>(256))
    };
    let (exit_tx, exit_rx) = oneshot::channel::<i32>();

    let outbound_tx = start_runner_pipe_writer(pipe_write);
    let writer_handle = start_runner_stdin_writer(writer_rx, outbound_tx.clone(), tty, stdin_open);
    let terminator = {
        let outbound_tx = outbound_tx.clone();
        Some(Box::new(move || {
            let msg = FramedMessage {
                version: 1,
                message: Message::Terminate {
                    payload: EmptyPayload::default(),
                },
            };
            let _ = outbound_tx.send(msg);
        }) as Box<dyn FnMut() + Send + Sync>)
    };

    start_runner_stdout_reader(
        pipe_read,
        stdout_tx,
        stderr_rx.as_ref().map(|(tx, _rx)| tx.clone()),
        exit_tx,
    );

    Ok(finish_driver_spawn(
        ProcessDriver {
            writer_tx,
            stdout_rx,
            stderr_rx: stderr_rx.map(|(_tx, rx)| rx),
            exit_rx,
            terminator,
            writer_handle: Some(writer_handle),
            resizer: if tty {
                Some(make_runner_resizer(outbound_tx))
            } else {
                None
            },
        },
        stdin_open,
    ))
}

#[cfg(test)]
mod tests;
