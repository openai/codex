use crate::app_event_sender::AppEventSender;
use base64::Engine;
use codex_app_server_client::legacy_core::config::Config;
use codex_protocol::protocol::ConversationAudioParams;
use codex_protocol::protocol::RealtimeAudioFrame;
use cpal::traits::DeviceTrait;
use cpal::traits::StreamTrait;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU16;
use std::sync::atomic::Ordering;
use tracing::error;

const MODEL_AUDIO_SAMPLE_RATE: u32 = 24_000;
const MODEL_AUDIO_CHANNELS: u16 = 1;

pub struct VoiceCapture {
    stream: Option<cpal::Stream>,
    stopped: Arc<AtomicBool>,
    last_peak: Arc<AtomicU16>,
}

impl VoiceCapture {
    pub fn start_realtime(config: &Config, tx: AppEventSender) -> Result<Self, String> {
        let (device, config) = select_realtime_input_device_and_config(config)?;

        let sample_rate = config.sample_rate().0;
        let channels = config.channels();
        let stopped = Arc::new(AtomicBool::new(false));
        let last_peak = Arc::new(AtomicU16::new(0));

        let stream = build_realtime_input_stream(
            &device,
            &config,
            sample_rate,
            channels,
            tx,
            last_peak.clone(),
        )?;
        stream
            .play()
            .map_err(|e| format!("failed to start input stream: {e}"))?;

        Ok(Self {
            stream: Some(stream),
            stopped,
            last_peak,
        })
    }

    pub fn stop(mut self) {
        // Mark stopped so any metering task can exit cleanly.
        self.stopped.store(true, Ordering::SeqCst);
        // Dropping the stream stops capture.
        self.stream.take();
    }

    pub fn stopped_flag(&self) -> Arc<AtomicBool> {
        self.stopped.clone()
    }

    pub fn last_peak_arc(&self) -> Arc<AtomicU16> {
        self.last_peak.clone()
    }
}

pub(crate) struct RecordingMeterState {
    history: VecDeque<char>,
    noise_ema: f64,
    env: f64,
}

impl RecordingMeterState {
    pub(crate) fn new() -> Self {
        let mut history = VecDeque::with_capacity(4);
        while history.len() < 4 {
            history.push_back('⠤');
        }
        Self {
            history,
            noise_ema: 0.02,
            env: 0.0,
        }
    }

    pub(crate) fn next_text(&mut self, peak: u16) -> String {
        const SYMBOLS: [char; 7] = ['⠤', '⠴', '⠶', '⠷', '⡷', '⡿', '⣿'];
        const ALPHA_NOISE: f64 = 0.05;
        const ATTACK: f64 = 0.80;
        const RELEASE: f64 = 0.25;

        let latest_peak = peak as f64 / (i16::MAX as f64);

        if latest_peak > self.env {
            self.env = ATTACK * latest_peak + (1.0 - ATTACK) * self.env;
        } else {
            self.env = RELEASE * latest_peak + (1.0 - RELEASE) * self.env;
        }

        let rms_approx = self.env * 0.7;
        self.noise_ema = (1.0 - ALPHA_NOISE) * self.noise_ema + ALPHA_NOISE * rms_approx;
        let ref_level = self.noise_ema.max(0.01);
        let fast_signal = 0.8 * latest_peak + 0.2 * self.env;
        let target = 2.0f64;
        let raw = (fast_signal / (ref_level * target)).max(0.0);
        let k = 1.6f64;
        let compressed = (raw.ln_1p() / k.ln_1p()).min(1.0);
        let idx = (compressed * (SYMBOLS.len() as f64 - 1.0))
            .round()
            .clamp(0.0, SYMBOLS.len() as f64 - 1.0) as usize;
        let level_char = SYMBOLS[idx];

        if self.history.len() >= 4 {
            self.history.pop_front();
        }
        self.history.push_back(level_char);

        let mut text = String::with_capacity(4);
        for ch in &self.history {
            text.push(*ch);
        }
        text
    }
}

// -------------------------
// Voice input helpers
// -------------------------

fn select_realtime_input_device_and_config(
    config: &Config,
) -> Result<(cpal::Device, cpal::SupportedStreamConfig), String> {
    crate::audio_device::select_configured_input_device_and_config(config)
}

fn build_realtime_input_stream(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    sample_rate: u32,
    channels: u16,
    tx: AppEventSender,
    last_peak: Arc<AtomicU16>,
) -> Result<cpal::Stream, String> {
    match config.sample_format() {
        cpal::SampleFormat::F32 => device
            .build_input_stream(
                &config.clone().into(),
                move |input: &[f32], _| {
                    let peak = peak_f32(input);
                    last_peak.store(peak, Ordering::Relaxed);
                    let samples = input.iter().copied().map(f32_to_i16).collect::<Vec<_>>();
                    send_realtime_audio_chunk(&tx, samples, sample_rate, channels);
                },
                move |err| error!("audio input error: {err}"),
                None,
            )
            .map_err(|e| format!("failed to build input stream: {e}")),
        cpal::SampleFormat::I16 => device
            .build_input_stream(
                &config.clone().into(),
                move |input: &[i16], _| {
                    let peak = peak_i16(input);
                    last_peak.store(peak, Ordering::Relaxed);
                    send_realtime_audio_chunk(&tx, input.to_vec(), sample_rate, channels);
                },
                move |err| error!("audio input error: {err}"),
                None,
            )
            .map_err(|e| format!("failed to build input stream: {e}")),
        cpal::SampleFormat::U16 => device
            .build_input_stream(
                &config.clone().into(),
                move |input: &[u16], _| {
                    let mut samples = Vec::with_capacity(input.len());
                    let peak = convert_u16_to_i16_and_peak(input, &mut samples);
                    last_peak.store(peak, Ordering::Relaxed);
                    send_realtime_audio_chunk(&tx, samples, sample_rate, channels);
                },
                move |err| error!("audio input error: {err}"),
                None,
            )
            .map_err(|e| format!("failed to build input stream: {e}")),
        _ => Err("unsupported input sample format".to_string()),
    }
}

fn send_realtime_audio_chunk(
    tx: &AppEventSender,
    samples: Vec<i16>,
    sample_rate: u32,
    channels: u16,
) {
    if samples.is_empty() || sample_rate == 0 || channels == 0 {
        return;
    }

    let samples = if sample_rate == MODEL_AUDIO_SAMPLE_RATE && channels == MODEL_AUDIO_CHANNELS {
        samples
    } else {
        convert_pcm16(
            &samples,
            sample_rate,
            channels,
            MODEL_AUDIO_SAMPLE_RATE,
            MODEL_AUDIO_CHANNELS,
        )
    };
    if samples.is_empty() {
        return;
    }

    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for sample in &samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }

    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    let samples_per_channel = (samples.len() / usize::from(MODEL_AUDIO_CHANNELS)) as u32;

    tx.realtime_conversation_audio(ConversationAudioParams {
        frame: RealtimeAudioFrame {
            data: encoded,
            sample_rate: MODEL_AUDIO_SAMPLE_RATE,
            num_channels: MODEL_AUDIO_CHANNELS,
            samples_per_channel: Some(samples_per_channel),
            item_id: None,
        },
    });
}

#[inline]
fn f32_abs_to_u16(x: f32) -> u16 {
    let peak_u = (x.abs().min(1.0) * i16::MAX as f32) as i32;
    peak_u.max(0) as u16
}

#[inline]
fn f32_to_i16(s: f32) -> i16 {
    (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
}

fn peak_f32(input: &[f32]) -> u16 {
    let mut peak: f32 = 0.0;
    for &s in input {
        let a = s.abs();
        if a > peak {
            peak = a;
        }
    }
    f32_abs_to_u16(peak)
}

fn peak_i16(input: &[i16]) -> u16 {
    let mut peak: i32 = 0;
    for &s in input {
        let a = (s as i32).unsigned_abs() as i32;
        if a > peak {
            peak = a;
        }
    }
    peak as u16
}

fn convert_u16_to_i16_and_peak(input: &[u16], out: &mut Vec<i16>) -> u16 {
    let mut peak: i32 = 0;
    for &s in input {
        let v_i16 = (s as i32 - 32768) as i16;
        let a = (v_i16 as i32).unsigned_abs() as i32;
        if a > peak {
            peak = a;
        }
        out.push(v_i16);
    }
    peak as u16
}

// -------------------------
// Realtime audio playback helpers
// -------------------------

pub(crate) struct RealtimeAudioPlayer {
    _stream: cpal::Stream,
    queue: Arc<Mutex<VecDeque<i16>>>,
    output_sample_rate: u32,
    output_channels: u16,
}

impl RealtimeAudioPlayer {
    pub(crate) fn start(config: &Config) -> Result<Self, String> {
        let (device, config) =
            crate::audio_device::select_configured_output_device_and_config(config)?;
        let output_sample_rate = config.sample_rate().0;
        let output_channels = config.channels();
        let queue = Arc::new(Mutex::new(VecDeque::new()));
        let stream = build_output_stream(&device, &config, Arc::clone(&queue))?;
        stream
            .play()
            .map_err(|e| format!("failed to start output stream: {e}"))?;
        Ok(Self {
            _stream: stream,
            queue,
            output_sample_rate,
            output_channels,
        })
    }

    pub(crate) fn enqueue_frame(&self, frame: &RealtimeAudioFrame) -> Result<(), String> {
        if frame.num_channels == 0 || frame.sample_rate == 0 {
            return Err("invalid realtime audio frame format".to_string());
        }
        let raw_bytes = base64::engine::general_purpose::STANDARD
            .decode(&frame.data)
            .map_err(|e| format!("failed to decode realtime audio: {e}"))?;
        if raw_bytes.len() % 2 != 0 {
            return Err("realtime audio frame had odd byte length".to_string());
        }
        let mut pcm = Vec::with_capacity(raw_bytes.len() / 2);
        for pair in raw_bytes.chunks_exact(2) {
            pcm.push(i16::from_le_bytes([pair[0], pair[1]]));
        }
        let converted = convert_pcm16(
            &pcm,
            frame.sample_rate,
            frame.num_channels,
            self.output_sample_rate,
            self.output_channels,
        );
        if converted.is_empty() {
            return Ok(());
        }
        let mut guard = self
            .queue
            .lock()
            .map_err(|_| "failed to lock output audio queue".to_string())?;
        // TODO(aibrahim): Cap or trim this queue if we observe producer bursts outrunning playback.
        guard.extend(converted);
        Ok(())
    }

    pub(crate) fn clear(&self) {
        if let Ok(mut guard) = self.queue.lock() {
            guard.clear();
        }
    }
}

fn build_output_stream(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    queue: Arc<Mutex<VecDeque<i16>>>,
) -> Result<cpal::Stream, String> {
    let config_any: cpal::StreamConfig = config.clone().into();
    match config.sample_format() {
        cpal::SampleFormat::F32 => device
            .build_output_stream(
                &config_any,
                move |output: &mut [f32], _| fill_output_f32(output, &queue),
                move |err| error!("audio output error: {err}"),
                None,
            )
            .map_err(|e| format!("failed to build f32 output stream: {e}")),
        cpal::SampleFormat::I16 => device
            .build_output_stream(
                &config_any,
                move |output: &mut [i16], _| fill_output_i16(output, &queue),
                move |err| error!("audio output error: {err}"),
                None,
            )
            .map_err(|e| format!("failed to build i16 output stream: {e}")),
        cpal::SampleFormat::U16 => device
            .build_output_stream(
                &config_any,
                move |output: &mut [u16], _| fill_output_u16(output, &queue),
                move |err| error!("audio output error: {err}"),
                None,
            )
            .map_err(|e| format!("failed to build u16 output stream: {e}")),
        other => Err(format!("unsupported output sample format: {other:?}")),
    }
}

fn fill_output_i16(output: &mut [i16], queue: &Arc<Mutex<VecDeque<i16>>>) {
    if let Ok(mut guard) = queue.lock() {
        for sample in output {
            *sample = guard.pop_front().unwrap_or(0);
        }
        return;
    }
    output.fill(0);
}

fn fill_output_f32(output: &mut [f32], queue: &Arc<Mutex<VecDeque<i16>>>) {
    if let Ok(mut guard) = queue.lock() {
        for sample in output {
            let v = guard.pop_front().unwrap_or(0);
            *sample = (v as f32) / (i16::MAX as f32);
        }
        return;
    }
    output.fill(0.0);
}

fn fill_output_u16(output: &mut [u16], queue: &Arc<Mutex<VecDeque<i16>>>) {
    if let Ok(mut guard) = queue.lock() {
        for sample in output {
            let v = guard.pop_front().unwrap_or(0);
            *sample = (v as i32 + 32768).clamp(0, u16::MAX as i32) as u16;
        }
        return;
    }
    output.fill(32768);
}

fn convert_pcm16(
    input: &[i16],
    input_sample_rate: u32,
    input_channels: u16,
    output_sample_rate: u32,
    output_channels: u16,
) -> Vec<i16> {
    if input.is_empty() || input_channels == 0 || output_channels == 0 {
        return Vec::new();
    }

    let in_channels = input_channels as usize;
    let out_channels = output_channels as usize;
    let in_frames = input.len() / in_channels;
    if in_frames == 0 {
        return Vec::new();
    }

    let out_frames = if input_sample_rate == output_sample_rate {
        in_frames
    } else {
        (((in_frames as u64) * (output_sample_rate as u64)) / (input_sample_rate as u64)).max(1)
            as usize
    };

    let mut out = Vec::with_capacity(out_frames.saturating_mul(out_channels));
    for out_frame_idx in 0..out_frames {
        let src_frame_idx = if out_frames <= 1 || in_frames <= 1 {
            0
        } else {
            ((out_frame_idx as u64) * ((in_frames - 1) as u64) / ((out_frames - 1) as u64)) as usize
        };
        let src_start = src_frame_idx.saturating_mul(in_channels);
        let src = &input[src_start..src_start + in_channels];
        match (in_channels, out_channels) {
            (1, 1) => out.push(src[0]),
            (1, n) => {
                for _ in 0..n {
                    out.push(src[0]);
                }
            }
            (n, 1) if n >= 2 => {
                let sum: i32 = src.iter().map(|s| *s as i32).sum();
                out.push((sum / (n as i32)) as i16);
            }
            (n, m) if n == m => out.extend_from_slice(src),
            (n, m) if n > m => out.extend_from_slice(&src[..m]),
            (n, m) => {
                out.extend_from_slice(src);
                let last = *src.last().unwrap_or(&0);
                for _ in n..m {
                    out.push(last);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::convert_pcm16;
    use pretty_assertions::assert_eq;

    #[test]
    fn convert_pcm16_downmixes_and_resamples_for_model_input() {
        let input = vec![100, 300, 200, 400, 500, 700, 600, 800];
        let converted = convert_pcm16(
            &input, /*input_sample_rate*/ 48_000, /*input_channels*/ 2,
            /*output_sample_rate*/ 24_000, /*output_channels*/ 1,
        );
        assert_eq!(converted, vec![200, 700]);
    }
}
