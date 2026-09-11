//! Owns local device streams on the helper worker. Callbacks allocate no buffers and take no locks.
//! Small callbacks share full queue slots; overload discards stale media and resumes fresh audio.
//! Capture and actual rendered output carry device timing.
//! References start with worker service; unmute rejects earlier device capture buffers.

#[path = "audio_sink.rs"]
mod audio_sink;
#[path = "device_buffers.rs"]
mod buffers;
#[path = "capture_worker.rs"]
mod capture_worker;
#[path = "playback.rs"]
mod playback;
#[path = "playout.rs"]
mod playout;
#[path = "processing.rs"]
mod processing;

use std::io;
use std::sync::Arc;
use std::sync::atomic::AtomicU16;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use cpal::FromSample;
use cpal::Sample;
use cpal::SampleFormat;
use cpal::SizedSample;
use cpal::traits::DeviceTrait;
use cpal::traits::HostTrait;
use cpal::traits::StreamTrait;

use buffers::BLOCK;
use buffers::Buffers;
use buffers::CaptureBoundary;
use buffers::Frame;
use buffers::FramePacker;
use buffers::MAX_CALLBACK_FRAMES;
use buffers::Playback;
use buffers::QUEUE_CAPACITY;
use playback::PlaybackPort;

const MAX_CAPTURE_AGE: Duration = Duration::from_secs(/*secs*/ 1);

pub(super) struct Devices {
    _input: cpal::Stream,
    output: Option<cpal::Stream>,
    output_device: cpal::Device,
    input_frames: u32,
    playback: PlaybackPort,
    playout: Option<playout::Playout>,
    worker: capture_worker::CaptureWorker,
}

macro_rules! stream {
    ($format:expr, $build:ident, $($arg:expr),+) => {
        match $format {
            SampleFormat::I8 => $build::<i8>($($arg),+),
            SampleFormat::I16 => $build::<i16>($($arg),+),
            SampleFormat::I32 => $build::<i32>($($arg),+),
            SampleFormat::I64 => $build::<i64>($($arg),+),
            SampleFormat::U8 => $build::<u8>($($arg),+),
            SampleFormat::U16 => $build::<u16>($($arg),+),
            SampleFormat::U32 => $build::<u32>($($arg),+),
            SampleFormat::U64 => $build::<u64>($($arg),+),
            SampleFormat::F32 => $build::<f32>($($arg),+),
            SampleFormat::F64 => $build::<f64>($($arg),+),
            _ => Err(cpal::Error::from(cpal::ErrorKind::UnsupportedConfig)),
        }
    };
}

impl Devices {
    pub(super) fn take_state(&self) -> io::Result<codex_realtime_webrtc::AudioState> {
        self.worker.buffers.take_state()
    }

    pub(super) fn open() -> io::Result<Self> {
        let host = cpal::default_host();
        let input = host
            .default_input_device()
            .ok_or_else(|| io::Error::other("microphone unavailable"))?;
        let output = host
            .default_output_device()
            .ok_or_else(|| io::Error::other("speaker unavailable"))?;
        let input_config = input
            .default_input_config()
            .map_err(|_| io::Error::other("microphone configuration unavailable"))?;
        let output_config = output
            .default_output_config()
            .map_err(|_| io::Error::other("speaker configuration unavailable"))?;
        let input_stream_config = bounded_stream_config(&input_config)?;
        let output_stream_config = bounded_stream_config(&output_config)?;
        let buffers = Arc::new(Buffers::new(
            input_config.sample_rate(),
            output_config.sample_rate(),
        ));
        let playback = PlaybackPort::new(buffers.clone(), output_config.sample_rate());
        let processor =
            processing::Processor::new(input_config.sample_rate(), output_config.sample_rate())
                .map_err(io::Error::other)?;
        let (cpal::BufferSize::Fixed(input_frames), cpal::BufferSize::Fixed(output_frames)) = (
            input_stream_config.buffer_size,
            output_stream_config.buffer_size,
        ) else {
            return Err(io::Error::other("audio callback size unavailable"));
        };
        processor
            .validate_callback_timing(input_frames, output_frames)
            .map_err(io::Error::other)?;
        let input = stream!(
            input_config.sample_format(),
            build_input,
            &input,
            &input_stream_config,
            buffers.clone()
        )
        .map_err(|_| io::Error::other("failed to open microphone"))?;
        let output_stream = stream!(
            output_config.sample_format(),
            build_output,
            &output,
            &output_stream_config,
            buffers.clone()
        )
        .map_err(|_| io::Error::other("failed to open speaker"))?;
        output_stream
            .play()
            .map_err(|_| io::Error::other("failed to start speaker"))?;
        input
            .play()
            .map_err(|_| io::Error::other("failed to start microphone"))?;
        Ok(Self {
            _input: input,
            output: Some(output_stream),
            output_device: output,
            input_frames,
            playback,
            playout: None,
            worker: capture_worker::CaptureWorker {
                buffers,
                processor,
                pending: Default::default(),
            },
        })
    }

    pub(super) fn set_controls(
        &mut self,
        controls: codex_realtime_webrtc::AudioControls,
    ) -> io::Result<()> {
        let buffers = self.worker.buffers.clone();
        let previous = buffers.speaker.load(Ordering::Acquire);
        self.worker.set_controls(controls)?;
        if previous != buffers.speaker.load(Ordering::Acquire) {
            // Stop callbacks before native teardown can block the worker.
            drop(self.output.take());
            drop(self.playout.take());
            // The new epoch cancelled writers before either teardown; wait for
            // any in-flight write before resetting its queue.
            let producer = self
                .playback
                .0
                .producer
                .lock()
                .map_err(|_| io::Error::other("speaker writer failed"))?;
            while buffers.playback.pop().is_some() {}
            while buffers.rendered.pop().is_some() {}
            buffers.queued.store(/*val*/ 0, Ordering::Release);
            buffers.last_dac_ns.store(/*val*/ 0, Ordering::Release);
            drop(producer);
            while buffers.rendered.pop().is_some() {}
            // Opening a Bluetooth microphone can change the speaker's format.
            // Requery after stopping the old stream rather than restoring its
            // pre-microphone rate, which the device may no longer support.
            let output_config = self
                .output_device
                .default_output_config()
                .map_err(|_| io::Error::other("speaker configuration unavailable"))?;
            let output_stream_config = bounded_stream_config(&output_config)?;
            let cpal::BufferSize::Fixed(output_frames) = output_stream_config.buffer_size else {
                return Err(io::Error::other("audio callback size unavailable"));
            };
            self.worker
                .processor
                .set_render_rate(output_config.sample_rate())
                .map_err(io::Error::other)?;
            self.worker
                .processor
                .validate_callback_timing(self.input_frames, output_frames)
                .map_err(io::Error::other)?;
            self.playback = PlaybackPort::new(buffers.clone(), output_config.sample_rate());
            if !controls.speaker_suppressed {
                self.playout =
                    Some(playout::Playout::new(self.playback.writer()).map_err(io::Error::other)?);
            }
            let output = stream!(
                output_config.sample_format(),
                build_output,
                &self.output_device,
                &output_stream_config,
                buffers
            )
            .map_err(|_| io::Error::other("failed to reset speaker"))?;
            output
                .play()
                .map_err(|_| io::Error::other("failed to restart speaker"))?;
            self.output = Some(output);
        }
        Ok(())
    }

    pub(super) async fn service(
        &mut self,
        audio: &mut crate::audio_track::AudioTrack,
    ) -> io::Result<usize> {
        if let Some(playout) = &self.playout {
            playout
                .check()
                .map_err(|_| io::Error::other(crate::service_failure::ServiceFailure::Playout))?;
        }
        self.worker.service(audio, Instant::now).await
    }

    pub(super) fn receive(&self, packet: crate::incoming::ReceivedRtp) -> io::Result<()> {
        if let Some(playout) = &self.playout {
            playout.push(packet).map_err(io::Error::other)?;
        }
        Ok(())
    }
}

impl Drop for Devices {
    fn drop(&mut self) {
        self.worker.buffers.failed.store(true, Ordering::Release);
        drop(self.output.take());
        drop(self.playout.take());
        let _producer = self.playback.0.producer.lock().ok();
    }
}

fn bounded_stream_config(
    supported: &cpal::SupportedStreamConfig,
) -> io::Result<cpal::StreamConfig> {
    if supported.channels() == 0
        || supported.channels() > 32
        || !(8_000..=384_000).contains(&supported.sample_rate())
    {
        return Err(io::Error::other("unsupported audio device configuration"));
    }
    let cpal::SupportedBufferSize::Range { min, max } = *supported.buffer_size() else {
        return Err(io::Error::other("audio callback size range unavailable"));
    };
    let mut config = supported.config();
    let min = min.max(1);
    let max = max.min(MAX_CALLBACK_FRAMES as u32);
    if min > max {
        return Err(io::Error::other("unsupported audio callback size range"));
    }
    // ALSA allocates two periods. A 20 ms ring can be smaller than one PipeWire
    // graph cycle (e.g. 2048 frames at 48 kHz), silently losing capture samples
    // every cycle. Give Linux a bounded 100 ms ring instead, while retaining
    // 10 ms callbacks elsewhere and rejecting incompatible device ranges below.
    let periods_per_second = if cfg!(target_os = "linux") { 20 } else { 100 };
    let frames = (config.sample_rate / periods_per_second).clamp(min, max);
    let callback_duration =
        Duration::from_secs_f64(f64::from(frames) / f64::from(config.sample_rate));
    // Backends may deliver smaller callbacks than requested. Packing makes queue
    // coverage depend on samples, while an incomplete block adds bounded latency.
    let packing_duration = Duration::from_secs_f64(BLOCK as f64 / f64::from(config.sample_rate));
    let queue_duration = packing_duration * QUEUE_CAPACITY as u32;
    if callback_duration + packing_duration + super::DEVICE_SERVICE_INTERVAL >= MAX_CAPTURE_AGE
        || queue_duration <= super::DEVICE_SERVICE_INTERVAL
    {
        return Err(io::Error::other("unsupported audio callback timing"));
    }
    config.buffer_size = cpal::BufferSize::Fixed(frames);
    Ok(config)
}

fn record_peak(peak: &AtomicU16, sample: f32) {
    peak.fetch_max(
        (sample.abs().min(1.0) * f32::from(u16::MAX)) as u16,
        Ordering::Relaxed,
    );
}

// CPAL reports recoverable underruns/overruns through the same callback as device loss.
fn handle_stream_error(buffers: &Buffers, error: cpal::Error) {
    if error.kind() != cpal::ErrorKind::Xrun {
        buffers.failed.store(true, Ordering::Release);
    }
}

fn build_input<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    buffers: Arc<Buffers>,
) -> Result<cpal::Stream, cpal::Error>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = usize::from(config.channels);
    let rate = f64::from(config.sample_rate);
    let failure = buffers.clone();
    let mut origin = None;
    let mut boundary = CaptureBoundary::default();
    let mut capture = FramePacker::default();
    device.build_input_stream(
        *config,
        move |data: &[T], info: &cpal::InputCallbackInfo| {
            let generation = buffers.microphone.load(Ordering::Acquire);
            if generation % 2 == 1 {
                capture.reset();
                return;
            }
            let timestamp = info.timestamp();
            let origin = origin.get_or_insert(timestamp.capture);
            let (Some(callback), Some(captured)) = (
                timestamp.callback.checked_duration_since(*origin),
                timestamp.capture.checked_duration_since(*origin),
            ) else {
                capture.reset();
                return;
            };
            if !boundary.accepts(generation, callback, captured) {
                capture.reset();
                return;
            }
            let start = Instant::now()
                .checked_sub(
                    timestamp
                        .callback
                        .checked_duration_since(timestamp.capture)
                        .unwrap_or_default(),
                )
                .unwrap_or_else(Instant::now);
            capture.discard_capture_gap(start, rate);
            for (index, chunk) in data.chunks(BLOCK * channels).enumerate() {
                let mut frame = Frame {
                    samples: [0.0; BLOCK],
                    len: chunk.len() / channels,
                    at: start + Duration::from_secs_f64((index * BLOCK) as f64 / rate),
                    generation,
                };
                for (output, input) in frame.samples.iter_mut().zip(chunk.chunks_exact(channels)) {
                    *output = input
                        .iter()
                        .map(|sample| f32::from_sample(*sample))
                        .sum::<f32>()
                        / channels as f32;
                    if !output.is_finite() {
                        buffers.failed.store(true, Ordering::Release);
                        return;
                    }
                    record_peak(&buffers.microphone_peak, *output);
                }
                if !capture.push(frame, rate, &buffers.capture) {
                    capture.reset();
                    buffers.capture_dropped.store(true, Ordering::Release);
                    return;
                }
            }
        },
        move |error| handle_stream_error(&failure, error),
        /*timeout*/ None,
    )
}

fn build_output<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    buffers: Arc<Buffers>,
) -> Result<cpal::Stream, cpal::Error>
where
    T: SizedSample + FromSample<f32>,
    f32: FromSample<T>,
{
    let channels = usize::from(config.channels);
    let rate = f64::from(config.sample_rate);
    let failure = buffers.clone();
    let mut output = OutputState::default();
    device.build_output_stream(
        *config,
        move |data: &mut [T], info: &cpal::OutputCallbackInfo| {
            buffers
                .callback_sequence
                .fetch_add(/*val*/ 1, Ordering::AcqRel);
            let timestamp = info.timestamp();
            let start = Instant::now()
                + timestamp
                    .playback
                    .checked_duration_since(timestamp.callback)
                    .unwrap_or_default();
            render_output(data, channels, rate, start, &buffers, &mut output);
            buffers
                .callback_sequence
                .fetch_add(/*val*/ 1, Ordering::Release);
        },
        move |error| handle_stream_error(&failure, error),
        /*timeout*/ None,
    )
}

#[derive(Default)]
struct OutputState {
    playback: Playback,
    reference: FramePacker,
}

fn render_output<T>(
    data: &mut [T],
    channels: usize,
    rate: f64,
    start: Instant,
    buffers: &Buffers,
    output: &mut OutputState,
) where
    T: SizedSample + FromSample<f32>,
    f32: FromSample<T>,
{
    if !buffers.serviced.load(Ordering::Acquire) {
        data.fill(T::from_sample(0.0));
        return;
    }
    let mut delivered_until = None;
    for (index, chunk) in data.chunks_mut(BLOCK * channels).enumerate() {
        let mut reference = Frame {
            samples: [0.0; BLOCK],
            len: chunk.len() / channels,
            at: start + Duration::from_secs_f64((index * BLOCK) as f64 / rate),
            generation: buffers.speaker.load(Ordering::Acquire),
        };
        for (offset, (frame, sample)) in chunk
            .chunks_mut(channels)
            .zip(&mut reference.samples)
            .enumerate()
        {
            let next = output.playback.next(buffers);
            if next.is_some() {
                delivered_until = Some(
                    start + Duration::from_secs_f64((index * BLOCK + offset + 1) as f64 / rate),
                );
            }
            let rendered = T::from_sample(next.unwrap_or(0.0));
            frame.fill(rendered);
            *sample = f32::from_sample(rendered);
            record_peak(&buffers.speaker_peak, *sample);
        }
        if !output.reference.push(reference, rate, &buffers.rendered) {
            output.reference.reset();
            buffers.render_dropped.store(true, Ordering::Release);
        }
    }
    if let Some(end) = delivered_until {
        buffers.last_dac_ns.store(
            end.saturating_duration_since(buffers.clock)
                .as_nanos()
                .min(u128::from(u64::MAX)) as u64,
            Ordering::Release,
        );
    }
}

#[cfg(test)]
#[path = "devices_tests.rs"]
mod tests;
