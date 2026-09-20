//! Bounded mono F32LE writes for the native sink, never called from a device callback.
//! Each writer belongs to one speaker epoch. Reset cancels it before locking the producer.

use super::buffers::BLOCK;
use super::buffers::Buffers;
use super::buffers::Frame;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

pub(super) struct PlaybackState {
    pub(super) producer: Mutex<()>,
    buffers: Arc<Buffers>,
    rate: u32,
}

pub(super) struct PlaybackPort(pub(super) Arc<PlaybackState>);

pub(crate) struct PlaybackWriter {
    state: Arc<PlaybackState>,
    epoch: u64,
}

impl PlaybackPort {
    pub(super) fn new(buffers: Arc<Buffers>, rate: u32) -> Self {
        Self(Arc::new(PlaybackState {
            producer: Mutex::new(()),
            buffers,
            rate,
        }))
    }

    pub(super) fn writer(&self) -> PlaybackWriter {
        PlaybackWriter {
            state: self.0.clone(),
            epoch: self.0.buffers.speaker.load(Ordering::Acquire),
        }
    }
}

impl PlaybackWriter {
    pub(crate) fn fail_if_current(&self) {
        let buffers = &self.state.buffers;
        let _transition = buffers
            .speaker_failure_gate
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if buffers.speaker.load(Ordering::Acquire) == self.epoch {
            buffers.failed.store(true, Ordering::Release);
        }
    }

    pub(crate) fn rate(&self) -> u32 {
        self.state.rate
    }

    pub(crate) fn write(&self, bytes: &[u8]) -> Result<usize, &'static str> {
        if bytes.is_empty() || !bytes.len().is_multiple_of(4) {
            return Err("invalid speaker buffer");
        }
        let mut frame = Frame {
            samples: [0.0; BLOCK],
            len: (bytes.len() / 4).min(BLOCK),
            at: Instant::now(),
            generation: self.epoch,
        };
        for (sample, bytes) in frame.samples.iter_mut().zip(bytes.chunks_exact(4)) {
            *sample = f32::from_le_bytes(bytes.try_into().map_err(|_| "invalid speaker buffer")?);
            if !sample.is_finite() {
                return Err("invalid speaker sample");
            }
        }
        let _producer = self
            .state
            .producer
            .lock()
            .map_err(|_| "speaker writer failed")?;
        let deadline = Instant::now() + Duration::from_millis(/*millis*/ 100);
        let buffers = &self.state.buffers;
        // Linux requests 50 ms callbacks; retain two callbacks of audio so a
        // callback can fill without racing the producer. Keep the queue cap.
        let windows_per_second = if cfg!(target_os = "linux") { 10 } else { 25 };
        let limit = (self.state.rate / windows_per_second)
            .min((BLOCK * buffers.playback.capacity()) as u32);
        loop {
            if self.epoch % 2 == 1
                || buffers.speaker.load(Ordering::Acquire) != self.epoch
                || buffers.failed.load(Ordering::Acquire)
            {
                return Err("speaker writer cancelled");
            }
            if buffers.queued.load(Ordering::Acquire) + frame.len as u32 <= limit
                && !buffers.playback.is_full()
            {
                let bytes = frame.len * 4;
                buffers
                    .push_playback(frame)
                    .map_err(|_| "speaker queue failed")?;
                return Ok(bytes);
            }
            if Instant::now() >= deadline {
                // The sink must consume this chunk to catch up with the device clock.
                // Dropping it keeps the bounded queue and current epoch intact.
                return Ok(frame.len * 4);
            }
            std::thread::park_timeout(Duration::from_millis(/*millis*/ 1));
        }
    }
}

#[cfg(test)]
#[path = "playback_tests.rs"]
mod tests;
