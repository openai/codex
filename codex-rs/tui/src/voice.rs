use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use codex_core::auth::AuthCredentialsStoreMode;
use codex_core::config::Config;
use codex_core::config::find_codex_home;
use codex_core::default_client::get_codex_user_agent;
use codex_login::AuthMode;
use codex_login::CodexAuth;
use cpal::traits::DeviceTrait;
use cpal::traits::HostTrait;
use cpal::traits::StreamTrait;
use hound::SampleFormat;
use hound::WavSpec;
use hound::WavWriter;
use std::collections::VecDeque;
use std::io::Cursor;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU16;
use std::sync::atomic::Ordering;
use tracing::error;
use tracing::info;
use tracing::trace;

struct TranscriptionAuthContext {
    mode: AuthMode,
    bearer_token: String,
    chatgpt_account_id: Option<String>,
    chatgpt_base_url: String,
}

pub struct RecordedAudio {
    pub data: Vec<i16>,
    pub sample_rate: u32,
    pub channels: u16,
}

pub struct VoiceCapture {
    stream: Option<cpal::Stream>,
    sample_rate: u32,
    channels: u16,
    data: Arc<Mutex<Vec<i16>>>,
    stopped: Arc<AtomicBool>,
    last_peak: Arc<AtomicU16>,
}

impl VoiceCapture {
    pub fn start() -> Result<Self, String> {
        let (device, config) = select_input_device_and_config()?;

        let sample_rate = config.sample_rate().0;
        let channels = config.channels();
        let data: Arc<Mutex<Vec<i16>>> = Arc::new(Mutex::new(Vec::new()));
        let stopped = Arc::new(AtomicBool::new(false));
        let last_peak = Arc::new(AtomicU16::new(0));

        let stream = build_input_stream(&device, &config, data.clone(), last_peak.clone())?;
        stream
            .play()
            .map_err(|e| format!("failed to start input stream: {e}"))?;

        Ok(Self {
            stream: Some(stream),
            sample_rate,
            channels,
            data,
            stopped,
            last_peak,
        })
    }

    pub fn stop(mut self) -> Result<RecordedAudio, String> {
        // Mark stopped so any metering task can exit cleanly.
        self.stopped.store(true, Ordering::SeqCst);
        // Dropping the stream stops capture.
        self.stream.take();
        let data = self
            .data
            .lock()
            .map_err(|_| "failed to lock audio buffer".to_string())?
            .clone();
        Ok(RecordedAudio {
            data,
            sample_rate: self.sample_rate,
            channels: self.channels,
        })
    }

    pub fn data_arc(&self) -> Arc<Mutex<Vec<i16>>> {
        self.data.clone()
    }

    pub fn stopped_flag(&self) -> Arc<AtomicBool> {
        self.stopped.clone()
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.channels
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

pub fn transcribe_async(
    id: String,
    audio: RecordedAudio,
    context: Option<String>,
    tx: AppEventSender,
) {
    std::thread::spawn(move || {
        // Enforce minimum duration to avoid garbage outputs.
        const MIN_DURATION_SECONDS: f32 = 1.0;
        let duration_seconds = clip_duration_seconds(&audio);
        if duration_seconds < MIN_DURATION_SECONDS {
            let msg = format!(
                "recording too short ({duration_seconds:.2}s); minimum is {MIN_DURATION_SECONDS:.2}s"
            );
            info!("{msg}");
            tx.send(AppEvent::TranscriptionFailed { id, error: msg });
            return;
        }

        // Encode entire clip as normalized WAV.
        let wav_bytes = match encode_wav_normalized(&audio) {
            Ok(b) => b,
            Err(e) => {
                error!("failed to encode wav: {e}");
                tx.send(AppEvent::TranscriptionFailed { id, error: e });
                return;
            }
        };

        // Run the HTTP request on a small, dedicated runtime.
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                error!("failed to create tokio runtime: {e}");
                return;
            }
        };

        let tx2 = tx.clone();
        let id2 = id.clone();
        let res: Result<String, String> = rt
            .block_on(async move { transcribe_bytes(wav_bytes, context, duration_seconds).await });

        match res {
            Ok(text) => {
                tx2.send(AppEvent::TranscriptionComplete { id: id2, text });
                info!("voice transcription succeeded");
            }
            Err(e) => {
                error!("voice transcription error: {e}");
                tx.send(AppEvent::TranscriptionFailed { id, error: e });
            }
        }
    });
}

// -------------------------
// Voice input helpers
// -------------------------

fn select_input_device_and_config() -> Result<(cpal::Device, cpal::SupportedStreamConfig), String> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| "no input audio device available".to_string())?;
    let config = device
        .default_input_config()
        .map_err(|e| format!("failed to get default input config: {e}"))?;
    Ok((device, config))
}

fn build_input_stream(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    data: Arc<Mutex<Vec<i16>>>,
    last_peak: Arc<AtomicU16>,
) -> Result<cpal::Stream, String> {
    match config.sample_format() {
        cpal::SampleFormat::F32 => device
            .build_input_stream(
                &config.clone().into(),
                move |input: &[f32], _| {
                    let peak = peak_f32(input);
                    last_peak.store(peak, Ordering::Relaxed);
                    if let Ok(mut buf) = data.lock() {
                        for &s in input {
                            buf.push(f32_to_i16(s));
                        }
                    }
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
                    if let Ok(mut buf) = data.lock() {
                        buf.extend_from_slice(input);
                    }
                },
                move |err| error!("audio input error: {err}"),
                None,
            )
            .map_err(|e| format!("failed to build input stream: {e}")),
        cpal::SampleFormat::U16 => device
            .build_input_stream(
                &config.clone().into(),
                move |input: &[u16], _| {
                    if let Ok(mut buf) = data.lock() {
                        let peak = convert_u16_to_i16_and_peak(input, &mut buf);
                        last_peak.store(peak, Ordering::Relaxed);
                    }
                },
                move |err| error!("audio input error: {err}"),
                None,
            )
            .map_err(|e| format!("failed to build input stream: {e}")),
        _ => Err("unsupported input sample format".to_string()),
    }
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
// Transcription helpers
// -------------------------

fn clip_duration_seconds(audio: &RecordedAudio) -> f32 {
    let total_samples = audio.data.len() as f32;
    let samples_per_second = (audio.sample_rate as f32) * (audio.channels as f32);
    if samples_per_second > 0.0 {
        total_samples / samples_per_second
    } else {
        0.0
    }
}

fn encode_wav_normalized(audio: &RecordedAudio) -> Result<Vec<u8>, String> {
    let mut wav_bytes: Vec<u8> = Vec::new();
    let spec = WavSpec {
        channels: audio.channels,
        sample_rate: audio.sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut cursor = Cursor::new(&mut wav_bytes);
    let mut writer =
        WavWriter::new(&mut cursor, spec).map_err(|_| "failed to create wav writer".to_string())?;

    // Simple peak normalization with headroom to improve audibility on quiet inputs.
    let segment = &audio.data[..];
    let mut peak: i16 = 0;
    for &s in segment {
        let a = s.unsigned_abs();
        if a > peak.unsigned_abs() {
            peak = s;
        }
    }
    let peak_abs = (peak as i32).unsigned_abs() as i32;
    let target = (i16::MAX as f32) * 0.9; // leave some headroom
    let gain: f32 = if peak_abs > 0 {
        target / (peak_abs as f32)
    } else {
        1.0
    };

    for &s in segment {
        let v = ((s as f32) * gain)
            .round()
            .clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        writer
            .write_sample(v)
            .map_err(|_| "failed writing wav sample".to_string())?;
    }
    writer
        .finalize()
        .map_err(|_| "failed to finalize wav".to_string())?;
    Ok(wav_bytes)
}

fn normalize_chatgpt_base_url(input: &str) -> String {
    let mut base_url = input.to_string();
    while base_url.ends_with('/') {
        base_url.pop();
    }
    if (base_url.starts_with("https://chatgpt.com")
        || base_url.starts_with("https://chat.openai.com"))
        && !base_url.contains("/backend-api")
    {
        base_url = format!("{base_url}/backend-api");
    }
    base_url
}

async fn resolve_auth() -> Result<TranscriptionAuthContext, String> {
    let codex_home = find_codex_home().map_err(|e| format!("failed to find codex home: {e}"))?;
    let auth = CodexAuth::from_auth_storage(&codex_home, AuthCredentialsStoreMode::Auto)
        .map_err(|e| format!("failed to read auth.json: {e}"))?
        .ok_or_else(|| "No Codex auth is configured; please run `codex login`".to_string())?;

    let chatgpt_account_id = auth.get_account_id();

    let token = auth
        .get_token()
        .map_err(|e| format!("failed to get auth token: {e}"))?;
    let config = Config::load_with_cli_overrides(Vec::new())
        .await
        .map_err(|e| format!("failed to load config: {e}"))?;
    Ok(TranscriptionAuthContext {
        mode: auth.api_auth_mode(),
        bearer_token: token,
        chatgpt_account_id,
        chatgpt_base_url: normalize_chatgpt_base_url(&config.chatgpt_base_url),
    })
}

async fn transcribe_bytes(
    wav_bytes: Vec<u8>,
    context: Option<String>,
    duration_seconds: f32,
) -> Result<String, String> {
    let auth = resolve_auth().await?;
    let client = reqwest::Client::new();
    let audio_bytes = wav_bytes.len();
    let prompt_for_log = context.as_deref().unwrap_or("").to_string();
    let (endpoint, request) =
        if matches!(auth.mode, AuthMode::Chatgpt | AuthMode::ChatgptAuthTokens) {
            let part = reqwest::multipart::Part::bytes(wav_bytes)
                .file_name("audio.wav")
                .mime_str("audio/wav")
                .map_err(|e| format!("failed to set mime: {e}"))?;
            let form = reqwest::multipart::Form::new().part("file", part);
            let endpoint = format!("{}/transcribe", auth.chatgpt_base_url);
            let mut req = client
                .post(&endpoint)
                .bearer_auth(&auth.bearer_token)
                .multipart(form)
                .header("User-Agent", get_codex_user_agent());
            if let Some(acc) = auth.chatgpt_account_id {
                req = req.header("ChatGPT-Account-Id", acc);
            }
            (endpoint, req)
        } else {
            let part = reqwest::multipart::Part::bytes(wav_bytes)
                .file_name("audio.wav")
                .mime_str("audio/wav")
                .map_err(|e| format!("failed to set mime: {e}"))?;
            let mut form = reqwest::multipart::Form::new()
                .text("model", "gpt-4o-transcribe")
                .part("file", part);
            if let Some(context) = context {
                form = form.text("prompt", context);
            }
            let endpoint = "https://api.openai.com/v1/audio/transcriptions".to_string();
            (
                endpoint,
                client
                    .post("https://api.openai.com/v1/audio/transcriptions")
                    .bearer_auth(&auth.bearer_token)
                    .multipart(form)
                    .header("User-Agent", get_codex_user_agent()),
            )
        };

    let audio_kib = audio_bytes as f32 / 1024.0;
    let mode = auth.mode;
    trace!(
        "sending transcription request: mode={mode:?} endpoint={endpoint} duration={duration_seconds:.2}s audio={audio_kib:.1}KiB prompt={prompt_for_log}"
    );

    let resp = request
        .send()
        .await
        .map_err(|e| format!("transcription request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp
            .text()
            .await
            .unwrap_or_else(|_| "<failed to read body>".to_string());
        return Err(format!("transcription failed: {status} {body}"));
    }

    let v: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("failed to parse json: {e}"))?;
    let text = v
        .get("text")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();

    if text.is_empty() {
        Err("empty transcription result".to_string())
    } else {
        Ok(text)
    }
}
