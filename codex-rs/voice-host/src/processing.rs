//! Streaming device-rate conversion, echo/noise/gain processing, and 20 ms Opus encoding.
//! Mute transitions and capture gaps reset history; delayed pre-unmute buffers are discarded.

use std::collections::VecDeque;
use std::time::Duration;
use std::time::Instant;

use rubato::Resampler;
use rubato::audioadapter_buffers::direct::InterleavedSlice;
use sonora::AudioProcessing;
use sonora::StreamConfig;
use sonora::config::AdaptiveDigital;
use sonora::config::EchoCanceller;
use sonora::config::GainController2;
use sonora::config::NoiseSuppression;

use super::buffers::CAPTURE_GAP_TOLERANCE;
use super::buffers::Frame;
use crate::audio_track::EncodedAudio;

type Result<T> = std::result::Result<T, &'static str>;
const BLOCK: usize = 480;
pub(super) const MAX_PROCESSING_DELAY: Duration = Duration::from_millis(/*millis*/ 500);

struct Converter {
    resampler: rubato::Async<f32>,
    input: VecDeque<f32>,
    output: VecDeque<f32>,
    scratch: Vec<f32>,
    rate: u32,
    end: Option<Instant>,
}

impl Converter {
    fn new(rate: u32) -> Result<Self> {
        let resampler = rubato::Async::new_sinc(
            48_000.0 / f64::from(rate),
            /*max_resample_ratio_relative*/ 1.0,
            &rubato::SincInterpolationParameters::default(),
            rate.div_ceil(100) as usize,
            /*nbr_channels*/ 1,
            rubato::FixedAsync::Input,
        )
        .map_err(|_| "failed to create voice resampler")?;
        let scratch = vec![0.0; resampler.output_frames_max()];
        Ok(Self {
            resampler,
            input: VecDeque::new(),
            output: VecDeque::new(),
            scratch,
            rate,
            end: None,
        })
    }

    fn push(&mut self, frame: &Frame) -> Result<()> {
        if frame.len > frame.samples.len()
            || self.input.len() + frame.len > self.rate as usize
            || self.output.len() > 48_000
        {
            return Err("voice resampler backlog exceeded");
        }
        self.end =
            Some(frame.at + Duration::from_secs_f64(frame.len as f64 / f64::from(self.rate)));
        self.input.extend(&frame.samples[..frame.len]);
        while self.input.len() >= self.resampler.input_frames_next() {
            let samples = self.input.make_contiguous();
            let input = InterleavedSlice::new(samples, /*channels*/ 1, samples.len())
                .map_err(|_| "invalid voice resampler input")?;
            let capacity = self.scratch.len();
            let mut output =
                InterleavedSlice::new_mut(&mut self.scratch, /*channels*/ 1, capacity)
                    .map_err(|_| "invalid voice resampler output")?;
            let (consumed, produced) = self
                .resampler
                .process_into_buffer(&input, &mut output, /*indexing*/ None)
                .map_err(|_| "failed to resample voice audio")?;
            self.input.drain(..consumed);
            self.output.extend(&self.scratch[..produced]);
        }
        Ok(())
    }

    fn next(&mut self) -> Option<(Instant, [f32; BLOCK])> {
        if self.output.len() < BLOCK {
            return None;
        }
        let delay = self.input.len() as f64 / f64::from(self.rate)
            + (self.output.len() + self.resampler.output_delay()) as f64 / 48_000.0;
        let end = self.end?;
        let at = end
            .checked_sub(Duration::from_secs_f64(delay))
            .unwrap_or(end);
        let mut output = [0.0; BLOCK];
        for (out, sample) in output.iter_mut().zip(self.output.drain(..BLOCK)) {
            *out = sample;
        }
        Some((at, output))
    }
}

pub(super) struct Processor {
    capture: Converter,
    render: Converter,
    apm: AudioProcessing,
    encoder: opus::Encoder,
    pending: Vec<f32>,
    at: Instant,
    cutoff: Instant,
    render_delay: i64,
}

impl Processor {
    pub(super) fn new(input_rate: u32, output_rate: u32) -> Result<Self> {
        Ok(Self {
            capture: Converter::new(input_rate)?,
            render: Converter::new(output_rate)?,
            apm: AudioProcessing::builder()
                .config(sonora::Config {
                    echo_canceller: Some(EchoCanceller::default()),
                    noise_suppression: Some(NoiseSuppression::default()),
                    gain_controller2: Some(GainController2 {
                        adaptive_digital: Some(AdaptiveDigital::default()),
                        ..Default::default()
                    }),
                    ..Default::default()
                })
                .capture_config(StreamConfig::new(
                    /*sample_rate_hz*/ 48_000, /*num_channels*/ 1,
                ))
                .render_config(StreamConfig::new(
                    /*sample_rate_hz*/ 48_000, /*num_channels*/ 1,
                ))
                .build(),
            encoder: opus::Encoder::new(
                /*sample_rate*/ 48_000,
                opus::Channels::Mono,
                opus::Application::Voip,
            )
            .map_err(|_| "failed to create voice encoder")?,
            pending: Vec::with_capacity(960),
            at: Instant::now(),
            cutoff: Instant::now(),
            render_delay: 0,
        })
    }

    pub(super) fn reset(&mut self) -> Result<()> {
        *self = Self::new(self.capture.rate, self.render.rate)?;
        Ok(())
    }

    pub(super) fn validate_callback_timing(
        &self,
        input_frames: u32,
        output_frames: u32,
    ) -> Result<()> {
        let mut delay = super::super::DEVICE_SERVICE_INTERVAL;
        for (converter, frames) in [(&self.capture, input_frames), (&self.render, output_frames)] {
            // Include callback and queue packing latency, plus the resampler's
            // actual delay and one processing block, for both device directions.
            delay += Duration::from_secs_f64(
                (f64::from(frames) + super::buffers::BLOCK as f64) / f64::from(converter.rate)
                    + (converter.resampler.output_delay() + BLOCK) as f64 / 48_000.0,
            );
        }
        if delay >= MAX_PROCESSING_DELAY {
            return Err("audio callback timing exceeds processing deadline");
        }
        Ok(())
    }

    pub(super) fn render(&mut self, frame: &Frame) -> Result<()> {
        self.render.push(frame)?;
        while let Some((at, input)) = self.render.next() {
            let mut output = [0.0; BLOCK];
            self.apm
                .process_render_f32(&[&input], &mut [&mut output])
                .map_err(|_| "voice echo reference failed")?;
            let now = Instant::now();
            self.render_delay = if at > now {
                at.duration_since(now).as_millis() as i64
            } else {
                -(now.duration_since(at).as_millis() as i64)
            };
        }
        Ok(())
    }

    pub(super) fn capture(
        &mut self,
        frame: &Frame,
        now: impl Fn() -> Instant,
    ) -> Result<Vec<EncodedAudio>> {
        if frame.at < self.cutoff {
            return Ok(vec![]);
        }
        if self
            .capture
            .end
            .is_some_and(|end| frame.at.saturating_duration_since(end) > CAPTURE_GAP_TOLERANCE)
        {
            // Drop resampler, APM and half-packet history without advancing the
            // unmute boundary: this frame may already have waited in the queue.
            let cutoff = self.cutoff;
            self.reset()?;
            self.cutoff = cutoff;
        }
        self.capture.push(frame)?;
        let mut encoded = Vec::new();
        while let Some((at, input)) = self.capture.next() {
            let capture_age = now().saturating_duration_since(at);
            if capture_age > MAX_PROCESSING_DELAY {
                return Err("voice processing fell behind");
            }
            let delay = (capture_age.as_millis() as i64 + self.render_delay).max(0);
            if delay > MAX_PROCESSING_DELAY.as_millis() as i64 {
                return Err("voice processing fell behind");
            }
            self.apm
                .set_stream_delay_ms(delay as i32)
                .map_err(|_| "invalid voice echo delay")?;
            let mut output = [0.0; BLOCK];
            self.apm
                .process_capture_f32(&[&input], &mut [&mut output])
                .map_err(|_| "voice capture processing failed")?;
            if output.iter().any(|sample| !sample.is_finite()) {
                return Err("invalid processed voice audio");
            }
            if self.pending.is_empty() {
                self.at = at;
            }
            self.pending.extend(output);
            if self.pending.len() == 960 {
                let mut data = vec![0; 1275];
                let len = self
                    .encoder
                    .encode_float(&self.pending, &mut data)
                    .map_err(|_| "failed to encode voice audio")?;
                data.truncate(len);
                encoded.push(EncodedAudio { data, at: self.at });
                self.pending.clear();
            }
        }
        Ok(encoded)
    }
}

#[cfg(test)]
#[path = "processing_tests.rs"]
mod tests;
