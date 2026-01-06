use std::collections::HashMap;
use std::io;
use std::io::ErrorKind;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use anyhow::Result;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::process::Command;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tokio::time::Duration as TokioDuration;

use crate::process::ChildTerminator;
use crate::process::ProcessHandle;
use crate::process::SpawnedProcess;

#[cfg(unix)]
use libc;

struct PipeChildTerminator {
    pid: u32,
}

impl ChildTerminator for PipeChildTerminator {
    fn kill(&mut self) -> io::Result<()> {
        kill_process(self.pid)
    }
}

#[cfg(unix)]
fn kill_process(pid: u32) -> io::Result<()> {
    let result = unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
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

/// Spawn a process using regular pipes (no PTY), returning handles for stdin, output, and exit.
pub async fn spawn_process(
    program: &str,
    args: &[String],
    cwd: &Path,
    env: &HashMap<String, String>,
    arg0: &Option<String>,
) -> Result<SpawnedProcess> {
    if program.is_empty() {
        anyhow::bail!("missing program for pipe spawn");
    }

    let mut command = Command::new(program);
    if let Some(arg0) = arg0 {
        command.arg0(arg0);
    }
    command.current_dir(cwd);
    command.env_clear();
    for (key, value) in env {
        command.env(key, value);
    }
    for arg in args {
        command.arg(arg);
    }
    command.stdin(Stdio::piped());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let mut child = command.spawn()?;
    let pid = child
        .id()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "missing child pid"))?;

    let stdin = child.stdin.take();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let (writer_tx, mut writer_rx) = mpsc::channel::<Vec<u8>>(128);
    let (output_tx, _) = broadcast::channel::<Vec<u8>>(256);
    let initial_output_rx = output_tx.subscribe();

    let writer_handle = tokio::spawn({
        let writer = stdin.map(|w| Arc::new(tokio::sync::Mutex::new(w)));
        async move {
            while let Some(bytes) = writer_rx.recv().await {
                if let Some(writer) = &writer {
                    let mut guard = writer.lock().await;
                    let _ = guard.write_all(&bytes).await;
                    let _ = guard.flush().await;
                }
            }
        }
    });

    let output_tx_clone = output_tx.clone();
    let reader_handle = tokio::spawn(async move {
        let mut stdout_reader = stdout.map(BufReader::new);
        let mut stderr_reader = stderr.map(BufReader::new);
        let mut stdout_buf = vec![0u8; 8_192];
        let mut stderr_buf = vec![0u8; 8_192];

        loop {
            let mut progressed = false;

            if let Some(reader) = stdout_reader.as_mut() {
                match reader.read(&mut stdout_buf).await {
                    Ok(0) => stdout_reader = None,
                    Ok(n) => {
                        progressed = true;
                        let _ = output_tx_clone.send(stdout_buf[..n].to_vec());
                    }
                    Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                    Err(_) => stdout_reader = None,
                }
            }

            if let Some(reader) = stderr_reader.as_mut() {
                match reader.read(&mut stderr_buf).await {
                    Ok(0) => stderr_reader = None,
                    Ok(n) => {
                        progressed = true;
                        let _ = output_tx_clone.send(stderr_buf[..n].to_vec());
                    }
                    Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                    Err(_) => stderr_reader = None,
                }
            }

            if stdout_reader.is_none() && stderr_reader.is_none() {
                break;
            }

            if !progressed {
                sleep(TokioDuration::from_millis(5)).await;
            }
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
        Box::new(PipeChildTerminator { pid }),
        reader_handle,
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
