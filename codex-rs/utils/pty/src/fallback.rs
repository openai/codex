use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::process::Command as StdCommand;
use std::process::Stdio;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::Duration;

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

    let child = Arc::new(StdMutex::new(child));
    let (exit_tx, exit_rx) = oneshot::channel::<i32>();
    let exit_status = Arc::new(AtomicBool::new(false));
    let wait_exit_status = Arc::clone(&exit_status);
    let exit_code = Arc::new(StdMutex::new(None));
    let wait_exit_code = Arc::clone(&exit_code);
    let wait_child = Arc::clone(&child);
    let wait_handle: JoinHandle<()> = tokio::task::spawn_blocking(move || {
        let code = loop {
            let status = match wait_child.lock() {
                Ok(mut guard) => guard.try_wait(),
                Err(_) => break -1,
            };
            match status {
                Ok(Some(status)) => {
                    break status
                        .code()
                        .unwrap_or_else(|| if status.success() { 0 } else { 1 });
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                Err(_) => break -1,
            }
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
        Box::new(PipedChildKiller::new(child)),
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
    child: Arc<StdMutex<std::process::Child>>,
}

impl PipedChildKiller {
    fn new(child: Arc<StdMutex<std::process::Child>>) -> Self {
        Self { child }
    }
}

impl portable_pty::ChildKiller for PipedChildKiller {
    fn kill(&mut self) -> io::Result<()> {
        if let Ok(mut guard) = self.child.try_lock() {
            return guard.kill();
        }

        let child = Arc::clone(&self.child);
        std::thread::spawn(move || {
            if let Ok(mut guard) = child.lock() {
                let _ = guard.kill();
            }
        });
        Ok(())
    }

    fn clone_killer(&self) -> Box<dyn portable_pty::ChildKiller + Send + Sync> {
        Box::new(Self {
            child: Arc::clone(&self.child),
        })
    }
}
