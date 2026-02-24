use anyhow::Context;
use anyhow::Result;
use base64::Engine;
use codex_protocol::protocol::ConversationAudioParams;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RealtimeAudioFrame;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU16;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;
use tokio::sync::mpsc::Sender;
use tokio::sync::mpsc::error::TrySendError;
use tracing::warn;
const TARGET_SAMPLE_RATE: u32 = 24_000;
const TARGET_NUM_CHANNELS: u16 = 1;
const TARGET_SAMPLES_PER_CHANNEL: u32 = 480;
const PLAYBACK_BUFFER_SECONDS: usize = 5;
const MAX_AUDIO_FRAME_DECODED_BYTES: usize = 128 * 1024;
const MAX_AUDIO_FRAME_ENCODED_BYTES: usize = 192 * 1024;
const MIN_REALTIME_PLAYBACK_SAMPLE_RATE: u32 = 8_000;
const MAX_REALTIME_PLAYBACK_SAMPLE_RATE: u32 = 192_000;
const MAX_RESAMPLED_MONO_SAMPLES_PER_FRAME: usize = 480_000;

pub(crate) struct RealtimeAudioController {
    backend: RealtimeAudioBackend,
}

pub(crate) struct RealtimeMicMeterHandles {
    pub(crate) last_peak: Arc<AtomicU16>,
    pub(crate) stop: Arc<AtomicBool>,
}

enum RealtimeAudioBackend {
    Live(LiveRealtimeAudioController),
    Stub,
}
struct LiveRealtimeAudioController {
    _input_capture: crate::voice::VoiceCapture,
    _input_capture_thread: Option<thread::JoinHandle<()>>,
    _output_stream: cpal::Stream,
    playback_state: Arc<Mutex<PlaybackState>>,
    last_input_peak: Arc<AtomicU16>,
    meter_stop: Arc<AtomicBool>,
}

impl RealtimeAudioController {
    pub(crate) fn start(realtime_audio_op_tx: Sender<Op>) -> Result<Self> {
        if cfg!(test) {
            let _ = realtime_audio_op_tx;
            return Ok(Self {
                backend: RealtimeAudioBackend::Stub,
            });
        }
        let host = cpal::default_host();
        let output_device = host
            .default_output_device()
            .context("no default output device available")?;
        let output_supported = output_device
            .default_output_config()
            .context("failed to query default output config")?;

        let output_config = output_supported.config();
        // TODO(aibrahim): Add persisted audio device + sample-rate selection/config for TUI
        // realtime conversations instead of always using defaults.
        let input_capture = crate::voice::VoiceCapture::start().map_err(anyhow::Error::msg)?;
        let source_sample_rate = input_capture.sample_rate();
        let source_channels = input_capture.channels();
        if source_sample_rate == 0 || source_channels == 0 {
            return Err(anyhow::anyhow!(
                "unsupported realtime microphone format from VoiceCapture"
            ));
        }
        let source_data = input_capture.data_arc();
        let last_input_peak = input_capture.last_peak_arc();
        let meter_stop = input_capture.stopped_flag();
        let mic_state = Arc::new(Mutex::new(MicCaptureState::new(
            realtime_audio_op_tx,
            source_sample_rate,
            source_channels,
        )));
        let source_data_thread = source_data;
        let input_state = Arc::clone(&mic_state);
        let input_capture_thread = thread::spawn({
            let stop = Arc::clone(&meter_stop);
            move || {
                while !stop.load(Ordering::Relaxed) {
                    let samples = match source_data_thread.lock() {
                        Ok(mut data) => {
                            if data.is_empty() {
                                None
                            } else {
                                Some(std::mem::take(&mut *data))
                            }
                        }
                        Err(_) => {
                            warn!("failed to lock realtime microphone buffer");
                            None
                        }
                    };
                    if let Some(samples) = samples {
                        if let Ok(mut state) = input_state.lock() {
                            state.push_input_samples_i16(&samples);
                        } else {
                            warn!("failed to lock realtime microphone processing state");
                        }
                    }
                    thread::sleep(Duration::from_millis(5));
                }
            }
        });

        let playback_state = Arc::new(Mutex::new(PlaybackState::new(
            output_config.sample_rate.0,
            output_config.channels,
        )));
        let output_stream = build_output_stream(
            &output_device,
            output_supported.sample_format(),
            &output_config,
            Arc::clone(&playback_state),
        )
        .context("failed to open speaker output stream")?;

        output_stream
            .play()
            .context("failed to start speaker output stream")?;

        Ok(Self {
            backend: RealtimeAudioBackend::Live(LiveRealtimeAudioController {
                _input_capture: input_capture,
                _input_capture_thread: Some(input_capture_thread),
                _output_stream: output_stream,
                playback_state,
                last_input_peak,
                meter_stop,
            }),
        })
    }

    pub(crate) fn enqueue_audio_out(&self, frame: RealtimeAudioFrame) -> Result<()> {
        match &self.backend {
            RealtimeAudioBackend::Live(controller) => {
                let mut state = controller
                    .playback_state
                    .lock()
                    .map_err(|_| anyhow::anyhow!("playback state lock poisoned"))?;
                state.enqueue(frame)?;
            }
            RealtimeAudioBackend::Stub => {
                let _ = frame;
            }
        }
        Ok(())
    }

    pub(crate) fn shutdown(self) {
        if let RealtimeAudioBackend::Live(controller) = self.backend {
            if let Err(err) = controller._input_capture.stop().map(|_| ()) {
                warn!("failed to stop realtime microphone capture: {err}");
            }
            if let Some(handle) = controller._input_capture_thread {
                if let Err(err) = handle.join() {
                    warn!("failed to join realtime microphone input thread: {err:?}");
                }
            }
        }
    }

    pub(crate) fn meter_handles(&self) -> Option<RealtimeMicMeterHandles> {
        match &self.backend {
            RealtimeAudioBackend::Live(controller) => Some(RealtimeMicMeterHandles {
                last_peak: Arc::clone(&controller.last_input_peak),
                stop: Arc::clone(&controller.meter_stop),
            }),
            RealtimeAudioBackend::Stub => None,
        }
    }
}
#[derive(Debug)]
struct MicCaptureState {
    realtime_audio_op_tx: Sender<Op>,
    source_sample_rate: u32,
    source_channels: u16,
    source_mono: Vec<f32>,
    source_position: f64,
    resampled: Vec<f32>,
}
impl MicCaptureState {
    fn new(
        realtime_audio_op_tx: Sender<Op>,
        source_sample_rate: u32,
        source_channels: u16,
    ) -> Self {
        Self {
            realtime_audio_op_tx,
            source_sample_rate,
            source_channels,
            source_mono: Vec::new(),
            source_position: 0.0,
            resampled: Vec::new(),
        }
    }

    fn push_input_samples_i16(&mut self, data: &[i16]) {
        self.push_mono_samples_from_frames(
            data.iter().map(|sample| *sample as f32 / i16::MAX as f32),
        );
    }

    fn push_mono_samples_from_frames<I>(&mut self, mut samples: I)
    where
        I: Iterator<Item = f32>,
    {
        let channels = usize::from(self.source_channels.max(1));
        loop {
            let mut sum = 0.0f32;
            let mut count = 0usize;
            for _ in 0..channels {
                let Some(sample) = samples.next() else {
                    self.process_and_send_ready_frames();
                    return;
                };
                sum += sample;
                count += 1;
            }
            self.source_mono.push(sum / count as f32);
        }
    }

    fn process_and_send_ready_frames(&mut self) {
        if self.source_mono.is_empty() {
            return;
        }

        let step = self.source_sample_rate as f64 / TARGET_SAMPLE_RATE as f64;
        while self.source_position + 1.0 < self.source_mono.len() as f64 {
            let idx = self.source_position.floor() as usize;
            let frac = (self.source_position - idx as f64) as f32;
            let a = self.source_mono[idx];
            let b = self.source_mono[idx + 1];
            self.resampled.push(a + (b - a) * frac);
            self.source_position += step;
        }

        let consumed = self.source_position.floor() as usize;
        if consumed > 0 {
            self.source_mono.drain(..consumed);
            self.source_position -= consumed as f64;
        }

        let chunk_len = TARGET_SAMPLES_PER_CHANNEL as usize;
        while self.resampled.len() >= chunk_len {
            let samples: Vec<f32> = self.resampled.drain(..chunk_len).collect();
            let data = encode_pcm16_le_base64(&samples);
            let op = Op::RealtimeConversationAudio(ConversationAudioParams {
                frame: RealtimeAudioFrame {
                    data,
                    sample_rate: TARGET_SAMPLE_RATE,
                    num_channels: TARGET_NUM_CHANNELS,
                    samples_per_channel: Some(TARGET_SAMPLES_PER_CHANNEL),
                },
            });
            match self.realtime_audio_op_tx.try_send(op) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => {
                    warn!("dropping realtime microphone frame due to full TUI audio queue");
                }
                Err(TrySendError::Closed(_)) => {
                    warn!("failed to send realtime microphone frame: channel closed");
                    break;
                }
            }
        }
    }
}
#[derive(Debug)]
struct PlaybackState {
    output_sample_rate: u32,
    output_channels: u16,
    queue: VecDeque<f32>,
    max_queue_samples: usize,
}
impl PlaybackState {
    fn new(output_sample_rate: u32, output_channels: u16) -> Self {
        let max_queue_samples =
            output_sample_rate as usize * usize::from(output_channels) * PLAYBACK_BUFFER_SECONDS;
        Self {
            output_sample_rate,
            output_channels,
            queue: VecDeque::new(),
            max_queue_samples,
        }
    }

    fn enqueue(&mut self, frame: RealtimeAudioFrame) -> Result<()> {
        if frame.num_channels == 0 {
            return Ok(());
        }
        if frame.data.len() > MAX_AUDIO_FRAME_ENCODED_BYTES {
            warn!(
                encoded_len = frame.data.len(),
                "dropping oversized realtime audio frame before base64 decode"
            );
            return Ok(());
        }
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(frame.data.as_bytes())
            .context("failed to decode realtime audio base64")?;
        if decoded.len() > MAX_AUDIO_FRAME_DECODED_BYTES {
            warn!(
                decoded_len = decoded.len(),
                "dropping oversized realtime audio frame after base64 decode"
            );
            return Ok(());
        }
        if decoded.len() % 2 != 0 {
            return Err(anyhow::anyhow!(
                "realtime audio payload has odd byte length"
            ));
        }

        let pcm: Vec<i16> = decoded
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();
        if pcm.is_empty() {
            return Ok(());
        }

        let mono = interleaved_i16_to_mono_f32(&pcm, frame.num_channels);
        if frame.sample_rate == 0 {
            warn!("dropping realtime audio frame with zero sample rate");
            return Ok(());
        }
        if !is_supported_realtime_playback_sample_rate(frame.sample_rate) {
            warn!(
                sample_rate = frame.sample_rate,
                min_sample_rate = MIN_REALTIME_PLAYBACK_SAMPLE_RATE,
                max_sample_rate = MAX_REALTIME_PLAYBACK_SAMPLE_RATE,
                "dropping realtime audio frame with unsupported sample rate"
            );
            return Ok(());
        }

        let Some(resampled) =
            resample_linear_mono(&mono, frame.sample_rate, self.output_sample_rate.max(1))
        else {
            warn!(
                src_rate = frame.sample_rate,
                dst_rate = self.output_sample_rate.max(1),
                max_samples = MAX_RESAMPLED_MONO_SAMPLES_PER_FRAME,
                "dropping realtime audio frame due to oversized resampled output"
            );
            return Ok(());
        };
        for sample in resampled {
            for _ in 0..self.output_channels {
                self.queue.push_back(sample);
            }
        }

        if self.queue.len() > self.max_queue_samples {
            let drop_count = self.queue.len() - self.max_queue_samples;
            self.queue.drain(..drop_count);
            warn!("dropping old playback samples due to realtime audio buffer overflow");
        }
        Ok(())
    }

    fn next_sample(&mut self) -> f32 {
        self.queue.pop_front().unwrap_or(0.0)
    }
}
fn interleaved_i16_to_mono_f32(samples: &[i16], num_channels: u16) -> Vec<f32> {
    let channels = usize::from(num_channels.max(1));
    let mut mono = Vec::with_capacity(samples.len() / channels.max(1));
    for frame in samples.chunks(channels) {
        let sum: f32 = frame
            .iter()
            .map(|sample| *sample as f32 / i16::MAX as f32)
            .sum();
        mono.push(sum / frame.len() as f32);
    }
    mono
}

fn is_supported_realtime_playback_sample_rate(sample_rate: u32) -> bool {
    (MIN_REALTIME_PLAYBACK_SAMPLE_RATE..=MAX_REALTIME_PLAYBACK_SAMPLE_RATE).contains(&sample_rate)
}

fn resample_linear_mono(input: &[f32], src_rate: u32, dst_rate: u32) -> Option<Vec<f32>> {
    if input.is_empty() || src_rate == 0 || dst_rate == 0 {
        return Some(Vec::new());
    }
    if src_rate == dst_rate || input.len() == 1 {
        return Some(input.to_vec());
    }

    let out_len = ((input.len() as u64 * dst_rate as u64) / src_rate as u64).max(1);
    if out_len > MAX_RESAMPLED_MONO_SAMPLES_PER_FRAME as u64 {
        return None;
    }
    let out_len = usize::try_from(out_len).ok()?;
    let step = src_rate as f64 / dst_rate as f64;
    let mut pos = 0.0f64;
    let mut out = Vec::with_capacity(out_len);
    for _ in 0..out_len {
        let idx = pos.floor() as usize;
        if idx + 1 >= input.len() {
            out.push(*input.last().unwrap_or(&0.0));
        } else {
            let frac = (pos - idx as f64) as f32;
            let a = input[idx];
            let b = input[idx + 1];
            out.push(a + (b - a) * frac);
        }
        pos += step;
    }
    Some(out)
}
fn encode_pcm16_le_base64(samples: &[f32]) -> String {
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let scaled = (clamped * i16::MAX as f32).round() as i16;
        bytes.extend_from_slice(&scaled.to_le_bytes());
    }
    base64::engine::general_purpose::STANDARD.encode(bytes)
}
fn build_output_stream(
    device: &cpal::Device,
    sample_format: cpal::SampleFormat,
    config: &cpal::StreamConfig,
    playback_state: Arc<Mutex<PlaybackState>>,
) -> Result<cpal::Stream> {
    let err_fn = |err| warn!("realtime speaker stream error: {err}");
    let stream = match sample_format {
        cpal::SampleFormat::F32 => device.build_output_stream(
            config,
            move |data: &mut [f32], _| write_output_f32(data, &playback_state),
            err_fn,
            None,
        )?,
        cpal::SampleFormat::I16 => device.build_output_stream(
            config,
            move |data: &mut [i16], _| write_output_i16(data, &playback_state),
            err_fn,
            None,
        )?,
        cpal::SampleFormat::U16 => device.build_output_stream(
            config,
            move |data: &mut [u16], _| write_output_u16(data, &playback_state),
            err_fn,
            None,
        )?,
        other => {
            return Err(anyhow::anyhow!(
                "unsupported speaker sample format: {other:?}"
            ));
        }
    };
    Ok(stream)
}
fn write_output_f32(data: &mut [f32], playback_state: &Arc<Mutex<PlaybackState>>) {
    fill_output_buffer(data, playback_state, |sample| sample);
}
fn write_output_i16(data: &mut [i16], playback_state: &Arc<Mutex<PlaybackState>>) {
    fill_output_buffer(data, playback_state, |sample| {
        (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16
    });
}
fn write_output_u16(data: &mut [u16], playback_state: &Arc<Mutex<PlaybackState>>) {
    fill_output_buffer(data, playback_state, |sample| {
        let normalized = (sample.clamp(-1.0, 1.0) + 1.0) * 0.5;
        (normalized * u16::MAX as f32).round() as u16
    });
}
fn fill_output_buffer<T>(
    data: &mut [T],
    playback_state: &Arc<Mutex<PlaybackState>>,
    mut convert: impl FnMut(f32) -> T,
) {
    let mut maybe_state = playback_state.lock().ok();
    for slot in data.iter_mut() {
        let sample = maybe_state
            .as_mut()
            .map_or(0.0, |state| state.next_sample());
        *slot = convert(sample);
    }
}

#[cfg(test)]
mod tests {
    use super::MAX_RESAMPLED_MONO_SAMPLES_PER_FRAME;
    use super::is_supported_realtime_playback_sample_rate;
    use super::resample_linear_mono;
    use pretty_assertions::assert_eq;

    #[test]
    fn resample_linear_passthrough_when_same_rate() {
        let input = vec![0.1, -0.2, 0.3];
        assert_eq!(resample_linear_mono(&input, 24_000, 24_000), Some(input));
    }

    #[test]
    fn playback_sample_rate_validation_rejects_unrealistic_rates() {
        assert!(!is_supported_realtime_playback_sample_rate(1));
        assert!(!is_supported_realtime_playback_sample_rate(7_999));
        assert!(is_supported_realtime_playback_sample_rate(8_000));
        assert!(is_supported_realtime_playback_sample_rate(24_000));
        assert!(is_supported_realtime_playback_sample_rate(192_000));
        assert!(!is_supported_realtime_playback_sample_rate(192_001));
    }

    #[test]
    fn resample_linear_returns_none_when_output_would_exceed_cap() {
        let input = vec![0.0; (MAX_RESAMPLED_MONO_SAMPLES_PER_FRAME / 2) + 1];
        assert_eq!(resample_linear_mono(&input, 1, 2), None);
    }
}
