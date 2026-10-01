//! Mixes validated microphone channels before metering or processing captured audio.
//! Unset selections retain the legacy mix; explicit selections never fall back to other channels.

use std::io;
use std::num::NonZeroU16;

use cpal::FromSample;
use cpal::Sample;

pub(super) struct InputChannel(Vec<usize>);

impl InputChannel {
    pub(super) fn new(channel: Option<Vec<NonZeroU16>>, channels: u16) -> io::Result<Self> {
        let mut indices: Vec<_> = channel.map_or_else(
            || (0..usize::from(channels)).collect(),
            |selected| {
                selected
                    .into_iter()
                    .map(|channel| usize::from(channel.get() - 1))
                    .collect()
            },
        );
        if indices.is_empty() || indices.iter().any(|index| *index >= usize::from(channels)) {
            return Err(io::Error::other("selected microphone channels unavailable"));
        }
        indices.sort_unstable();
        indices.dedup();
        Ok(Self(indices))
    }

    pub(super) fn sample<T: Copy>(&self, frame: &[T]) -> f32
    where
        f32: FromSample<T>,
    {
        self.0
            .iter()
            .map(|index| f32::from_sample(frame[*index]))
            .sum::<f32>()
            / self.0.len() as f32
    }
}

#[cfg(test)]
#[path = "input_channel_tests.rs"]
mod tests;
