use std::sync::Mutex as StdMutex;

use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

#[derive(Debug)]
pub(crate) struct ExecCommandSession {
    /// Queue for writing bytes to the process stdin (PTY master write side).
    writer_tx: mpsc::Sender<Vec<u8>>,
    /// Broadcast stream of output chunks read from the PTY. New subscribers
    /// receive only chunks emitted after they subscribe.
    output_tx: broadcast::Sender<Vec<u8>>,

    /// Child killer handle for termination on drop (can signal independently
    /// of a thread blocked in `.wait()`).
    killer: StdMutex<Option<Box<dyn portable_pty::ChildKiller + Send + Sync>>>,

    /// JoinHandle for the blocking PTY reader task.
    reader_handle: StdMutex<Option<JoinHandle<()>>>,

    /// JoinHandle for the stdin writer task.
    writer_handle: StdMutex<Option<JoinHandle<()>>>,

    /// JoinHandle for the child wait task.
    wait_handle: StdMutex<Option<JoinHandle<()>>>,

    /// Tracks whether the underlying process has exited.
    exit_status: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl ExecCommandSession {
    pub(crate) fn new(
        writer_tx: mpsc::Sender<Vec<u8>>,
        output_tx: broadcast::Sender<Vec<u8>>,
        killer: Box<dyn portable_pty::ChildKiller + Send + Sync>,
        reader_handle: JoinHandle<()>,
        writer_handle: JoinHandle<()>,
        wait_handle: JoinHandle<()>,
        exit_status: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> (Self, broadcast::Receiver<Vec<u8>>) {
        let initial_output_rx = output_tx.subscribe();
        (
            Self {
                writer_tx,
                output_tx,
                killer: StdMutex::new(Some(killer)),
                reader_handle: StdMutex::new(Some(reader_handle)),
                writer_handle: StdMutex::new(Some(writer_handle)),
                wait_handle: StdMutex::new(Some(wait_handle)),
                exit_status,
            },
            initial_output_rx,
        )
    }

    pub(crate) fn writer_sender(&self) -> mpsc::Sender<Vec<u8>> {
        self.writer_tx.clone()
    }

    pub(crate) fn output_receiver(&self) -> broadcast::Receiver<Vec<u8>> {
        self.output_tx.subscribe()
    }

    pub(crate) fn has_exited(&self) -> bool {
        self.exit_status.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl Drop for ExecCommandSession {
    fn drop(&mut self) {
        // Best-effort: terminate child first so blocking tasks can complete.
        if let Ok(mut killer_opt) = self.killer.lock()
            && let Some(mut killer) = killer_opt.take()
        {
            let _ = killer.kill();
        }

        // Abort background tasks; they may already have exited after kill.
        if let Ok(mut h) = self.reader_handle.lock()
            && let Some(handle) = h.take()
        {
            handle.abort();
        }
        if let Ok(mut h) = self.writer_handle.lock()
            && let Some(handle) = h.take()
        {
            handle.abort();
        }
        if let Ok(mut h) = self.wait_handle.lock()
            && let Some(handle) = h.take()
        {
            handle.abort();
        }
    }
}
