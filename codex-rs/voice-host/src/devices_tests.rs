use super::*;
use pretty_assertions::assert_eq;

#[test]
fn startup_output_waits_for_service_but_later_overflow_still_fails() {
    let buffers = Buffers::new(/*input_rate*/ 48_000, /*output_rate*/ 384_000);
    let mut state = OutputState::default();
    let mut output = [1.0_f32; BLOCK * 2];
    let start = Instant::now();
    for _ in 0..QUEUE_CAPACITY + 1 {
        render_output(
            &mut output,
            /*channels*/ 2,
            /*rate*/ 384_000.0,
            start,
            &buffers,
            &mut state,
        );
        assert_eq!(output, [0.0; BLOCK * 2]);
    }
    assert!(buffers.rendered.is_empty());
    assert!(!buffers.failed.load(Ordering::Acquire));

    buffers.serviced.store(true, Ordering::Release);
    render_output(
        &mut output,
        /*channels*/ 2,
        /*rate*/ 384_000.0,
        start,
        &buffers,
        &mut state,
    );
    let reference = buffers.rendered.pop().unwrap();
    assert_eq!(
        (
            reference.samples,
            reference.len,
            reference.at,
            reference.generation
        ),
        (
            [0.0; BLOCK],
            BLOCK,
            start,
            buffers.speaker.load(Ordering::Acquire)
        ),
    );
    Buffers::set_disabled(&buffers.speaker, /*disabled*/ false).unwrap();
    for _ in 0..buffers.rendered.capacity() {
        render_output(
            &mut output,
            /*channels*/ 2,
            /*rate*/ 384_000.0,
            start,
            &buffers,
            &mut state,
        );
    }
    assert!(!buffers.failed.load(Ordering::Acquire));
    render_output(
        &mut output,
        /*channels*/ 2,
        /*rate*/ 384_000.0,
        start,
        &buffers,
        &mut state,
    );
    assert!(buffers.failed.load(Ordering::Acquire));
}

#[test]
fn callback_configuration_fits_supported_range_and_actual_queue() {
    for (rate, min, max, frames) in [
        (96_000, 0, u32::MAX, 960),
        (96_000, 1, 16, 16),
        (384_000, 1, 64, 64),
        (8_000, 7_680, 8_192, 7_680),
        (384_000, 1, 16, 16),
        (96_000, 1, 15, 15),
        (48_000, 1, 128, 128),
        (48_000, 2_048, 16_384, 2_048),
        (384_000, 6_016, 16_384, 6_016),
    ] {
        let supported = cpal::SupportedStreamConfig::new(
            /*channels*/ 2,
            rate,
            cpal::SupportedBufferSize::Range { min, max },
            cpal::SampleFormat::F32,
        );
        let config = bounded_stream_config(&supported).unwrap();
        assert_eq!(
            config,
            cpal::StreamConfig {
                channels: 2,
                sample_rate: rate,
                buffer_size: cpal::BufferSize::Fixed(frames),
            }
        );
        let buffers = Buffers::new(rate, rate);
        for queue in [&buffers.capture, &buffers.rendered] {
            for offset in (0..frames as usize).step_by(BLOCK) {
                assert!(
                    queue
                        .push(Frame {
                            samples: [0.0; BLOCK],
                            len: (frames as usize - offset).min(BLOCK),
                            at: Instant::now(),
                            generation: 0,
                        })
                        .is_ok()
                );
            }
        }
    }
}

#[test]
fn callback_configuration_fits_dynamic_queue_with_following_callbacks() {
    for min in [6_016, 8_192] {
        let supported = cpal::SupportedStreamConfig::new(
            /*channels*/ 1,
            384_000,
            cpal::SupportedBufferSize::Range { min, max: 16_384 },
            cpal::SampleFormat::F32,
        );
        let cpal::BufferSize::Fixed(frames) =
            bounded_stream_config(&supported).unwrap().buffer_size
        else {
            panic!("callback size must be bounded");
        };
        let buffers = Buffers::new(/*input_rate*/ 48_000, /*output_rate*/ 384_000);
        buffers.serviced.store(true, Ordering::Release);
        let mut output = OutputState::default();
        let start = Instant::now();
        // An existing partial block, a full callback, then 5 ms of 128-frame callbacks.
        for count in [BLOCK - 1, frames as usize].into_iter().chain([128; 15]) {
            render_output(
                &mut vec![0.0_f32; count],
                /*channels*/ 1,
                /*rate*/ 384_000.0,
                start,
                &buffers,
                &mut output,
            );
        }
        assert_eq!(
            buffers.rendered.len(),
            (BLOCK - 1 + frames as usize + 15 * 128) / BLOCK,
        );
        assert!(buffers.rendered.len() < buffers.rendered.capacity());
        assert!(!buffers.failed.load(Ordering::Acquire));
    }
}

#[test]
fn unsupported_callback_ranges_do_not_fall_back_to_default() {
    for range in [
        cpal::SupportedBufferSize::Unknown,
        cpal::SupportedBufferSize::Range { min: 0, max: 0 },
        cpal::SupportedBufferSize::Range {
            min: 1_024,
            max: 512,
        },
        cpal::SupportedBufferSize::Range {
            min: 8_193,
            max: 16_384,
        },
    ] {
        let supported = cpal::SupportedStreamConfig::new(
            /*channels*/ 2,
            96_000,
            range,
            cpal::SampleFormat::F32,
        );
        assert!(bounded_stream_config(&supported).is_err(), "{range:?}");
    }
}

#[test]
fn callback_timing_must_fit_service_and_stale_limits() {
    for (rate, min, max) in [(8_000, 8_192, 8_192), (8_000, 7_704, 8_192)] {
        let supported = cpal::SupportedStreamConfig::new(
            /*channels*/ 2,
            rate,
            cpal::SupportedBufferSize::Range { min, max },
            cpal::SampleFormat::F32,
        );
        assert!(
            bounded_stream_config(&supported).is_err(),
            "rate={rate}, range={min}..={max}",
        );
    }
}

#[test]
fn callback_pair_must_fit_the_processing_deadline() {
    for (input_rate, input_frames, output_rate, output_frames, accepted) in [
        (8_000, 7_680, 48_000, 480, false),
        (8_000, 2_000, 8_000, 2_000, false),
        (48_000, 480, 48_000, 480, true),
        (44_100, 441, 48_000, 480, true),
        (8_000, 1_600, 48_000, 480, true),
    ] {
        for (rate, frames) in [(input_rate, input_frames), (output_rate, output_frames)] {
            let supported = cpal::SupportedStreamConfig::new(
                /*channels*/ 1,
                rate,
                cpal::SupportedBufferSize::Range {
                    min: frames,
                    max: frames,
                },
                cpal::SampleFormat::F32,
            );
            assert!(bounded_stream_config(&supported).is_ok());
        }
        let processor = processing::Processor::new(input_rate, output_rate).unwrap();
        assert_eq!(
            processor
                .validate_callback_timing(input_frames, output_frames)
                .is_ok(),
            accepted,
            "input={input_rate}/{input_frames}, output={output_rate}/{output_frames}",
        );
    }
}

#[test]
fn tiny_output_callbacks_pack_references_without_delaying_rendering() {
    let buffers = Buffers::new(/*input_rate*/ 48_000, /*output_rate*/ 384_000);
    buffers.serviced.store(true, Ordering::Release);
    Buffers::set_disabled(&buffers.speaker, /*disabled*/ false).unwrap();
    let generation = buffers.speaker.load(Ordering::Acquire);
    let start = Instant::now();
    for _ in 0..8 {
        assert!(
            buffers
                .playback
                .push(Frame {
                    samples: [0.5; BLOCK],
                    len: BLOCK,
                    at: start,
                    generation,
                })
                .is_ok()
        );
    }
    let mut state = OutputState::default();
    for callback in 0..120 {
        let mut output = [0.0_f32; 32];
        render_output(
            &mut output,
            /*channels*/ 2,
            /*rate*/ 384_000.0,
            start + Duration::from_secs_f64((callback * 16) as f64 / 384_000.0),
            &buffers,
            &mut state,
        );
        assert_eq!(output, [0.5; 32]);
    }
    assert_eq!(buffers.rendered.len(), 7);
    assert!(!buffers.failed.load(Ordering::Acquire));
}

#[test]
fn rendered_queue_retains_high_rate_callbacks_during_bounded_service_pause() {
    let buffers = Buffers::new(/*input_rate*/ 48_000, /*output_rate*/ 384_000);
    buffers.serviced.store(true, Ordering::Release);
    let mut state = OutputState::default();
    let start = Instant::now();
    let samples = 384_000 * 63 / 100;
    for offset in (0..samples).step_by(17) {
        let mut output = [1.0_f32; 34];
        let len = (samples - offset).min(17) * 2;
        render_output(
            &mut output[..len],
            /*channels*/ 2,
            /*rate*/ 384_000.0,
            start + Duration::from_secs_f64(offset as f64 / 384_000.0),
            &buffers,
            &mut state,
        );
        assert_eq!(&output[..len], &[0.0; 34][..len]);
        assert!(!buffers.failed.load(Ordering::Acquire));
    }
    assert_eq!(buffers.rendered.len(), samples / BLOCK);
}

#[test]
fn suppressed_output_does_not_fail_when_reference_queue_is_full() {
    let buffers = Buffers::new(/*input_rate*/ 48_000, /*output_rate*/ 384_000);
    buffers.serviced.store(true, Ordering::Release);
    let mut state = OutputState::default();
    let mut output = [1.0_f32; BLOCK];
    for _ in 0..buffers.rendered.capacity() * 3 {
        render_output(
            &mut output,
            /*channels*/ 1,
            /*rate*/ 384_000.0,
            Instant::now(),
            &buffers,
            &mut state,
        );
        assert_eq!(output, [0.0; BLOCK]);
    }
    assert_eq!(buffers.rendered.len(), buffers.rendered.capacity());
    assert!(!buffers.failed.load(Ordering::Acquire));
    Buffers::set_disabled(&buffers.speaker, /*disabled*/ false).unwrap();
    render_output(
        &mut output,
        /*channels*/ 1,
        /*rate*/ 384_000.0,
        Instant::now(),
        &buffers,
        &mut state,
    );
    assert!(buffers.failed.load(Ordering::Acquire));
}

#[test]
fn stream_xruns_recover_without_clearing_device_failure() {
    let buffers = Buffers::new(/*input_rate*/ 48_000, /*output_rate*/ 48_000);
    handle_stream_error(&buffers, cpal::ErrorKind::Xrun.into());
    assert!(!buffers.failed.load(Ordering::Acquire));
    handle_stream_error(&buffers, cpal::ErrorKind::DeviceNotAvailable.into());
    assert!(buffers.failed.load(Ordering::Acquire));
    handle_stream_error(&buffers, cpal::ErrorKind::Xrun.into());
    assert!(buffers.failed.load(Ordering::Acquire));
}
