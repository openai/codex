//! Pumps bounded device queues through capture processing and the outgoing audio track.
//! Devices retain real stream ownership; all queued and partial capture obey mute generations.
//! Retain at most one encoded batch and return between packets so controls can take priority.

use std::collections::VecDeque;
use std::io;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Instant;

use super::MAX_CAPTURE_AGE;
use super::buffers::Buffers;
use super::processing;

pub(super) struct CaptureWorker {
    pub(super) buffers: Arc<Buffers>,
    pub(super) processor: processing::Processor,
    pub(super) pending: VecDeque<crate::audio_track::EncodedAudio>,
}

impl CaptureWorker {
    pub(super) fn set_controls(
        &mut self,
        controls: codex_realtime_webrtc::AudioControls,
    ) -> io::Result<()> {
        let previous = self.buffers.microphone.load(Ordering::Acquire);
        Buffers::set_disabled(&self.buffers.microphone, controls.microphone_muted)?;
        if previous != self.buffers.microphone.load(Ordering::Acquire) {
            self.pending.clear();
            self.processor.reset().map_err(io::Error::other)?;
        }
        Buffers::set_disabled(&self.buffers.speaker, controls.speaker_suppressed)
    }

    pub(super) async fn service(
        &mut self,
        audio: &mut crate::audio_track::AudioTrack,
        now: impl Fn() -> Instant,
    ) -> io::Result<usize> {
        // Some backends start callbacks during stream construction, before open returns.
        self.buffers.serviced.store(true, Ordering::Release);
        let generation = self.buffers.microphone.load(Ordering::Acquire);
        for _ in 0..self.buffers.rendered.capacity() {
            let Some(frame) = self.buffers.rendered.pop() else {
                break;
            };
            if frame.at.elapsed() > MAX_CAPTURE_AGE {
                return Err(io::Error::other("audio rendering fell behind"));
            }
            self.processor.render(&frame).map_err(io::Error::other)?;
        }
        if self.pending.is_empty() {
            for _ in 0..self.buffers.capture.capacity() {
                let Some(frame) = self.buffers.capture.pop() else {
                    break;
                };
                if generation.is_multiple_of(2) && frame.generation == generation {
                    let frames = self.processor.capture(&frame, &now);
                    self.pending.extend(frames.map_err(io::Error::other)?);
                }
            }
        }
        if self.buffers.failed.load(Ordering::Acquire) {
            return Err(io::Error::other("audio device failed"));
        }
        let Some(frame) = self.pending.pop_front() else {
            return Ok(0);
        };
        if now().saturating_duration_since(frame.at) > processing::MAX_PROCESSING_DELAY {
            return Err(io::Error::other("voice processing fell behind"));
        }
        audio.send(frame).await.map_err(io::Error::other)?;
        Ok(1)
    }
}

#[cfg(test)]
#[path = "capture_worker_tests.rs"]
mod tests;
