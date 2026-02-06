use std::collections::HashMap;
use std::io;
use std::io::ErrorKind;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use anyhow::Result;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::process::Command;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::process::ChildTerminator;
use crate::process::ProcessHandle;
use crate::process::SpawnedProcess;

#[cfg(target_os = "linux")]
use libc;

struct PipeChildTerminator {
    #[cfg(windows)]
    pid: u32,
    #[cfg(unix)]
    process_group_id: u32,
}

impl ChildTerminator for PipeChildTerminator {
    fn kill(&mut self) -> io::Result<()> {
        #[cfg(unix)]
        {
            crate::process_group::kill_process_group(self.process_group_id)
        }

        #[cfg(windows)]
        {
            kill_process(self.pid)
        }

        #[cfg(not(any(unix, windows)))]
        {
            Ok(())
        }
    }
}

#[cfg(windows)]
fn kill_process(pid: u32) -> io::Result<()> {
    unsafe {
        let handle = winapi::um::processthreadsapi::OpenProcess(
            winapi::um::winnt::PROCESS_TERMINATE,
            0,
            pid,
        );
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let success = winapi::um::processthreadsapi::TerminateProcess(handle, 1);
        let err = io::Error::last_os_error();
        winapi::um::handleapi::CloseHandle(handle);
        if success == 0 {
            Err(err)
        } else {
            Ok(())
        }
    }
}

async fn read_output_stream<R>(mut reader: R, output_tx: broadcast::Sender<Vec<u8>>)
where
    R: AsyncRead + Unpin,
{
    let mut buf = vec![0u8; 8_192];
    loop {
        match reader.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => {
                let _ = output_tx.send(buf[..n].to_vec());
            }
            Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
}

#[derive(Clone, Copy)]
enum PipeStdinMode {
    Piped,
    Null,
}

async fn spawn_process_with_stdin_mode(
    program: &str,
    args: &[String],
    cwd: &Path,
    env: &HashMap<String, String>,
    arg0: &Option<String>,
    stdin_mode: PipeStdinMode,
) -> Result<SpawnedProcess> {
    if program.is_empty() {
        anyhow::bail!("missing program for pipe spawn");
    }

    let mut command = Command::new(program);
    #[cfg(unix)]
    if let Some(arg0) = arg0 {
        command.arg0(arg0);
    }
    #[cfg(target_os = "linux")]
    let parent_pid = unsafe { libc::getpid() };
    #[cfg(unix)]
    unsafe {
        command.pre_exec(move || {
            crate::process_group::detach_from_tty()?;
            #[cfg(target_os = "linux")]
            crate::process_group::set_parent_death_signal(parent_pid)?;
            Ok(())
        });
    }
    #[cfg(not(unix))]
    let _ = arg0;
    command.current_dir(cwd);
    command.env_clear();
    for (key, value) in env {
        command.env(key, value);
    }
    for arg in args {
        command.arg(arg);
    }
    match stdin_mode {
        PipeStdinMode::Piped => {
            command.stdin(Stdio::piped());
        }
        PipeStdinMode::Null => {
            command.stdin(Stdio::null());
        }
    }
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let mut child = command.spawn()?;
    let pid = child
        .id()
        .ok_or_else(|| io::Error::other("missing child pid"))?;
    #[cfg(unix)]
    let process_group_id = pid;

    let stdin = child.stdin.take();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let (writer_tx, mut writer_rx) = mpsc::channel::<Vec<u8>>(128);
    let (output_tx, _) = broadcast::channel::<Vec<u8>>(256);
    let initial_output_rx = output_tx.subscribe();

    let writer_handle = if let Some(stdin) = stdin {
        let writer = Arc::new(tokio::sync::Mutex::new(stdin));
        tokio::spawn(async move {
            while let Some(bytes) = writer_rx.recv().await {
                let mut guard = writer.lock().await;
                let _ = guard.write_all(&bytes).await;
                let _ = guard.flush().await;
            }
        })
    } else {
        drop(writer_rx);
        tokio::spawn(async {})
    };

    let stdout_handle = stdout.map(|stdout| {
        let output_tx = output_tx.clone();
        tokio::spawn(async move {
            read_output_stream(BufReader::new(stdout), output_tx).await;
        })
    });
    let stderr_handle = stderr.map(|stderr| {
        let output_tx = output_tx.clone();
        tokio::spawn(async move {
            read_output_stream(BufReader::new(stderr), output_tx).await;
        })
    });
    let mut reader_abort_handles = Vec::new();
    if let Some(handle) = stdout_handle.as_ref() {
        reader_abort_handles.push(handle.abort_handle());
    }
    if let Some(handle) = stderr_handle.as_ref() {
        reader_abort_handles.push(handle.abort_handle());
    }
    let reader_handle = tokio::spawn(async move {
        if let Some(handle) = stdout_handle {
            let _ = handle.await;
        }
        if let Some(handle) = stderr_handle {
            let _ = handle.await;
        }
    });

    let (exit_tx, exit_rx) = oneshot::channel::<i32>();
    let exit_status = Arc::new(AtomicBool::new(false));
    let wait_exit_status = Arc::clone(&exit_status);
    let exit_code = Arc::new(StdMutex::new(None));
    let wait_exit_code = Arc::clone(&exit_code);
    let wait_handle: JoinHandle<()> = tokio::spawn(async move {
        let code = match child.wait().await {
            Ok(status) => status.code().unwrap_or(-1),
            Err(_) => -1,
        };
        wait_exit_status.store(true, std::sync::atomic::Ordering::SeqCst);
        if let Ok(mut guard) = wait_exit_code.lock() {
            *guard = Some(code);
        }
        let _ = exit_tx.send(code);
    });

    let (handle, output_rx) = ProcessHandle::new(
        writer_tx,
        output_tx,
        initial_output_rx,
        Box::new(PipeChildTerminator {
            #[cfg(windows)]
            pid,
            #[cfg(unix)]
            process_group_id,
        }),
        reader_handle,
        reader_abort_handles,
        writer_handle,
        wait_handle,
        exit_status,
        exit_code,
        None,
    );

    Ok(SpawnedProcess {
        session: handle,
        output_rx,
        exit_rx,
    })
}

/// Spawn a process using regular pipes (no PTY), returning handles for stdin, output, and exit.
pub async fn spawn_process(
    program: &str,
    args: &[String],
    cwd: &Path,
    env: &HashMap<String, String>,
    arg0: &Option<String>,
) -> Result<SpawnedProcess> {
    spawn_process_with_stdin_mode(program, args, cwd, env, arg0, PipeStdinMode::Piped).await
}

/// Spawn a process using regular pipes, but close stdin immediately.
pub async fn spawn_process_no_stdin(
    program: &str,
    args: &[String],
    cwd: &Path,
    env: &HashMap<String, String>,
    arg0: &Option<String>,
) -> Result<SpawnedProcess> {
    spawn_process_with_stdin_mode(program, args, cwd, env, arg0, PipeStdinMode::Null).await
}
