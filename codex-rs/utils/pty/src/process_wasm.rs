use core::fmt;
use std::io;

use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

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

pub struct ProcessHandle {
    writer_tx: mpsc::Sender<Vec<u8>>,
}

impl fmt::Debug for ProcessHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProcessHandle").finish()
    }
}

impl ProcessHandle {
    pub(crate) fn unavailable() -> Self {
        let (writer_tx, writer_rx) = mpsc::channel(1);
        drop(writer_rx);
        Self { writer_tx }
    }

    pub fn writer_sender(&self) -> mpsc::Sender<Vec<u8>> {
        self.writer_tx.clone()
    }

    pub fn has_exited(&self) -> bool {
        true
    }

    pub fn exit_code(&self) -> Option<i32> {
        Some(1)
    }

    pub fn resize(&self, _size: TerminalSize) -> anyhow::Result<()> {
        anyhow::bail!("PTY resize is unavailable on wasm32");
    }

    pub fn close_stdin(&self) {}

    pub fn request_terminate(&self) {}

    pub fn terminate(&self) {}
}

pub fn combine_output_receivers(
    _stdout_rx: mpsc::Receiver<Vec<u8>>,
    _stderr_rx: mpsc::Receiver<Vec<u8>>,
) -> broadcast::Receiver<Vec<u8>> {
    let (tx, rx) = broadcast::channel(1);
    drop(tx);
    rx
}

#[derive(Debug)]
pub struct SpawnedProcess {
    pub session: ProcessHandle,
    pub stdout_rx: mpsc::Receiver<Vec<u8>>,
    pub stderr_rx: mpsc::Receiver<Vec<u8>>,
    pub exit_rx: oneshot::Receiver<i32>,
}

pub type ExecCommandSession = ProcessHandle;
pub type SpawnedPty = SpawnedProcess;

pub(crate) trait ChildTerminator: Send + Sync {
    fn kill(&mut self) -> io::Result<()>;
}
