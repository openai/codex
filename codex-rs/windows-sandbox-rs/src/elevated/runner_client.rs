use crate::identity::SandboxCreds;
use crate::ipc_framed::ErrorPayload;
use crate::ipc_framed::ErrorStage;
use crate::ipc_framed::FramedMessage;
use crate::ipc_framed::IPC_PROTOCOL_VERSION;
use crate::ipc_framed::Message;
use crate::ipc_framed::SpawnRequest;
use crate::ipc_framed::read_frame;
use crate::ipc_framed::write_frame;
use crate::process::WindowsProcessLaunch;
use crate::runner_pipe::PIPE_ACCESS_INBOUND;
use crate::runner_pipe::PIPE_ACCESS_OUTBOUND;
use crate::runner_pipe::connect_pipe;
use crate::runner_pipe::create_named_pipe;
use crate::runner_pipe::find_runner_exe;
use crate::runner_pipe::pipe_pair;
use crate::winutil::quote_windows_arg;
use crate::winutil::to_wide;
use crate::winutil::to_win32_path_wide;
use anyhow::Context;
use anyhow::Result;
use std::ffi::c_void;
use std::fs::File;
use std::os::windows::io::AsRawHandle;
use std::os::windows::io::FromRawHandle;
use std::path::Path;
use std::ptr;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::Foundation::DUPLICATE_SAME_ACCESS;
use windows_sys::Win32::Foundation::DuplicateHandle;
use windows_sys::Win32::Foundation::ERROR_LOGON_FAILURE;
use windows_sys::Win32::Foundation::ERROR_NO_SUCH_LOGON_SESSION;
use windows_sys::Win32::Foundation::ERROR_NOT_FOUND;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::System::Diagnostics::Debug::SetErrorMode;
use windows_sys::Win32::System::IO::CancelSynchronousIo;
use windows_sys::Win32::System::Pipes::PeekNamedPipe;
use windows_sys::Win32::System::Threading::CreateProcessWithLogonW;
use windows_sys::Win32::System::Threading::GetCurrentProcess;
use windows_sys::Win32::System::Threading::GetCurrentThread;
use windows_sys::Win32::System::Threading::LOGON_WITH_PROFILE;
use windows_sys::Win32::System::Threading::PROCESS_INFORMATION;
use windows_sys::Win32::System::Threading::STARTUPINFOW;
use windows_sys::Win32::System::Threading::TerminateProcess;
use windows_sys::Win32::System::Threading::WaitForSingleObject;

const RUNNER_SPAWN_READY_TIMEOUT: Duration = Duration::from_secs(15);
const RUNNER_PIPE_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const RUNNER_SPAWN_READY_POLL_INTERVAL: Duration = Duration::from_millis(50);
const RUNNER_ERROR_MODE_FLAGS: u32 = 0x0001 | 0x0002;
const WAIT_OBJECT_0: u32 = 0;

#[derive(Debug)]
struct RunnerLogonError {
    code: u32,
}

impl std::fmt::Display for RunnerLogonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CreateProcessWithLogonW failed: {}", self.code)
    }
}

impl std::error::Error for RunnerLogonError {}

#[derive(Debug)]
pub(crate) struct RunnerStartupError {
    payload: ErrorPayload,
}

impl RunnerStartupError {
    pub(crate) fn new(payload: ErrorPayload) -> Self {
        Self { payload }
    }
}

impl std::fmt::Display for RunnerStartupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "runner failed during {:?}: {}",
            self.payload.stage, self.payload.message
        )?;
        if let Some(code) = self.payload.windows_error_code {
            write!(f, " (Windows error {code})")?;
        }
        Ok(())
    }
}

impl std::error::Error for RunnerStartupError {}

pub(crate) struct RunnerTransport {
    pipe_write: File,
    pipe_read: File,
}

fn is_refreshable_windows_error(code: u32) -> bool {
    matches!(code, ERROR_LOGON_FAILURE | ERROR_NO_SUCH_LOGON_SESSION)
}

fn command_targets_windows_apps(launch: &WindowsProcessLaunch) -> bool {
    launch
        .application_path
        .as_deref()
        .or_else(|| launch.command.first().map(Path::new))
        .is_some_and(|program| {
            program.components().any(|component| {
                component
                    .as_os_str()
                    .to_string_lossy()
                    .eq_ignore_ascii_case("WindowsApps")
            })
        })
}

pub(crate) fn is_refreshable_sandbox_creds_error(
    err: &anyhow::Error,
    launch: &WindowsProcessLaunch,
) -> bool {
    if err
        .downcast_ref::<RunnerLogonError>()
        .is_some_and(|err| is_refreshable_windows_error(err.code))
    {
        return true;
    }

    err.downcast_ref::<RunnerStartupError>().is_some_and(|err| {
        err.payload.stage == ErrorStage::SpawnChild
            && err.payload.windows_error_code.is_some_and(|code| {
                // AppX activation can return 1312 for a healthy sandbox token. Rotating the
                // account password cannot make the same WindowsApps command launch.
                is_refreshable_windows_error(code)
                    && (code != ERROR_NO_SUCH_LOGON_SESSION
                        || !command_targets_windows_apps(launch))
            })
    })
}

pub(crate) fn retry_runner_spawn_once<T>(
    sandbox_creds: SandboxCreds,
    launch: &WindowsProcessLaunch,
    mut spawn: impl FnMut(SandboxCreds) -> Result<T>,
    refresh: impl FnOnce() -> Result<SandboxCreds>,
) -> Result<T> {
    match spawn(sandbox_creds) {
        Ok(result) => Ok(result),
        Err(err) if is_refreshable_sandbox_creds_error(&err, launch) => spawn(refresh()?),
        Err(err) => Err(err),
    }
}

impl RunnerTransport {
    pub(crate) fn send_spawn_request(&mut self, request: SpawnRequest) -> Result<()> {
        let spawn_request = FramedMessage {
            version: IPC_PROTOCOL_VERSION,
            message: Message::SpawnRequest {
                payload: Box::new(request),
            },
        };
        write_frame(&mut self.pipe_write, &spawn_request)
    }

    pub(crate) fn read_spawn_ready(&mut self) -> Result<()> {
        wait_for_complete_frame(&self.pipe_read, RUNNER_SPAWN_READY_TIMEOUT)?;
        let msg = read_frame(&mut self.pipe_read)?
            .ok_or_else(|| anyhow::anyhow!("runner pipe closed before spawn_ready"))?;
        match msg.message {
            Message::SpawnReady { .. } => Ok(()),
            Message::Error { payload } => Err(RunnerStartupError::new(payload).into()),
            other => Err(anyhow::anyhow!(
                "expected spawn_ready from runner, got {other:?}"
            )),
        }
    }

    pub(crate) fn into_files(self) -> (File, File) {
        (self.pipe_write, self.pipe_read)
    }
}

fn try_take_completed_connect_result(
    connect_thread: &mut Option<thread::JoinHandle<()>>,
    connect_result_rx: &mpsc::Receiver<Result<()>>,
    thread_handle: HANDLE,
    pipe_label: &str,
) -> Result<Option<Result<()>>> {
    let thread_wait = unsafe { WaitForSingleObject(thread_handle, 0) };
    if thread_wait != WAIT_OBJECT_0 {
        return Ok(None);
    }

    if let Some(connect_thread) = connect_thread.take() {
        let _ = connect_thread.join();
    }

    let result = connect_result_rx.recv().map_err(|_| {
        anyhow::anyhow!("runner {pipe_label} connect thread exited before reporting its result")
    })?;
    Ok(Some(result))
}

fn connect_pipe_with_timeout(
    h_pipe: HANDLE,
    expected_runner_pid: u32,
    pipe_label: &str,
) -> Result<()> {
    let pipe_label = pipe_label.to_string();
    let pipe_label_for_thread = pipe_label.clone();
    let (thread_handle_tx, thread_handle_rx) = mpsc::sync_channel(1);
    let (connect_result_tx, connect_result_rx) = mpsc::sync_channel(1);
    let mut connect_thread = Some(
        thread::Builder::new()
            .name(format!("codex-runner-connect-{pipe_label}"))
            .spawn(move || {
                let current_process = unsafe { GetCurrentProcess() };
                let mut thread_handle = 0;
                let duplicate_ok = unsafe {
                    DuplicateHandle(
                        current_process,
                        GetCurrentThread(),
                        current_process,
                        &mut thread_handle,
                        0,
                        0,
                        DUPLICATE_SAME_ACCESS,
                    )
                };
                if duplicate_ok == 0 {
                    let _ = thread_handle_tx.send(Err(anyhow::anyhow!(
                        "DuplicateHandle failed for runner {pipe_label_for_thread} connect thread: {}",
                        unsafe { GetLastError() }
                    )));
                    return;
                }

                // Publish the helper thread HANDLE before the blocking pipe connect so the
                // parent can cancel this specific operation if it times out.
                let _ = thread_handle_tx.send(Ok(thread_handle));

                let result = connect_pipe(h_pipe, expected_runner_pid)
                    .map_err(anyhow::Error::from)
                    .context(format!("connect {pipe_label_for_thread}"));
                let _ = connect_result_tx.send(result);
            })?,
    );
    let thread_handle = thread_handle_rx.recv().map_err(|_| {
        anyhow::anyhow!("runner {pipe_label} connect thread exited before publishing its handle")
    })??;

    let result = match connect_result_rx.recv_timeout(RUNNER_PIPE_CONNECT_TIMEOUT) {
        Ok(result) => {
            if let Some(connect_thread) = connect_thread.take() {
                let _ = connect_thread.join();
            }
            result
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            if let Some(result) = try_take_completed_connect_result(
                &mut connect_thread,
                &connect_result_rx,
                thread_handle,
                &pipe_label,
            )? {
                result
            } else {
                let cancel_ok = unsafe { CancelSynchronousIo(thread_handle) };
                if cancel_ok == 0 {
                    let err = unsafe { GetLastError() };
                    if err != ERROR_NOT_FOUND {
                        Err(anyhow::anyhow!(
                            "CancelSynchronousIo failed for runner {pipe_label} connect thread: {err}"
                        ))
                    } else if let Some(result) = try_take_completed_connect_result(
                        &mut connect_thread,
                        &connect_result_rx,
                        thread_handle,
                        &pipe_label,
                    )? {
                        result
                    } else {
                        Err(anyhow::anyhow!(
                            "timed out after {}ms connecting runner {pipe_label}",
                            RUNNER_PIPE_CONNECT_TIMEOUT.as_millis()
                        ))
                    }
                } else {
                    // Do not join the helper thread on the timeout path. Parent-side cleanup will
                    // close the pipe handles, which lets the blocked connect unwind without
                    // risking another indefinite wait here.
                    Err(anyhow::anyhow!(
                        "timed out after {}ms connecting runner {pipe_label}",
                        RUNNER_PIPE_CONNECT_TIMEOUT.as_millis()
                    ))
                }
            }
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            if let Some(connect_thread) = connect_thread.take() {
                let _ = connect_thread.join();
            }
            Err(anyhow::anyhow!(
                "runner {pipe_label} connect thread exited before reporting its result"
            ))
        }
    };

    unsafe {
        CloseHandle(thread_handle);
    }

    result
}

pub(crate) fn spawn_runner_transport(
    codex_home: &Path,
    cwd: &Path,
    sandbox_creds: &SandboxCreds,
    log_dir: Option<&Path>,
    spawn_request: SpawnRequest,
) -> Result<RunnerTransport> {
    let (pipe_in_name, pipe_out_name) = pipe_pair();
    let h_pipe_in =
        create_named_pipe(&pipe_in_name, PIPE_ACCESS_OUTBOUND, &sandbox_creds.username)?;
    let h_pipe_out =
        create_named_pipe(&pipe_out_name, PIPE_ACCESS_INBOUND, &sandbox_creds.username)?;

    let runner_exe = find_runner_exe(codex_home, log_dir);
    let runner_cmdline = runner_exe
        .to_str()
        .map(str::to_owned)
        .unwrap_or_else(|| "codex-command-runner.exe".to_string());
    let runner_full_cmd = format!(
        "{} {} {}",
        quote_windows_arg(&runner_cmdline),
        quote_windows_arg(&format!("--pipe-in={pipe_in_name}")),
        quote_windows_arg(&format!("--pipe-out={pipe_out_name}"))
    );
    let mut cmdline_vec = to_wide(&runner_full_cmd);
    let exe_w = to_wide(&runner_cmdline);
    let cwd_w = to_win32_path_wide(cwd);
    let user_w = to_wide(&sandbox_creds.username);
    let domain_w = to_wide(".");
    let password_w = to_wide(&sandbox_creds.password);
    let mut si: STARTUPINFOW = unsafe { std::mem::zeroed() };
    si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
    let mut pi: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
    let env_block: Option<Vec<u16>> = None;

    let previous_error_mode = unsafe { SetErrorMode(RUNNER_ERROR_MODE_FLAGS) };
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
                .map(|block| block.as_ptr() as *const c_void)
                .unwrap_or(ptr::null()),
            cwd_w.as_ptr(),
            &si,
            &mut pi,
        )
    };
    unsafe {
        SetErrorMode(previous_error_mode);
    }
    if spawn_res == 0 {
        let err = unsafe { GetLastError() };
        unsafe {
            CloseHandle(h_pipe_in);
            CloseHandle(h_pipe_out);
        }
        return Err(RunnerLogonError { code: err }.into());
    }
    let expected_runner_pid = pi.dwProcessId;

    let connect_result = (|| -> Result<()> {
        connect_pipe_with_timeout(h_pipe_in, expected_runner_pid, "pipe-in")?;
        connect_pipe_with_timeout(h_pipe_out, expected_runner_pid, "pipe-out")?;
        Ok(())
    })();

    unsafe {
        if pi.hThread != 0 {
            CloseHandle(pi.hThread);
        }
    }

    if let Err(err) = connect_result {
        unsafe {
            // Keep the process handle alive until the pipe handshake finishes. If the handshake
            // fails after the runner process has already launched, we still need a way to stop
            // that child instead of leaking a stray `codex-command-runner.exe`.
            if pi.hProcess != 0 {
                let _ = TerminateProcess(pi.hProcess, 1);
                CloseHandle(pi.hProcess);
            }
            CloseHandle(h_pipe_in);
            CloseHandle(h_pipe_out);
        }
        return Err(err);
    }

    let mut transport = RunnerTransport {
        // Once the pipe connect phase succeeds we can transfer the raw HANDLEs into `File`s.
        // From here on, the `RunnerTransport` owns closing the pipes on every success/error path.
        pipe_write: unsafe { File::from_raw_handle(h_pipe_in as _) },
        pipe_read: unsafe { File::from_raw_handle(h_pipe_out as _) },
    };
    let startup_result = (|| -> Result<()> {
        // Keep the runner process HANDLE alive until the *entire* startup handshake finishes.
        // That way, a later `send_spawn_request` or `spawn_ready` failure can still terminate the
        // runner instead of leaving a stray `codex-command-runner.exe` behind.
        transport.send_spawn_request(spawn_request)?;
        transport.read_spawn_ready()?;
        Ok(())
    })();
    if let Err(err) = startup_result {
        unsafe {
            if pi.hProcess != 0 {
                let _ = TerminateProcess(pi.hProcess, 1);
                CloseHandle(pi.hProcess);
            }
        }
        drop(transport);
        return Err(err);
    }

    unsafe {
        if pi.hProcess != 0 {
            // The runner has now connected both pipes *and* acknowledged the spawn request, so
            // startup is complete. At that point the transport pipes become the only lifetime
            // anchor we need to keep the session alive.
            CloseHandle(pi.hProcess);
        }
    }

    Ok(transport)
}

fn wait_for_complete_frame(pipe_read: &File, timeout: Duration) -> Result<()> {
    let handle = pipe_read.as_raw_handle() as HANDLE;
    let deadline = Instant::now() + timeout;
    let mut len_buf = [0u8; 4];

    loop {
        let mut bytes_read = 0u32;
        let mut total_available = 0u32;
        let ok = unsafe {
            PeekNamedPipe(
                handle,
                len_buf.as_mut_ptr() as *mut c_void,
                len_buf.len() as u32,
                &mut bytes_read,
                &mut total_available,
                ptr::null_mut(),
            )
        };
        if ok == 0 {
            let err = unsafe { GetLastError() } as i32;
            return Err(anyhow::anyhow!(
                "PeekNamedPipe failed while waiting for spawn_ready: {err}"
            ));
        }

        if bytes_read == len_buf.len() as u32 {
            let frame_len = u32::from_le_bytes(len_buf) as usize;
            let total_len = frame_len
                .checked_add(len_buf.len())
                .ok_or_else(|| anyhow::anyhow!("runner frame length overflow"))?;
            if total_available as usize >= total_len {
                return Ok(());
            }
        }

        if Instant::now() >= deadline {
            return Err(anyhow::anyhow!(
                "timed out after {}ms waiting for runner spawn_ready",
                timeout.as_millis()
            ));
        }

        std::thread::sleep(RUNNER_SPAWN_READY_POLL_INTERVAL);
    }
}

#[cfg(test)]
mod tests {
    use super::RunnerLogonError;
    use super::RunnerStartupError;
    use super::is_refreshable_sandbox_creds_error;
    use crate::ipc_framed::ErrorPayload;
    use crate::ipc_framed::ErrorStage;
    use crate::process::WindowsProcessLaunch;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;
    use windows_sys::Win32::Foundation::ERROR_LOGON_FAILURE;
    use windows_sys::Win32::Foundation::ERROR_NO_SUCH_LOGON_SESSION;
    use windows_sys::Win32::Foundation::ERROR_NOT_FOUND;

    #[test]
    fn refreshable_sandbox_creds_error_recognizes_credential_and_child_start_failures() {
        let launch = WindowsProcessLaunch {
            application_path: None,
            command: vec!["cmd.exe".to_string()],
        };
        assert_eq!(
            [
                ERROR_LOGON_FAILURE,
                ERROR_NO_SUCH_LOGON_SESSION,
                ERROR_NOT_FOUND,
            ]
            .map(|code| {
                let err =
                    anyhow::Error::new(RunnerLogonError { code }).context("runner launch failed");
                is_refreshable_sandbox_creds_error(&err, &launch)
            }),
            [true, true, false]
        );

        assert_eq!(
            [
                (ErrorStage::SpawnChild, ERROR_NO_SUCH_LOGON_SESSION),
                (ErrorStage::SpawnChild, ERROR_NOT_FOUND),
                (ErrorStage::ReadSpawnRequest, ERROR_NO_SUCH_LOGON_SESSION),
            ]
            .map(|(stage, windows_error_code)| {
                let err = anyhow::Error::new(RunnerStartupError::new(ErrorPayload {
                    message: "runner startup failed".to_string(),
                    stage,
                    windows_error_code: Some(windows_error_code),
                }));
                is_refreshable_sandbox_creds_error(&err, &launch)
            }),
            [true, false, false]
        );

        let windows_apps_launches = [
            WindowsProcessLaunch {
                application_path: Some(PathBuf::from(
                    r"C:\Users\user\AppData\Local\Microsoft\WindowsApps\pwsh.exe",
                )),
                command: vec!["pwsh.exe".to_string()],
            },
            WindowsProcessLaunch {
                application_path: Some(PathBuf::from(
                    r"C:\Program Files\WindowsApps\Microsoft.PowerShell_7.6.3.0_x64__8wekyb3d8bbwe\pwsh.exe",
                )),
                command: vec!["pwsh.exe".to_string()],
            },
        ];
        assert_eq!(
            windows_apps_launches.map(|launch| {
                let err = anyhow::Error::new(RunnerStartupError::new(ErrorPayload {
                    message: "runner startup failed".to_string(),
                    stage: ErrorStage::SpawnChild,
                    windows_error_code: Some(ERROR_NO_SUCH_LOGON_SESSION),
                }));
                is_refreshable_sandbox_creds_error(&err, &launch)
            }),
            [false, false]
        );
    }
}
