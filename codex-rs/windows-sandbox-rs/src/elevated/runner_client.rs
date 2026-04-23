use crate::identity::SandboxCreds;
use crate::ipc_framed::FramedMessage;
use crate::ipc_framed::Message;
use crate::ipc_framed::SpawnRequest;
use crate::ipc_framed::read_frame;
use crate::ipc_framed::write_frame;
use crate::runner_pipe::PIPE_ACCESS_INBOUND;
use crate::runner_pipe::PIPE_ACCESS_OUTBOUND;
use crate::runner_pipe::connect_pipe;
use crate::runner_pipe::create_named_pipe;
use crate::runner_pipe::find_runner_exe;
use crate::runner_pipe::pipe_pair;
use crate::winutil::quote_windows_arg;
use crate::winutil::to_wide;
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

const RUNNER_SPAWN_READY_TIMEOUT: Duration = Duration::from_secs(15);
const RUNNER_PIPE_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const RUNNER_SPAWN_READY_POLL_INTERVAL: Duration = Duration::from_millis(50);
const RUNNER_ERROR_MODE_FLAGS: u32 = 0x0001 | 0x0002;

pub(crate) struct RunnerTransport {
    pipe_write: File,
    pipe_read: File,
}

impl RunnerTransport {
    pub(crate) fn send_spawn_request(&mut self, request: SpawnRequest) -> Result<()> {
        let spawn_request = FramedMessage {
            version: 1,
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
            Message::Error { payload } => Err(anyhow::anyhow!("runner error: {}", payload.message)),
            other => Err(anyhow::anyhow!(
                "expected spawn_ready from runner, got {other:?}"
            )),
        }
    }

    pub(crate) fn into_files(self) -> (File, File) {
        (self.pipe_write, self.pipe_read)
    }
}

pub(crate) fn spawn_runner_transport(
    codex_home: &Path,
    cwd: &Path,
    sandbox_creds: &SandboxCreds,
    log_dir: Option<&Path>,
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
    let cwd_w = to_wide(cwd);
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
        let err = unsafe { GetLastError() } as i32;
        unsafe {
            CloseHandle(h_pipe_in);
            CloseHandle(h_pipe_out);
        }
        return Err(anyhow::anyhow!("CreateProcessWithLogonW failed: {err}"));
    }

    let connect_result = (|| -> Result<()> {
        // `connect_pipe` is a synchronous named-pipe call, so there is no built-in async
        // timeout we can apply from this thread. Run the two blocking connects on a helper
        // thread, duplicate that thread's HANDLE back to the parent, and use
        // `CancelSynchronousIo` if the handshake does not finish in time.
        let (thread_handle_tx, thread_handle_rx) = mpsc::sync_channel(1);
        let (connect_result_tx, connect_result_rx) = mpsc::sync_channel(1);
        let connect_thread = thread::Builder::new()
            .name("codex-runner-connect".to_string())
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
                        "DuplicateHandle failed for runner connect thread: {}",
                        unsafe { GetLastError() }
                    )));
                    return;
                }
                // Publish the helper thread HANDLE before we start the blocking pipe connects so
                // the parent always has a way to cancel them on timeout.
                let _ = thread_handle_tx.send(Ok(thread_handle));

                let result = (|| -> Result<()> {
                    connect_pipe(h_pipe_in)?;
                    connect_pipe(h_pipe_out)?;
                    Ok(())
                })();
                let _ = connect_result_tx.send(result);
            })?;
        let thread_handle = thread_handle_rx.recv().map_err(|_| {
            anyhow::anyhow!("runner connect thread exited before publishing its handle")
        })??;

        let connect_result = match connect_result_rx.recv_timeout(RUNNER_PIPE_CONNECT_TIMEOUT) {
            Ok(result) => result,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let cancel_ok = unsafe { CancelSynchronousIo(thread_handle) };
                if cancel_ok == 0 {
                    let err = unsafe { GetLastError() };
                    if err != ERROR_NOT_FOUND {
                        let _ = connect_thread.join();
                        unsafe {
                            CloseHandle(thread_handle);
                        }
                        return Err(anyhow::anyhow!("CancelSynchronousIo failed: {err}"));
                    }
                    // `ERROR_NOT_FOUND` means the helper thread no longer has a cancelable
                    // synchronous I/O request. In practice that usually means the connect call
                    // finished in the narrow race between our timeout firing and the cancel
                    // request reaching the kernel, so we still treat this as the timeout path
                    // and join the thread to observe its final result.
                }
                let _ = connect_thread.join();
                unsafe {
                    CloseHandle(thread_handle);
                }
                return Err(anyhow::anyhow!(
                    "timed out after {}ms connecting runner pipes",
                    RUNNER_PIPE_CONNECT_TIMEOUT.as_millis()
                ));
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(anyhow::anyhow!(
                "runner pipe connect thread exited before reporting its result"
            )),
        };
        let _ = connect_thread.join();
        unsafe {
            CloseHandle(thread_handle);
        }
        connect_result
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

    unsafe {
        if pi.hProcess != 0 {
            // After the handshake succeeds we no longer need the runner process HANDLE. The
            // pipes are now the lifetime anchor for the transport.
            CloseHandle(pi.hProcess);
        }
    }

    let pipe_write = unsafe { File::from_raw_handle(h_pipe_in as _) };
    let pipe_read = unsafe { File::from_raw_handle(h_pipe_out as _) };
    Ok(RunnerTransport {
        pipe_write,
        pipe_read,
    })
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
