use anyhow::Result;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RealtimeAudioFrame;
use tokio::sync::mpsc::UnboundedSender;

#[cfg(not(test))]
use anyhow::Context;
#[cfg(not(test))]
use base64::Engine;
#[cfg(not(test))]
use codex_protocol::protocol::ConversationAudioParams;
#[cfg(not(test))]
use cpal::traits::DeviceTrait;
#[cfg(not(test))]
use std::collections::VecDeque;
#[cfg(not(test))]
use std::sync::Arc;
#[cfg(not(test))]
use std::sync::Mutex;
#[cfg(not(test))]
use tracing::warn;

#[cfg(not(test))]
const TARGET_SAMPLE_RATE: u32 = 24_000;
#[cfg(not(test))]
const TARGET_NUM_CHANNELS: u16 = 1;
#[cfg(not(test))]
const TARGET_SAMPLES_PER_CHANNEL: u32 = 480;
#[cfg(not(test))]
const PLAYBACK_BUFFER_SECONDS: usize = 5;

pub(crate) struct RealtimeAudioController {
    backend: RealtimeAudioBackend,
}

enum RealtimeAudioBackend {
    #[cfg(not(test))]
    Live(LiveRealtimeAudioController),
    #[cfg(test)]
    Stub,
}

#[cfg(not(test))]
struct LiveRealtimeAudioController {
    _input_stream: cpal::Stream,
    _output_stream: cpal::Stream,
    playback_state: Arc<Mutex<PlaybackState>>,
}

impl RealtimeAudioController {
    pub(crate) fn start(codex_op_tx: UnboundedSender<Op>) -> Result<Self> {
        #[cfg(test)]
        {
            let _ = codex_op_tx;
            Ok(Self {
                backend: RealtimeAudioBackend::Stub,
            })
        }

        #[cfg(not(test))]
        {
            use cpal::traits::HostTrait;
            use cpal::traits::StreamTrait;

            let host = cpal::default_host();
            let input_device = host
                .default_input_device()
                .context("no default input device available")?;
            let output_device = host
                .default_output_device()
                .context("no default output device available")?;

            let input_supported = input_device
                .default_input_config()
                .context("failed to query default input config")?;
            let output_supported = output_device
                .default_output_config()
                .context("failed to query default output config")?;

            let input_config = input_supported.config();
            let output_config = output_supported.config();
            // TODO(aibrahim): Add persisted audio device + sample-rate selection/config for TUI
            // realtime conversations instead of always using defaults.

            let playback_state = Arc::new(Mutex::new(PlaybackState::new(
                output_config.sample_rate.0,
                output_config.channels,
            )));
            let mic_state = Arc::new(Mutex::new(MicCaptureState::new(
                codex_op_tx,
                input_config.sample_rate.0,
                input_config.channels,
            )));

            let input_stream = build_input_stream(
                &input_device,
                input_supported.sample_format(),
                &input_config,
                Arc::clone(&mic_state),
            )
            .context("failed to open microphone input stream")?;
            let output_stream = build_output_stream(
                &output_device,
                output_supported.sample_format(),
                &output_config,
                Arc::clone(&playback_state),
            )
            .context("failed to open speaker output stream")?;

            input_stream
                .play()
                .context("failed to start microphone stream")?;
            output_stream
                .play()
                .context("failed to start speaker output stream")?;

            Ok(Self {
                backend: RealtimeAudioBackend::Live(LiveRealtimeAudioController {
                    _input_stream: input_stream,
                    _output_stream: output_stream,
                    playback_state,
                }),
            })
        }
    }

    pub(crate) fn enqueue_audio_out(&self, frame: RealtimeAudioFrame) -> Result<()> {
        match &self.backend {
            #[cfg(not(test))]
            RealtimeAudioBackend::Live(controller) => {
                let mut state = controller
                    .playback_state
                    .lock()
                    .map_err(|_| anyhow::anyhow!("playback state lock poisoned"))?;
                state.enqueue(frame)?;
            }
            #[cfg(test)]
            RealtimeAudioBackend::Stub => {
                let _ = frame;
            }
        }
        Ok(())
    }

    pub(crate) fn shutdown(self) {}
}

#[cfg(not(test))]
#[derive(Debug)]
struct MicCaptureState {
    codex_op_tx: UnboundedSender<Op>,
    source_sample_rate: u32,
    source_channels: u16,
    source_mono: Vec<f32>,
    source_position: f64,
    resampled: Vec<f32>,
}

#[cfg(not(test))]
impl MicCaptureState {
    fn new(
        codex_op_tx: UnboundedSender<Op>,
        source_sample_rate: u32,
        source_channels: u16,
    ) -> Self {
        Self {
            codex_op_tx,
            source_sample_rate,
            source_channels,
            source_mono: Vec::new(),
            source_position: 0.0,
            resampled: Vec::new(),
        }
    }

    fn push_input_samples_f32(&mut self, data: &[f32]) {
        self.push_mono_samples_from_frames(data.iter().copied());
    }

    fn push_input_samples_i16(&mut self, data: &[i16]) {
        self.push_mono_samples_from_frames(
            data.iter().map(|sample| *sample as f32 / i16::MAX as f32),
        );
    }

    fn push_input_samples_u16(&mut self, data: &[u16]) {
        self.push_mono_samples_from_frames(
            data.iter()
                .map(|sample| (*sample as f32 / u16::MAX as f32) * 2.0 - 1.0),
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
            if let Err(err) = self.codex_op_tx.send(op) {
                warn!("failed to send realtime microphone frame: {err}");
                break;
            }
        }
    }
}

#[cfg(not(test))]
#[derive(Debug)]
struct PlaybackState {
    output_sample_rate: u32,
    output_channels: u16,
    queue: VecDeque<f32>,
    max_queue_samples: usize,
}

#[cfg(not(test))]
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
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(frame.data.as_bytes())
            .context("failed to decode realtime audio base64")?;
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

        let resampled =
            resample_linear_mono(&mono, frame.sample_rate, self.output_sample_rate.max(1));
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

#[cfg(not(test))]
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

fn resample_linear_mono(input: &[f32], src_rate: u32, dst_rate: u32) -> Vec<f32> {
    if input.is_empty() || src_rate == 0 || dst_rate == 0 {
        return Vec::new();
    }
    if src_rate == dst_rate || input.len() == 1 {
        return input.to_vec();
    }

    let out_len = ((input.len() as u64 * dst_rate as u64) / src_rate as u64)
        .max(1)
        .try_into()
        .unwrap_or(usize::MAX);
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
    out
}

#[cfg(not(test))]
fn encode_pcm16_le_base64(samples: &[f32]) -> String {
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let scaled = (clamped * i16::MAX as f32).round() as i16;
        bytes.extend_from_slice(&scaled.to_le_bytes());
    }
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

#[cfg(not(test))]
fn build_input_stream(
    device: &cpal::Device,
    sample_format: cpal::SampleFormat,
    config: &cpal::StreamConfig,
    mic_state: Arc<Mutex<MicCaptureState>>,
) -> Result<cpal::Stream> {
    let err_fn = |err| warn!("realtime microphone stream error: {err}");
    let stream = match sample_format {
        cpal::SampleFormat::F32 => device.build_input_stream(
            config,
            move |data: &[f32], _| {
                if let Ok(mut state) = mic_state.lock() {
                    state.push_input_samples_f32(data);
                }
            },
            err_fn,
            None,
        )?,
        cpal::SampleFormat::I16 => device.build_input_stream(
            config,
            move |data: &[i16], _| {
                if let Ok(mut state) = mic_state.lock() {
                    state.push_input_samples_i16(data);
                }
            },
            err_fn,
            None,
        )?,
        cpal::SampleFormat::U16 => device.build_input_stream(
            config,
            move |data: &[u16], _| {
                if let Ok(mut state) = mic_state.lock() {
                    state.push_input_samples_u16(data);
                }
            },
            err_fn,
            None,
        )?,
        other => {
            return Err(anyhow::anyhow!(
                "unsupported microphone sample format: {other:?}"
            ));
        }
    };
    Ok(stream)
}

#[cfg(not(test))]
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

#[cfg(not(test))]
fn write_output_f32(data: &mut [f32], playback_state: &Arc<Mutex<PlaybackState>>) {
    fill_output_buffer(data, playback_state, |sample| sample);
}

#[cfg(not(test))]
fn write_output_i16(data: &mut [i16], playback_state: &Arc<Mutex<PlaybackState>>) {
    fill_output_buffer(data, playback_state, |sample| {
        (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16
    });
}

#[cfg(not(test))]
fn write_output_u16(data: &mut [u16], playback_state: &Arc<Mutex<PlaybackState>>) {
    fill_output_buffer(data, playback_state, |sample| {
        let normalized = (sample.clamp(-1.0, 1.0) + 1.0) * 0.5;
        (normalized * u16::MAX as f32).round() as u16
    });
}

#[cfg(not(test))]
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
    use super::resample_linear_mono;
    use pretty_assertions::assert_eq;

    #[test]
    fn resample_linear_passthrough_when_same_rate() {
        let input = vec![0.1, -0.2, 0.3];
        assert_eq!(resample_linear_mono(&input, 24_000, 24_000), input);
    }
}
