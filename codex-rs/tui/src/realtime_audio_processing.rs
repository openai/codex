use aec3::voip::VoipAec3;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use tracing::warn;

pub(crate) const AUDIO_PROCESSING_SAMPLE_RATE: u32 = 24_000;
pub(crate) const AUDIO_PROCESSING_CHANNELS: u16 = 1;

enum AudioProcessorCommand {
    Capture(Vec<i16>),
    Render(Vec<i16>),
}

#[derive(Clone)]
pub(crate) struct RealtimeAudioProcessor {
    capture_rx: Arc<Mutex<Option<Receiver<Vec<i16>>>>>,
    command_tx: Sender<AudioProcessorCommand>,
}

impl RealtimeAudioProcessor {
    pub(crate) fn new() -> Result<Self, String> {
        build_pipeline()?;

        let (command_tx, command_rx) = mpsc::channel();
        let (capture_tx, capture_rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("codex-realtime-aec3".to_string())
            .spawn(move || run_processor_thread(command_rx, capture_tx))
            .map_err(|err| format!("failed to spawn realtime audio processor: {err}"))?;

        Ok(Self {
            capture_rx: Arc::new(Mutex::new(Some(capture_rx))),
            command_tx,
        })
    }

    pub(crate) fn capture_stage(
        &self,
        input_sample_rate: u32,
        input_channels: u16,
    ) -> Result<RealtimeCaptureAudioProcessor, String> {
        let capture_rx = self
            .capture_rx
            .lock()
            .ok()
            .and_then(|mut capture_rx| capture_rx.take())
            .ok_or_else(|| "realtime capture stage was already created".to_string())?;

        Ok(RealtimeCaptureAudioProcessor {
            capture_rx,
            command_tx: self.command_tx.clone(),
            input_sample_rate,
            input_channels,
            processed_samples: VecDeque::new(),
        })
    }

    pub(crate) fn render_stage(
        &self,
        output_sample_rate: u32,
        output_channels: u16,
    ) -> RealtimeRenderAudioProcessor {
        RealtimeRenderAudioProcessor {
            command_tx: self.command_tx.clone(),
            output_sample_rate,
            output_channels,
        }
    }
}

pub(crate) struct RealtimeCaptureAudioProcessor {
    capture_rx: Receiver<Vec<i16>>,
    command_tx: Sender<AudioProcessorCommand>,
    input_sample_rate: u32,
    input_channels: u16,
    processed_samples: VecDeque<i16>,
}

impl RealtimeCaptureAudioProcessor {
    pub(crate) fn process_samples(&mut self, samples: &[i16]) -> Vec<i16> {
        let converted = convert_pcm16(
            samples,
            self.input_sample_rate,
            self.input_channels,
            AUDIO_PROCESSING_SAMPLE_RATE,
            AUDIO_PROCESSING_CHANNELS,
        );
        if !converted.is_empty()
            && let Err(err) = self
                .command_tx
                .send(AudioProcessorCommand::Capture(converted))
        {
            warn!("failed to queue realtime capture audio: {err}");
        }

        loop {
            match self.capture_rx.try_recv() {
                Ok(processed) => self.processed_samples.extend(processed),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    warn!("realtime capture audio processor disconnected");
                    break;
                }
            }
        }

        self.processed_samples.drain(..).collect()
    }
}

pub(crate) struct RealtimeRenderAudioProcessor {
    command_tx: Sender<AudioProcessorCommand>,
    output_sample_rate: u32,
    output_channels: u16,
}

impl RealtimeRenderAudioProcessor {
    pub(crate) fn process_samples(&mut self, samples: &[i16]) {
        let converted = convert_pcm16(
            samples,
            self.output_sample_rate,
            self.output_channels,
            AUDIO_PROCESSING_SAMPLE_RATE,
            AUDIO_PROCESSING_CHANNELS,
        );
        if !converted.is_empty()
            && let Err(err) = self
                .command_tx
                .send(AudioProcessorCommand::Render(converted))
        {
            warn!("failed to queue realtime render audio: {err}");
        }
    }
}

fn build_pipeline() -> Result<VoipAec3, String> {
    VoipAec3::builder(
        AUDIO_PROCESSING_SAMPLE_RATE as usize,
        usize::from(AUDIO_PROCESSING_CHANNELS),
        usize::from(AUDIO_PROCESSING_CHANNELS),
    )
    .enable_high_pass(false)
    .enable_noise_suppression(false)
    .build()
    .map_err(|err| format!("failed to initialize realtime audio processor: {err}"))
}

fn run_processor_thread(command_rx: Receiver<AudioProcessorCommand>, capture_tx: Sender<Vec<i16>>) {
    let mut pipeline = match build_pipeline() {
        Ok(pipeline) => pipeline,
        Err(err) => {
            warn!("{err}");
            return;
        }
    };
    let capture_frame_len =
        pipeline.capture_frame_samples() * usize::from(AUDIO_PROCESSING_CHANNELS);
    let render_frame_len = pipeline.render_frame_samples() * usize::from(AUDIO_PROCESSING_CHANNELS);
    let mut pending_capture = VecDeque::new();
    let mut pending_render = VecDeque::new();

    while let Ok(command) = command_rx.recv() {
        match command {
            AudioProcessorCommand::Capture(samples) => {
                pending_capture.extend(samples);
                while pending_capture.len() >= capture_frame_len {
                    let capture_frame =
                        drain_pending_frame(&mut pending_capture, capture_frame_len);
                    let capture_frame = capture_frame
                        .iter()
                        .copied()
                        .map(i16_to_f32)
                        .collect::<Vec<_>>();
                    let mut output = vec![0.0; capture_frame.len()];
                    if let Err(err) = pipeline.process_capture_frame(
                        &capture_frame,
                        /*level_change*/ false,
                        &mut output,
                    ) {
                        warn!("failed to process realtime capture audio: {err}");
                        continue;
                    }

                    let processed = output.into_iter().map(f32_to_i16).collect::<Vec<_>>();
                    if let Err(err) = capture_tx.send(processed) {
                        warn!("failed to deliver realtime capture audio: {err}");
                        return;
                    }
                }
            }
            AudioProcessorCommand::Render(samples) => {
                pending_render.extend(samples);
                while pending_render.len() >= render_frame_len {
                    let render_frame = drain_pending_frame(&mut pending_render, render_frame_len);
                    let render_frame = render_frame
                        .iter()
                        .copied()
                        .map(i16_to_f32)
                        .collect::<Vec<_>>();
                    if let Err(err) = pipeline.handle_render_frame(&render_frame) {
                        warn!("failed to process realtime render audio: {err}");
                    }
                }
            }
        }
    }
}

fn drain_pending_frame(pending_samples: &mut VecDeque<i16>, frame_len: usize) -> Vec<i16> {
    pending_samples.drain(..frame_len).collect()
}

pub(crate) fn convert_pcm16(
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

fn i16_to_f32(sample: i16) -> f32 {
    (sample as f32) / (i16::MAX as f32)
}

fn f32_to_i16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
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
