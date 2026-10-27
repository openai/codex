//! Framed I/O and per-instance queue admission for the plugin child.

use codex_plugin_host::framing;
use codex_plugin_host::protocol::MAX_IN_FLIGHT_TURNS;
use codex_plugin_host::protocol::Packet;
use codex_plugin_host::protocol::RuntimeMessage;
use futures::io::AllowStdIo;
use std::io;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::Semaphore;
use tokio_util::codec::FramedRead;
use tokio_util::codec::FramedWrite;
use tokio_util::codec::LengthDelimitedCodec;
use tokio_util::compat::Compat;
use tokio_util::compat::FuturesAsyncReadCompatExt;
use tokio_util::compat::FuturesAsyncWriteCompatExt;

// A reply may reach the parent just before its writer releases the output slot.
pub(super) const OUTPUT_SLOTS_PER_INSTANCE: usize = MAX_IN_FLIGHT_TURNS + 1;

/// Only poll on a dedicated blocking thread: AllowStdIo blocks inside poll_read.
pub(super) fn blocking_reader<R: io::Read>(
    input: R,
) -> FramedRead<Compat<AllowStdIo<R>>, LengthDelimitedCodec> {
    FramedRead::new(AllowStdIo::new(input).compat(), framing::codec())
}

/// Only poll on a dedicated blocking thread: AllowStdIo blocks inside poll_write.
pub(super) fn blocking_writer<W: io::Write>(
    output: W,
) -> FramedWrite<Compat<AllowStdIo<W>>, LengthDelimitedCodec> {
    FramedWrite::new(AllowStdIo::new(output).compat_write(), framing::codec())
}

pub(super) struct Instance {
    pub(super) id: u64,
    pub(super) retired: Arc<AtomicBool>,
    pub(super) close_output: OnceLock<(mpsc::SyncSender<QueuedOutput>, QueuedOutput)>,
    pub(super) closed: AtomicBool,
    pub(super) input_slots: Arc<Semaphore>,
    output_slots: Arc<Semaphore>,
}

impl Instance {
    pub(super) fn new(id: u64) -> Self {
        Self {
            id,
            retired: Arc::new(AtomicBool::new(false)),
            close_output: OnceLock::new(),
            closed: AtomicBool::new(false),
            input_slots: Arc::new(Semaphore::new(MAX_IN_FLIGHT_TURNS)),
            output_slots: Arc::new(Semaphore::new(OUTPUT_SLOTS_PER_INSTANCE)),
        }
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        // The engine, queued turns and earlier output have all released this instance.
        if let Some((outgoing_tx, output)) = self.close_output.take()
            && let Err(error) = outgoing_tx.try_send(output)
        {
            super::workers::fatal(io::Error::other(error));
        }
    }
}

pub(super) enum OutputReservation {
    Turn {
        _instance: Arc<Instance>,
        _permit: OwnedSemaphorePermit,
    },
    Close {
        retired: Arc<AtomicBool>,
    },
    Rejected {
        _permit: OwnedSemaphorePermit,
    },
}

pub(super) struct QueuedOutput {
    pub(super) packets: Vec<Packet<RuntimeMessage>>,
    pub(super) _reservation: OutputReservation,
}

pub(super) struct Transport {
    pub(super) instance: Arc<Instance>,
    pub(super) outgoing_tx: mpsc::SyncSender<QueuedOutput>,
}

impl Transport {
    pub(super) fn ensure_open(&self) -> io::Result<()> {
        if self.instance.closed.load(Ordering::Acquire) {
            return Err(io::Error::other("plugin instance closed"));
        }
        Ok(())
    }

    pub(super) fn write(&self, messages: Vec<RuntimeMessage>) -> io::Result<()> {
        self.ensure_open()?;
        if messages.is_empty() {
            return Ok(());
        }
        let permit = Arc::clone(&self.instance.output_slots)
            .try_acquire_owned()
            .map_err(|_| io::Error::other("plugin instance output full"))?;
        let packets = messages
            .into_iter()
            .map(|message| Packet {
                instance: self.instance.id,
                message,
            })
            .collect();
        self.outgoing_tx
            .try_send(QueuedOutput {
                packets,
                _reservation: OutputReservation::Turn {
                    _instance: Arc::clone(&self.instance),
                    _permit: permit,
                },
            })
            .map_err(io::Error::other)
    }
}

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;
