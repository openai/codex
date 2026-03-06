use core::fmt;
use std::io;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use anyhow::anyhow;
use portable_pty::MasterPty;
use portable_pty::PtySize;
use portable_pty::SlavePty;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::task::AbortHandle;
use tokio::task::JoinHandle;

pub(crate) trait ChildTerminator: Send + Sync {
    fn kill(&mut self) -> io::Result<()>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalSize {
    pub rows: u16,
    pub cols: u16,
}

impl Default for TerminalSize {
    fn default() -> Self {
        Self { rows: 24, cols: 80 }
    }
}

impl From<TerminalSize> for PtySize {
    fn from(value: TerminalSize) -> Self {
        Self {
            rows: value.rows,
            cols: value.cols,
            pixel_width: 0,
            pixel_height: 0,
        }
    }
}

pub struct PtyHandles {
    pub _slave: Option<Box<dyn SlavePty + Send>>,
    pub _master: Box<dyn MasterPty + Send>,
}

impl fmt::Debug for PtyHandles {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PtyHandles").finish()
    }
}

/// Handle for driving an interactive process (PTY or pipe).
pub struct ProcessHandle {
    writer_tx: StdMutex<Option<mpsc::Sender<Vec<u8>>>>,
    output_tx: Option<broadcast::Sender<Vec<u8>>>,
    killer: StdMutex<Option<Box<dyn ChildTerminator>>>,
    #[allow(dead_code)]
    reader_handle: StdMutex<Option<JoinHandle<()>>>,
    #[allow(dead_code)]
    reader_abort_handles: StdMutex<Vec<AbortHandle>>,
    #[allow(dead_code)]
    writer_handle: StdMutex<Option<JoinHandle<()>>>,
    wait_handle: StdMutex<Option<JoinHandle<()>>>,
    exit_status: Arc<AtomicBool>,
    exit_code: Arc<StdMutex<Option<i32>>>,
    // PtyHandles must be preserved because the process will receive Control+C if the
    // slave is closed
    _pty_handles: StdMutex<Option<PtyHandles>>,
}

impl fmt::Debug for ProcessHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProcessHandle").finish()
    }
}

impl ProcessHandle {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        writer_tx: mpsc::Sender<Vec<u8>>,
        output_tx: Option<broadcast::Sender<Vec<u8>>>,
        killer: Box<dyn ChildTerminator>,
        reader_handle: JoinHandle<()>,
        reader_abort_handles: Vec<AbortHandle>,
        writer_handle: JoinHandle<()>,
        wait_handle: JoinHandle<()>,
        exit_status: Arc<AtomicBool>,
        exit_code: Arc<StdMutex<Option<i32>>>,
        pty_handles: Option<PtyHandles>,
    ) -> Self {
        Self {
            writer_tx: StdMutex::new(Some(writer_tx)),
            output_tx,
            killer: StdMutex::new(Some(killer)),
            reader_handle: StdMutex::new(Some(reader_handle)),
            reader_abort_handles: StdMutex::new(reader_abort_handles),
            writer_handle: StdMutex::new(Some(writer_handle)),
            wait_handle: StdMutex::new(Some(wait_handle)),
            exit_status,
            exit_code,
            _pty_handles: StdMutex::new(pty_handles),
        }
    }

    /// Returns a channel sender for writing raw bytes to the child stdin.
    pub fn writer_sender(&self) -> mpsc::Sender<Vec<u8>> {
        if let Ok(writer_tx) = self.writer_tx.lock() {
            if let Some(writer_tx) = writer_tx.as_ref() {
                return writer_tx.clone();
            }
        }

        let (writer_tx, writer_rx) = mpsc::channel(1);
        drop(writer_rx);
        writer_tx
    }

    /// Returns a broadcast receiver that yields stdout/stderr chunks when
    /// combined output routing is configured.
    pub fn output_receiver(&self) -> broadcast::Receiver<Vec<u8>> {
        if let Some(output_tx) = self.output_tx.as_ref() {
            return output_tx.subscribe();
        }

        let (output_tx, output_rx) = broadcast::channel(1);
        drop(output_tx);
        output_rx
    }

    /// True if the child process has exited.
    pub fn has_exited(&self) -> bool {
        self.exit_status.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Returns the exit code if known.
    pub fn exit_code(&self) -> Option<i32> {
        self.exit_code.lock().ok().and_then(|guard| *guard)
    }

    /// Resize the PTY in character cells.
    pub fn resize(&self, size: TerminalSize) -> anyhow::Result<()> {
        let handles = self
            ._pty_handles
            .lock()
            .map_err(|_| anyhow!("failed to lock PTY handles"))?;
        let handles = handles
            .as_ref()
            .ok_or_else(|| anyhow!("process is not attached to a PTY"))?;
        handles._master.resize(size.into())
    }

    /// Close the child's stdin channel.
    pub fn close_stdin(&self) {
        if let Ok(mut writer_tx) = self.writer_tx.lock() {
            writer_tx.take();
        }
    }

    /// Attempts to kill the child while leaving the reader/writer tasks alive
    /// so callers can still drain output until EOF.
    pub fn request_terminate(&self) {
        if let Ok(mut killer_opt) = self.killer.lock() {
            if let Some(mut killer) = killer_opt.take() {
                let _ = killer.kill();
            }
        }
    }

    /// Attempts to kill the child and abort helper tasks.
    pub fn terminate(&self) {
        self.request_terminate();

        if let Ok(mut h) = self.reader_handle.lock() {
            if let Some(handle) = h.take() {
                handle.abort();
            }
        }
        if let Ok(mut handles) = self.reader_abort_handles.lock() {
            for handle in handles.drain(..) {
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

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        self.terminate();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OutputStream {
    Stdout,
    Stderr,
}

#[derive(Clone, Debug)]
pub enum OutputSink {
    BroadcastCombined(broadcast::Sender<Vec<u8>>),
    GuaranteedSeparate {
        stdout: mpsc::Sender<Vec<u8>>,
        stderr: mpsc::Sender<Vec<u8>>,
    },
}

impl OutputSink {
    pub fn broadcast_combined() -> (Self, broadcast::Receiver<Vec<u8>>) {
        let (tx, rx) = broadcast::channel(256);
        (OutputSink::BroadcastCombined(tx), rx)
    }

    pub fn guaranteed_separate() -> (Self, mpsc::Receiver<Vec<u8>>, mpsc::Receiver<Vec<u8>>) {
        let (stdout_tx, stdout_rx) = mpsc::channel(128);
        let (stderr_tx, stderr_rx) = mpsc::channel(128);
        (
            OutputSink::GuaranteedSeparate {
                stdout: stdout_tx,
                stderr: stderr_tx,
            },
            stdout_rx,
            stderr_rx,
        )
    }

    pub(crate) async fn send(&self, chunk: Vec<u8>, stream: OutputStream) {
        match self {
            OutputSink::BroadcastCombined(tx) => {
                let _ = tx.send(chunk);
            }
            OutputSink::GuaranteedSeparate { stdout, stderr } => match stream {
                OutputStream::Stdout => {
                    let _ = stdout.send(chunk).await;
                }
                OutputStream::Stderr => {
                    let _ = stderr.send(chunk).await;
                }
            },
        }
    }

    pub(crate) fn send_blocking(&self, chunk: Vec<u8>, stream: OutputStream) {
        match self {
            OutputSink::BroadcastCombined(tx) => {
                let _ = tx.send(chunk);
            }
            OutputSink::GuaranteedSeparate { stdout, stderr } => match stream {
                OutputStream::Stdout => {
                    let _ = stdout.blocking_send(chunk);
                }
                OutputStream::Stderr => {
                    let _ = stderr.blocking_send(chunk);
                }
            },
        }
    }

    pub(crate) fn combined_sender(&self) -> Option<broadcast::Sender<Vec<u8>>> {
        match self {
            OutputSink::BroadcastCombined(tx) => Some(tx.clone()),
            OutputSink::GuaranteedSeparate { .. } => None,
        }
    }
}

/// Return value from explicit streaming spawn helpers (PTY or pipe).
#[derive(Debug)]
pub struct SpawnedStreamingProcess {
    pub session: ProcessHandle,
    pub exit_rx: oneshot::Receiver<i32>,
}

impl SpawnedStreamingProcess {
    pub(crate) fn into_spawned_process(
        self,
        output_rx: broadcast::Receiver<Vec<u8>>,
    ) -> SpawnedProcess {
        let Self { session, exit_rx } = self;
        SpawnedProcess {
            session,
            output_rx,
            exit_rx,
        }
    }
}

/// Return value from backwards-compatible spawn helpers (PTY or pipe).
#[derive(Debug)]
pub struct SpawnedProcess {
    pub session: ProcessHandle,
    pub output_rx: broadcast::Receiver<Vec<u8>>,
    pub exit_rx: oneshot::Receiver<i32>,
}
