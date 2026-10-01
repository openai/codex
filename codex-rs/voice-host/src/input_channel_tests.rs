//! Multichannel capture regression coverage independent of physical hardware.

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn microphone_selection_excludes_playback_channels() {
    let source = InputChannel::new(
        Some(vec![NonZeroU16::new(/*n*/ 1).unwrap()]),
        /*channels*/ 16,
    )
    .unwrap();
    let mut plugged_in = [0.0_f32; 16];
    plugged_in[0] = 0.25;
    plugged_in[2] = 0.8;
    plugged_in[3] = 0.7;
    let mut unplugged = plugged_in;
    unplugged[0] = 0.0;
    let interleaved = [plugged_in, unplugged].concat();
    let captured: Vec<_> = interleaved
        .chunks_exact(/*chunk_size*/ 16)
        .map(|frame| source.sample(frame))
        .collect();
    assert_eq!(captured, vec![0.25, 0.0]);
}

#[test]
fn selected_channel_converts_integer_samples_without_attenuation() {
    let source = InputChannel::new(
        Some(vec![NonZeroU16::new(/*n*/ 2).unwrap()]),
        /*channels*/ 2,
    )
    .unwrap();
    assert_eq!(source.sample(&[0_i16, 16384]), 0.5);
}

#[test]
fn unset_channel_preserves_mixing() {
    let source = InputChannel::new(/*channel*/ None, /*channels*/ 2).unwrap();
    assert_eq!(source.sample(&[0.25_f32, 0.75]), 0.5);
}

#[test]
fn unavailable_channel_is_rejected() {
    assert!(
        InputChannel::new(
            Some(vec![NonZeroU16::new(/*n*/ 17).unwrap()]),
            /*channels*/ 16
        )
        .is_err()
    );
    assert!(InputChannel::new(/*channel*/ None, /*channels*/ 0).is_err());
}

#[test]
fn subset_mixes_only_selected_inputs_and_deduplicates() {
    let selection = Some(vec![
        NonZeroU16::new(/*n*/ 2).unwrap(),
        NonZeroU16::new(/*n*/ 1).unwrap(),
        NonZeroU16::new(/*n*/ 2).unwrap(),
    ]);
    let source = InputChannel::new(selection, /*channels*/ 4).unwrap();
    assert_eq!(source.sample(&[0.2_f32, 0.6, 0.9, 0.9]), 0.4);
    assert_eq!(source.sample(&[0.0_f32, 0.0, 0.9, 0.9]), 0.0);
    assert!(InputChannel::new(Some(vec![]), /*channels*/ 4).is_err());
    assert!(
        InputChannel::new(
            Some(vec![
                NonZeroU16::new(/*n*/ 1).unwrap(),
                NonZeroU16::new(/*n*/ 5).unwrap()
            ]),
            /*channels*/ 4
        )
        .is_err()
    );
}
