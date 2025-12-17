use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::process::Command as StdCommand;
use std::process::Stdio;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use anyhow::Result;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use super::ExecCommandSession;
use super::SpawnedPty;

pub async fn spawn_piped_process(
    program: &str,
    args: &[String],
    cwd: &Path,
    env: &HashMap<String, String>,
    arg0: &Option<String>,
) -> Result<SpawnedPty> {
    if program.is_empty() {
        anyhow::bail!("missing program for exec spawn");
    }

    let program = arg0.as_deref().unwrap_or(program);
    let mut command = StdCommand::new(program);
    command.args(args);
    command.current_dir(cwd);
    command.env_clear();
    command.envs(env);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command.spawn()?;
    let pid = child.id();

    let stdin = child.stdin.take().ok_or_else(|| {
        anyhow::anyhow!("stdin pipe was unexpectedly not available for exec spawn")
    })?;
    let stdout = child.stdout.take().ok_or_else(|| {
        anyhow::anyhow!("stdout pipe was unexpectedly not available for exec spawn")
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        anyhow::anyhow!("stderr pipe was unexpectedly not available for exec spawn")
    })?;

    let (writer_tx, mut writer_rx) = mpsc::channel::<Vec<u8>>(128);
    let (output_tx, _) = broadcast::channel::<Vec<u8>>(256);
    let initial_output_rx = output_tx.subscribe();

    // Pipes separate stdout and stderr; merge to match PTY semantics.
    let stdout_handle = spawn_pipe_reader(stdout, output_tx.clone());
    let stderr_handle = spawn_pipe_reader(stderr, output_tx.clone());

    let writer_handle = tokio::task::spawn_blocking(move || {
        let mut stdin = stdin;
        use std::io::Write;
        while let Some(bytes) = writer_rx.blocking_recv() {
            let _ = stdin.write_all(&bytes);
            let _ = stdin.flush();
        }
    });

    let (exit_tx, exit_rx) = oneshot::channel::<i32>();
    let exit_status = Arc::new(AtomicBool::new(false));
    let wait_exit_status = Arc::clone(&exit_status);
    let exit_code = Arc::new(StdMutex::new(None));
    let wait_exit_code = Arc::clone(&exit_code);
    let wait_handle: JoinHandle<()> = tokio::task::spawn_blocking(move || {
        let code = match child.wait() {
            Ok(status) => status.code().unwrap_or(-1),
            Err(_) => -1,
        };
        wait_exit_status.store(true, std::sync::atomic::Ordering::SeqCst);
        if let Ok(mut guard) = wait_exit_code.lock() {
            *guard = Some(code);
        }
        let _ = exit_tx.send(code);
    });

    let (session, output_rx) = ExecCommandSession::new(
        writer_tx,
        output_tx,
        initial_output_rx,
        Box::new(PipedChildKiller::new(pid)),
        vec![stdout_handle, stderr_handle],
        writer_handle,
        wait_handle,
        exit_status,
        exit_code,
        None,
    );

    Ok(SpawnedPty {
        session,
        output_rx,
        exit_rx,
    })
}

fn spawn_pipe_reader<R: std::io::Read + Send + 'static>(
    mut reader: R,
    output_tx: broadcast::Sender<Vec<u8>>,
) -> JoinHandle<()> {
    tokio::task::spawn_blocking(move || {
        let mut buf = [0u8; 8_192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let _ = output_tx.send(buf[..n].to_vec());
                }
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }
    })
}

#[derive(Debug)]
struct PipedChildKiller {
    pid: u32,
}

impl PipedChildKiller {
    fn new(pid: u32) -> Self {
        Self { pid }
    }
}

impl portable_pty::ChildKiller for PipedChildKiller {
    fn kill(&mut self) -> io::Result<()> {
        terminate_pid(self.pid)
    }

    fn clone_killer(&self) -> Box<dyn portable_pty::ChildKiller + Send + Sync> {
        Box::new(Self { pid: self.pid })
    }
}

#[cfg(unix)]
fn terminate_pid(pid: u32) -> io::Result<()> {
    let result = unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(windows)]
fn terminate_pid(pid: u32) -> io::Result<()> {
    use winapi::shared::minwindef::FALSE;
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::processthreadsapi::OpenProcess;
    use winapi::um::processthreadsapi::TerminateProcess;
    use winapi::um::winnt::PROCESS_TERMINATE;

    unsafe {
        let handle = OpenProcess(PROCESS_TERMINATE, FALSE, pid);
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let ok = TerminateProcess(handle, 1) != 0;
        let err = io::Error::last_os_error();
        CloseHandle(handle);
        if ok {
            Ok(())
        } else {
            Err(err)
        }
    }
}
