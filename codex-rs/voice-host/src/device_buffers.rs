//! Preallocated callback buffers pack small callbacks into full queue slots without locks.
//! Generation changes and rejected capture buffers discard incomplete frames.

use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use crossbeam_queue::ArrayQueue;

pub(super) const BLOCK: usize = 256;
pub(super) const QUEUE_CAPACITY: usize = 32;

pub(super) struct Frame {
    pub(super) samples: [f32; BLOCK],
    pub(super) len: usize,
    pub(super) at: Instant,
    pub(super) generation: u64,
}

/// Packs serialized callbacks without allocating; only complete blocks enter the queue.
/// The oldest sample keeps its timestamp. Callers reset after rejected capture data.
#[derive(Default)]
pub(super) struct FramePacker {
    frame: Option<Frame>,
}

impl FramePacker {
    pub(super) fn reset(&mut self) {
        self.frame = None;
    }

    pub(super) fn push(&mut self, frame: Frame, rate: f64, queue: &ArrayQueue<Frame>) -> bool {
        if self
            .frame
            .as_ref()
            .is_some_and(|partial| partial.generation != frame.generation)
        {
            self.reset();
        }
        let mut offset = 0;
        while offset < frame.len {
            let partial = self.frame.get_or_insert_with(|| Frame {
                samples: [0.0; BLOCK],
                len: 0,
                at: frame.at + Duration::from_secs_f64(offset as f64 / rate),
                generation: frame.generation,
            });
            let count = (BLOCK - partial.len).min(frame.len - offset);
            partial.samples[partial.len..partial.len + count]
                .copy_from_slice(&frame.samples[offset..offset + count]);
            partial.len += count;
            offset += count;
            if partial.len == BLOCK
                && let Some(full) = self.frame.take()
                && let Err(rejected) = queue.push(full)
            {
                // Suppressed output is known silence; dropping it cannot lose audible output.
                // Never discard an active generation or a block containing real audio.
                if rejected.generation.is_multiple_of(2)
                    || rejected.samples.iter().any(|sample| *sample != 0.0)
                {
                    return false;
                }
            }
        }
        true
    }
}

pub(super) struct Buffers {
    pub(super) capture: ArrayQueue<Frame>,
    pub(super) rendered: ArrayQueue<Frame>,
    pub(super) playback: ArrayQueue<Frame>,
    pub(super) microphone: AtomicU64,
    pub(super) speaker: AtomicU64,
    pub(super) serviced: AtomicBool,
    pub(super) failed: AtomicBool,
}

impl Buffers {
    pub(super) fn new() -> Self {
        Self {
            capture: ArrayQueue::new(QUEUE_CAPACITY),
            rendered: ArrayQueue::new(QUEUE_CAPACITY),
            playback: ArrayQueue::new(QUEUE_CAPACITY),
            microphone: AtomicU64::new(/*v*/ 1),
            speaker: AtomicU64::new(/*v*/ 1),
            serviced: AtomicBool::new(false),
            failed: AtomicBool::new(false),
        }
    }

    // One control worker writes each epoch. Odd epochs are disabled; every
    // transition advances the epoch so disable/re-enable cannot replay old audio.
    pub(super) fn set_disabled(epoch: &AtomicU64, disabled: bool) -> std::io::Result<()> {
        let current = epoch.load(Ordering::Acquire);
        if (current % 2 == 1) != disabled {
            epoch.store(
                current
                    .checked_add(1)
                    .ok_or_else(|| std::io::Error::other("audio generation exhausted"))?,
                Ordering::Release,
            );
        }
        Ok(())
    }
}

/// Rejects capture backlog after unmute, using offsets in the device's clock.
/// This is conservative software admission, not a guarantee of hardware clock accuracy.
#[derive(Default)]
pub(super) struct CaptureBoundary {
    generation: Option<u64>,
    cutoff: Option<Duration>,
    previous_capture: Option<Duration>,
}

impl CaptureBoundary {
    pub(super) fn accepts(
        &mut self,
        generation: u64,
        callback: Duration,
        capture: Duration,
    ) -> bool {
        if generation % 2 == 1 || self.generation != Some(generation) {
            self.generation = Some(generation);
            self.cutoff = None;
            self.previous_capture = None;
            // The current callback timestamp may have been sampled before unmute.
            // Wait for the next serialized callback before establishing a cutoff.
            return false;
        }
        let accepted = capture >= *self.cutoff.get_or_insert(callback)
            && self
                .previous_capture
                .is_none_or(|previous| capture >= previous);
        if accepted {
            self.previous_capture = Some(capture);
        }
        accepted
    }
}

#[derive(Default)]
pub(super) struct Playback {
    frame: Option<Frame>,
    offset: usize,
}

impl Playback {
    pub(super) fn next(&mut self, buffers: &Buffers) -> f32 {
        let epoch = buffers.speaker.load(Ordering::Acquire);
        if self
            .frame
            .as_ref()
            .is_some_and(|frame| frame.generation != epoch || self.offset == frame.len)
        {
            self.frame = None;
        }
        // Bound stale-frame work even if a producer keeps writing during a mute.
        for _ in 0..buffers.playback.capacity() {
            if self.frame.is_some() {
                break;
            }
            let Some(frame) = buffers.playback.pop() else {
                break;
            };
            if epoch.is_multiple_of(2)
                && frame.generation == epoch
                && frame.len > 0
                && frame.len <= BLOCK
            {
                self.frame = Some(frame);
                self.offset = 0;
            }
        }
        let Some(frame) = &self.frame else {
            return 0.0;
        };
        let sample = frame.samples[self.offset];
        self.offset += 1;
        if epoch % 2 == 1 || !sample.is_finite() {
            0.0
        } else {
            sample.clamp(-1.0, 1.0)
        }
    }
}

#[cfg(test)]
#[path = "device_buffers_tests.rs"]
mod tests;
