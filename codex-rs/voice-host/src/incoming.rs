//! Takes incoming Opus RTP before the upstream track queue, preserving network arrival time.
//! Packet and byte permits follow owned data through the consumer, not merely through this queue.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use rtc::interceptor::Interceptor;
use rtc::interceptor::NoopInterceptor;
use rtc::interceptor::Packet;
use rtc::interceptor::StreamInfo;
use rtc::interceptor::TaggedPacket;
use rtc::interceptor::interceptor;
use rtc::sansio;
use rtc::shared::error::Error;
use rtc::shared::marshal::Marshal;
use rtc::shared::marshal::MarshalSize;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::Semaphore;
use tokio::sync::mpsc;

const PACKETS: usize = 64;
const BYTES: usize = 2 * 1024 * 1024;
const PACKET_BYTES: usize = 64 * 1024;

struct State {
    epoch: AtomicU64,
    failed: AtomicBool,
    packets: Arc<Semaphore>,
    bytes: Arc<Semaphore>,
}

pub(crate) struct ReceivedRtp {
    data: Vec<u8>,
    pub(crate) at: Instant,
    epoch: u64,
    _packet: OwnedSemaphorePermit,
    _bytes: OwnedSemaphorePermit,
}

impl AsRef<[u8]> for ReceivedRtp {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}

pub(crate) struct Incoming {
    state: Arc<State>,
    receiver: mpsc::Receiver<ReceivedRtp>,
}

#[derive(Interceptor)]
pub(crate) struct Ingress {
    #[next]
    next: NoopInterceptor,
    state: Arc<State>,
    sender: mpsc::Sender<ReceivedRtp>,
    stream: Option<u32>,
}

impl Incoming {
    pub(crate) fn new() -> (Self, Ingress) {
        let state = Arc::new(State {
            epoch: AtomicU64::new(/*v*/ 1),
            failed: AtomicBool::new(false),
            packets: Arc::new(Semaphore::new(/*permits*/ PACKETS)),
            bytes: Arc::new(Semaphore::new(/*permits*/ BYTES)),
        });
        let (sender, receiver) = mpsc::channel(/*buffer*/ PACKETS);
        (
            Self {
                state: state.clone(),
                receiver,
            },
            Ingress {
                next: NoopInterceptor::new(),
                state,
                sender,
                stream: None,
            },
        )
    }

    pub(crate) fn set_suppressed(&self, suppressed: bool) -> Result<(), &'static str> {
        let epoch = self.state.epoch.load(Ordering::Acquire);
        if (epoch % 2 == 1) != suppressed {
            self.state.epoch.store(
                epoch.checked_add(1).ok_or("audio epoch exhausted")?,
                Ordering::Release,
            );
        }
        Ok(())
    }

    pub(crate) fn take(&mut self) -> Result<Option<ReceivedRtp>, &'static str> {
        if self.state.failed.load(Ordering::Acquire) {
            return Err("incoming audio failed");
        }
        let epoch = self.state.epoch.load(Ordering::Acquire);
        for _ in 0..PACKETS {
            let Ok(packet) = self.receiver.try_recv() else {
                return Ok(None);
            };
            if epoch.is_multiple_of(2) && packet.epoch == epoch {
                if packet.at.elapsed() > Duration::from_secs(/*secs*/ 1) {
                    return Err("incoming audio fell behind");
                }
                return Ok(Some(packet));
            }
        }
        Ok(None)
    }
}

#[interceptor]
impl Ingress {
    #[overrides]
    fn handle_read(&mut self, message: TaggedPacket) -> Result<(), Self::Error> {
        let Packet::Rtp(packet) = message.message else {
            return Ok(());
        };
        let epoch = self.state.epoch.load(Ordering::Acquire);
        if epoch % 2 == 1 || self.state.failed.load(Ordering::Acquire) {
            return Ok(());
        }
        let size = packet.marshal_size();
        let accepted = (|| {
            if size > PACKET_BYTES
                || packet.header.payload_type != crate::audio_track::OPUS_PAYLOAD_TYPE
                || self.stream.is_some_and(|ssrc| ssrc != packet.header.ssrc)
            {
                return None;
            }
            self.stream = Some(packet.header.ssrc);
            let packet_permit = self.state.packets.clone().try_acquire_owned().ok()?;
            let bytes_permit = self
                .state
                .bytes
                .clone()
                .try_acquire_many_owned(size as u32)
                .ok()?;
            let mut data = vec![0; size];
            if packet.marshal_to(&mut data).ok()? != size {
                return None;
            }
            self.sender
                .try_send(ReceivedRtp {
                    data,
                    at: message.now,
                    epoch,
                    _packet: packet_permit,
                    _bytes: bytes_permit,
                })
                .ok()
        })();
        if accepted.is_none() {
            self.state.failed.store(true, Ordering::Release);
        }
        // Consume here: no second copy is retained by the upstream track-event queue.
        Ok(())
    }
}

#[cfg(test)]
#[path = "incoming_tests.rs"]
mod tests;
