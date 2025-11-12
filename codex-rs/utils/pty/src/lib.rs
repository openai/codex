use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::Duration;

use anyhow::Result;
use portable_pty::native_pty_system;
use portable_pty::CommandBuilder;
use portable_pty::PtySize;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::Mutex as TokioMutex;
use tokio::task::JoinHandle;

pub struct ExecCommandSession {
    master: Box<dyn portable_pty::MasterPty + Send>,
    writer_tx: mpsc::Sender<Vec<u8>>,
    output_tx: broadcast::Sender<Vec<u8>>,
    killer: StdMutex<Option<Box<dyn portable_pty::ChildKiller + Send + Sync>>>,
    reader_handle: StdMutex<Option<JoinHandle<()>>>,
    writer_handle: StdMutex<Option<JoinHandle<()>>>,
    wait_handle: StdMutex<Option<JoinHandle<()>>>,
    exit_status: Arc<AtomicBool>,
    exit_code: Arc<StdMutex<Option<i32>>>,
}

impl std::fmt::Debug for ExecCommandSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecCommandSession")
            .field("exit_status", &self.exit_status)
            .field("exit_code", &self.exit_code)
            .finish()
    }
}

impl ExecCommandSession {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        master: Box<dyn portable_pty::MasterPty + Send>,
        writer_tx: mpsc::Sender<Vec<u8>>,
        output_tx: broadcast::Sender<Vec<u8>>,
        killer: Box<dyn portable_pty::ChildKiller + Send + Sync>,
        reader_handle: JoinHandle<()>,
        writer_handle: JoinHandle<()>,
        wait_handle: JoinHandle<()>,
        exit_status: Arc<AtomicBool>,
        exit_code: Arc<StdMutex<Option<i32>>>,
    ) -> (Self, broadcast::Receiver<Vec<u8>>) {
        let initial_output_rx = output_tx.subscribe();
        (
            Self {
                master,
                writer_tx,
                output_tx,
                killer: StdMutex::new(Some(killer)),
                reader_handle: StdMutex::new(Some(reader_handle)),
                writer_handle: StdMutex::new(Some(writer_handle)),
                wait_handle: StdMutex::new(Some(wait_handle)),
                exit_status,
                exit_code,
            },
            initial_output_rx,
        )
    }

    pub fn writer_sender(&self) -> mpsc::Sender<Vec<u8>> {
        self.writer_tx.clone()
    }

    pub fn output_receiver(&self) -> broadcast::Receiver<Vec<u8>> {
        self.output_tx.subscribe()
    }

    pub fn has_exited(&self) -> bool {
        self.exit_status.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn exit_code(&self) -> Option<i32> {
        self.exit_code.lock().ok().and_then(|guard| *guard)
    }
}

impl Drop for ExecCommandSession {
    fn drop(&mut self) {
        if let Ok(mut killer_opt) = self.killer.lock() {
            if let Some(mut killer) = killer_opt.take() {
                let _ = killer.kill();
            }
        }

        if let Ok(mut h) = self.reader_handle.lock() {
            if let Some(handle) = h.take() {
                handle.abort();
            }
        }
        if let Ok(mut h) = self.writer_handle.lock() {
            if let Some(handle) = h.take() {
                handle.abort();
            }
        }
        if let Ok(mut h) = self.wait_handle.lock() {
            if let Some(handle) = h.take() {
                handle.abort();
            }
        }
    }
}

#[derive(Debug)]
pub struct SpawnedPty {
    pub session: ExecCommandSession,
    pub output_rx: broadcast::Receiver<Vec<u8>>,
    pub exit_rx: oneshot::Receiver<i32>,
}

pub async fn spawn_pty_process(
    program: &str,
    args: &[String],
    cwd: &Path,
    env: &HashMap<String, String>,
    arg0: &Option<String>,
) -> Result<SpawnedPty> {
    if program.is_empty() {
        anyhow::bail!("missing program for PTY spawn");
    }

    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    let master = pair.master;
    let mut slave = pair.slave;

    let mut command_builder = CommandBuilder::new(program);
    let _ = arg0;
    command_builder.cwd(cwd);
    #[cfg(not(target_os = "windows"))]
    command_builder.env_clear();
    #[cfg(target_os = "windows")]
    {
        // Keep the inherited Windows environment to avoid missing critical
        // variables that cause console hosts to fail to initialize.
        for (key, value) in std::env::vars() {
            command_builder.env(key, value);
        }
    }
    for arg in args {
        command_builder.arg(arg);
    }
    for (key, value) in env {
        command_builder.env(key, value);
    }

    #[cfg(all(test, target_os = "windows"))]
    eprintln!(
        "spawn_pty_process env keys: {:?}",
        env.keys().cloned().collect::<Vec<_>>()
    );

    #[cfg(target_os = "windows")]
    {
        // Ensure core OS variables are present even if the provided env map
        // was minimized.
        for key in ["SystemRoot", "WINDIR", "COMSPEC", "PATHEXT", "PATH"] {
            if !env.contains_key(key) {
                if let Ok(value) = std::env::var(key) {
                    command_builder.env(key, value);
                }
            }
        }
    }

    let mut child = slave.spawn_command(command_builder)?;
    drop(slave);
    let killer = child.clone_killer();

    let (writer_tx, mut writer_rx) = mpsc::channel::<Vec<u8>>(128);
    let (output_tx, _) = broadcast::channel::<Vec<u8>>(256);

    let mut reader = master.try_clone_reader()?;
    let output_tx_clone = output_tx.clone();
    let reader_handle: JoinHandle<()> = tokio::task::spawn_blocking(move || {
        let mut buf = [0u8; 8_192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let _ = output_tx_clone.send(buf[..n].to_vec());
                }
                Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                }
                Err(_) => break,
            }
        }
    });

    let writer = master.take_writer()?;
    let writer = Arc::new(TokioMutex::new(writer));
    let writer_handle: JoinHandle<()> = tokio::spawn({
        let writer = Arc::clone(&writer);
        async move {
            while let Some(bytes) = writer_rx.recv().await {
                let mut guard = writer.lock().await;
                use std::io::Write;
                let _ = guard.write_all(&bytes);
                let _ = guard.flush();
            }
        }
    });

    let (exit_tx, exit_rx) = oneshot::channel::<i32>();
    let exit_status = Arc::new(AtomicBool::new(false));
    let wait_exit_status = Arc::clone(&exit_status);
    let exit_code = Arc::new(StdMutex::new(None));
    let wait_exit_code = Arc::clone(&exit_code);
    let wait_handle: JoinHandle<()> = tokio::task::spawn_blocking(move || {
        let code = match child.wait() {
            Ok(status) => status.exit_code() as i32,
            Err(_) => -1,
        };
        wait_exit_status.store(true, std::sync::atomic::Ordering::SeqCst);
        if let Ok(mut guard) = wait_exit_code.lock() {
            *guard = Some(code);
        }
        let _ = exit_tx.send(code);
    });

    let (session, output_rx) = ExecCommandSession::new(
        master,
        writer_tx,
        output_tx,
        killer,
        reader_handle,
        writer_handle,
        wait_handle,
        exit_status,
        exit_code,
    );

    Ok(SpawnedPty {
        session,
        output_rx,
        exit_rx,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[tokio::test]
    #[cfg(target_os = "windows")]
    async fn spawn_cmd_succeeds() {
        let mut env: HashMap<String, String> = std::env::vars().collect();
        if let Some(system_root) = env.get("SystemRoot").cloned() {
            let base_paths = vec![
                format!(r"{system_root}\system32"),
                system_root.clone(),
                format!(r"{system_root}\System32\Wbem"),
                format!(r"{system_root}\System32\WindowsPowerShell\v1.0"),
            ];
            env.insert("PATH".to_string(), base_paths.join(";"));
        }
        let cwd = std::env::current_dir().expect("current_dir");
        eprintln!(
            "SystemRoot={:?} ComSpec={:?} PATH={:?}",
            env.get("SystemRoot"),
            env.get("ComSpec"),
            env.get("PATH").map(|p| p.split(';').take(3).collect::<Vec<_>>())
        );

        let comspec = std::env::var("ComSpec").unwrap_or_else(|_| "cmd.exe".to_string());
        let mut spawned = spawn_pty_process(
            &comspec,
            &["/C".to_string(), "exit 0".to_string()],
            &cwd,
            &env,
            &None,
        )
        .await
        .expect("spawn cmd");

        let mut output_rx = spawned.output_rx;
        let first_chunk = output_rx.try_recv().ok();
        eprintln!(
            "first_chunk = {:?}",
            first_chunk
                .as_ref()
                .map(|bytes| String::from_utf8_lossy(bytes))
        );

        let status = spawned.exit_rx.await.expect("exit status");
        assert_eq!(status, 0, "cmd.exe should exit successfully");

        // Drain any output to avoid broadcast warnings.
        while output_rx.try_recv().is_ok() {}
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn spawn_cmd_blocking() {
        let pty_system = native_pty_system();
        let mut pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("open pty");

        let mut cmd = CommandBuilder::new(
            std::env::var("ComSpec").unwrap_or_else(|_| "C:\\windows\\system32\\cmd.exe".into()),
        );
        cmd.arg("/C");
        cmd.arg("exit 0");

        let mut child = pair
            .slave
            .spawn_command(cmd)
            .expect("spawn blocking cmd");
        drop(pair.slave);

        // Explicitly close stdin so the child can exit cleanly.
        drop(pair.master.take_writer().expect("writer"));

        let status = child.wait().expect("wait for child");
        assert_eq!(status.exit_code(), 0, "cmd.exe exit code");
    }
}
