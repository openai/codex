use std::collections::VecDeque;
use std::sync::Arc;
use tracing::warn;
use webrtc_audio_processing::Config as AudioProcessingConfig;
use webrtc_audio_processing::Processor;
use webrtc_audio_processing::config::EchoCanceller;
use webrtc_audio_processing::config::GainController;
use webrtc_audio_processing::config::HighPassFilter;
use webrtc_audio_processing::config::NoiseSuppression;
use webrtc_audio_processing::config::NoiseSuppressionLevel;
use webrtc_audio_processing::config::Pipeline;

pub(crate) const AUDIO_PROCESSING_SAMPLE_RATE: u32 = 24_000;
pub(crate) const AUDIO_PROCESSING_CHANNELS: u16 = 1;

#[derive(Clone)]
pub(crate) struct RealtimeAudioProcessor {
    processor: Arc<Processor>,
}

impl RealtimeAudioProcessor {
    pub(crate) fn new() -> Result<Self, String> {
        let processor = Processor::new(AUDIO_PROCESSING_SAMPLE_RATE)
            .map_err(|err| format!("failed to initialize realtime audio processor: {err}"))?;
        processor.set_config(AudioProcessingConfig {
            pipeline: Pipeline {
                multi_channel_capture: false,
                multi_channel_render: false,
                ..Default::default()
            },
            echo_canceller: Some(EchoCanceller::Full {
                stream_delay_ms: None,
            }),
            noise_suppression: Some(NoiseSuppression {
                level: NoiseSuppressionLevel::High,
                ..Default::default()
            }),
            gain_controller: Some(GainController::GainController2(Default::default())),
            high_pass_filter: Some(HighPassFilter::default()),
            ..Default::default()
        });
        processor.set_output_will_be_muted(true);
        Ok(Self {
            processor: Arc::new(processor),
        })
    }

    pub(crate) fn capture_stage(
        &self,
        input_sample_rate: u32,
        input_channels: u16,
    ) -> RealtimeCaptureAudioProcessor {
        RealtimeCaptureAudioProcessor {
            processor: self.processor.clone(),
            input_sample_rate,
            input_channels,
            pending_samples: VecDeque::new(),
        }
    }

    pub(crate) fn render_stage(
        &self,
        output_sample_rate: u32,
        output_channels: u16,
    ) -> RealtimeRenderAudioProcessor {
        RealtimeRenderAudioProcessor {
            processor: self.processor.clone(),
            output_sample_rate,
            output_channels,
            pending_samples: VecDeque::new(),
        }
    }

    pub(crate) fn set_output_will_be_muted(&self, muted: bool) {
        self.processor.set_output_will_be_muted(muted);
    }
}

pub(crate) struct RealtimeCaptureAudioProcessor {
    processor: Arc<Processor>,
    input_sample_rate: u32,
    input_channels: u16,
    pending_samples: VecDeque<i16>,
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
        self.pending_samples.extend(converted);

        let mut processed = Vec::new();
        while self.pending_samples.len() >= self.processor.num_samples_per_frame() {
            let mut frame = self.pop_pending_frame();
            if let Err(err) = self.processor.process_capture_frame([frame.as_mut_slice()]) {
                warn!("failed to process realtime capture audio: {err}");
                continue;
            }
            processed.extend(frame.into_iter().map(f32_to_i16));
        }
        processed
    }

    fn pop_pending_frame(&mut self) -> Vec<f32> {
        self.pending_samples
            .drain(..self.processor.num_samples_per_frame())
            .map(i16_to_f32)
            .collect()
    }
}

pub(crate) struct RealtimeRenderAudioProcessor {
    processor: Arc<Processor>,
    output_sample_rate: u32,
    output_channels: u16,
    pending_samples: VecDeque<i16>,
}

impl RealtimeRenderAudioProcessor {
    pub(crate) fn process_samples(&mut self, samples: &[i16]) {
        self.processor
            .set_output_will_be_muted(samples.iter().all(|sample| *sample == 0));

        let converted = convert_pcm16(
            samples,
            self.output_sample_rate,
            self.output_channels,
            AUDIO_PROCESSING_SAMPLE_RATE,
            AUDIO_PROCESSING_CHANNELS,
        );
        self.pending_samples.extend(converted);

        while self.pending_samples.len() >= self.processor.num_samples_per_frame() {
            let mut frame = self.pop_pending_frame();
            if let Err(err) = self.processor.process_render_frame([frame.as_mut_slice()]) {
                warn!("failed to process realtime render audio: {err}");
            }
        }
    }

    fn pop_pending_frame(&mut self) -> Vec<f32> {
        self.pending_samples
            .drain(..self.processor.num_samples_per_frame())
            .map(i16_to_f32)
            .collect()
    }
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

#[inline]
fn i16_to_f32(sample: i16) -> f32 {
    (sample as f32) / (i16::MAX as f32)
}

#[inline]
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
