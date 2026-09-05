use super::*;
use pretty_assertions::assert_eq;

#[test]
fn rendered_reference_changes_decoded_echo_capture() {
    let mut processors = [
        Processor::new(/*input_rate*/ 48_000, /*output_rate*/ 44_100).unwrap(),
        Processor::new(/*input_rate*/ 48_000, /*output_rate*/ 44_100).unwrap(),
    ];
    for processor in &mut processors {
        // Isolate echo cancellation from independent noise/gain adaptation.
        processor.apm.apply_config(sonora::Config {
            echo_canceller: Some(EchoCanceller::default()),
            ..Default::default()
        });
    }
    let start = Instant::now();
    let signal = |seconds: f32| {
        [311.0, 719.0, 1_423.0]
            .into_iter()
            .map(|frequency| (std::f32::consts::TAU * frequency * seconds).sin() * 0.1)
            .sum::<f32>()
    };
    let mut packets = [Vec::new(), Vec::new()];
    // Two seconds of deterministic samples allow AEC startup, without sleeping.
    for block in 0..200 {
        for offset in [0, 256] {
            let mut frame = Frame {
                samples: [0.0; 256],
                len: 256.min(441 - offset),
                at: start - Duration::from_secs(/*secs*/ 10)
                    + Duration::from_secs_f64((block * 441 + offset) as f64 / 44_100.0),
                generation: 0,
            };
            for (index, sample) in frame.samples[..frame.len].iter_mut().enumerate() {
                *sample = signal((block * 441 + offset + index) as f32 / 44_100.0);
            }
            processors[0].render(&frame).unwrap();
        }
        // Keep delay metadata identical: only consuming reference samples may
        // explain the difference in the decoded capture signal.
        processors[1].render_delay = processors[0].render_delay;
        for offset in [0, 256] {
            let mut frame = Frame {
                samples: [0.0; 256],
                len: 256.min(480 - offset),
                at: start + Duration::from_secs_f64((block * 480 + offset) as f64 / 48_000.0),
                generation: 2,
            };
            for (index, sample) in frame.samples[..frame.len].iter_mut().enumerate() {
                *sample = signal((block * 480 + offset + index) as f32 / 48_000.0 - 0.05);
            }
            for (processor, packets) in processors.iter_mut().zip(&mut packets) {
                packets.extend(processor.capture(&frame, || frame.at).unwrap());
            }
        }
    }
    let decoded = packets.map(|packets| {
        let mut decoder = opus::Decoder::new(/*sample_rate*/ 48_000, opus::Channels::Mono).unwrap();
        let mut decoded = Vec::new();
        for packet in packets {
            let mut samples = [0.0; 960];
            assert_eq!(
                decoder
                    .decode_float(&packet.data, &mut samples, /*fec*/ false)
                    .unwrap(),
                960
            );
            assert!(samples.iter().all(|sample| sample.is_finite()));
            decoded.extend(samples);
        }
        decoded
    });
    assert_eq!(decoded[0].len(), decoded[1].len());
    assert!(decoded[0].len() > 48_000);
    let energy = decoded.map(|samples| {
        samples[48_000..]
            .iter()
            .map(|sample| sample * sample)
            .sum::<f32>()
    });
    assert!(
        energy[1] > 1.0,
        "control capture must contain the echo signal"
    );
    assert!(
        energy[0] < energy[1] * 0.5,
        "reference/control echo energy: {energy:?}"
    );
}

#[test]
fn real_encoder_produces_twenty_millisecond_opus_packets() {
    for rate in [44_100, 48_000] {
        let mut processor = Processor::new(rate, /*output_rate*/ 48_000).unwrap();
        let mut packets = Vec::new();
        for block in 0..40 {
            let mut frame = Frame {
                samples: [0.0; 256],
                len: 256,
                at: Instant::now(),
                generation: 2,
            };
            for (index, sample) in frame.samples.iter_mut().enumerate() {
                *sample = (std::f32::consts::TAU * 440.0 * (block as usize * 256 + index) as f32
                    / rate as f32)
                    .sin()
                    * 0.1;
            }
            packets.extend(processor.capture(&frame, || frame.at).unwrap());
        }
        assert!(!packets.is_empty());
        let mut decoder = opus::Decoder::new(/*sample_rate*/ 48_000, opus::Channels::Mono).unwrap();
        let mut energy = 0.0_f32;
        for packet in packets {
            let mut output = [0.0; 960];
            assert_eq!(
                decoder
                    .decode_float(&packet.data, &mut output, /*fec*/ false)
                    .unwrap(),
                960
            );
            assert!(output.iter().all(|sample| sample.is_finite()));
            energy += output.iter().map(|sample| sample * sample).sum::<f32>();
        }
        assert!(
            energy > 0.01,
            "decoded capture must contain signal at {rate} Hz"
        );
    }
}

#[test]
fn unmute_reset_rejects_delayed_pre_unmute_audio_and_partial_history() {
    let mut processor = Processor::new(/*input_rate*/ 48_000, /*output_rate*/ 48_000).unwrap();
    let frame = Frame {
        samples: [0.25; 256],
        len: 256,
        at: Instant::now(),
        generation: 2,
    };
    assert!(processor.capture(&frame, || frame.at).unwrap().is_empty());
    assert_eq!(
        (0..10).find_map(|_| processor
            .capture(&frame, || frame.at + Duration::from_secs(1))
            .err()),
        Some("voice processing fell behind")
    );
    processor.reset().unwrap();
    for _ in 0..10 {
        assert!(processor.capture(&frame, || frame.at).unwrap().is_empty());
    }
    assert!(processor.pending.is_empty());
    assert!(processor.capture.input.is_empty());
    assert!(processor.capture.output.is_empty());
}

#[test]
fn capture_gap_discards_old_partial_packet_and_preserves_unmute_cutoff() {
    let mut processor = Processor::new(/*input_rate*/ 48_000, /*output_rate*/ 48_000).unwrap();
    let mut fresh = Processor::new(/*input_rate*/ 48_000, /*output_rate*/ 48_000).unwrap();
    let cutoff = processor.cutoff;
    let start = Instant::now();
    let mut frame = Frame {
        samples: [0.25; 256],
        len: 240,
        at: start,
        generation: 2,
    };
    for index in 0..5 {
        frame.at = start + Duration::from_millis(index * 5);
        assert!(processor.capture(&frame, || frame.at).unwrap().is_empty());
    }
    assert!(!processor.pending.is_empty());
    assert!(!processor.capture.input.is_empty());

    let resumed = start + Duration::from_millis(/*millis*/ 100);
    frame.samples.fill(0.0);
    let mut packets = Vec::new();
    let mut expected = Vec::new();
    for index in 0..12 {
        frame.at = resumed + Duration::from_millis(index * 5);
        let emitted = processor.capture(&frame, || frame.at).unwrap();
        if index < 2 {
            assert!(emitted.is_empty());
        }
        packets.extend(emitted.into_iter().map(|packet| (packet.at, packet.data)));
        expected.extend(
            fresh
                .capture(&frame, || frame.at)
                .unwrap()
                .into_iter()
                .map(|packet| (packet.at, packet.data)),
        );
    }
    assert!(!packets.is_empty());
    assert_eq!(packets, expected);
    assert_eq!(processor.cutoff, cutoff);
}

#[test]
fn ordinary_capture_timestamp_jitter_keeps_partial_audio() {
    for jitter_ms in [-2_i64, 0, 2] {
        let mut processor = Processor::new(/*input_rate*/ 48_000, /*output_rate*/ 48_000).unwrap();
        let start = Instant::now();
        let mut frame = Frame {
            samples: [0.1; 256],
            len: 240,
            at: start,
            generation: 2,
        };
        for index in 0..5 {
            frame.at = start + Duration::from_millis(index * 5);
            assert!(processor.capture(&frame, || frame.at).unwrap().is_empty());
        }
        frame.at = start + Duration::from_millis((25 + jitter_ms) as u64);
        assert_eq!(processor.capture(&frame, || frame.at).unwrap().len(), 1);
    }
}

#[test]
fn callback_gap_cannot_hide_in_a_packed_partial_block() {
    let mut processor = Processor::new(/*input_rate*/ 48_000, /*output_rate*/ 48_000).unwrap();
    let mut packer = super::super::buffers::FramePacker::default();
    let queue = crossbeam_queue::ArrayQueue::new(/*capacity*/ 8);
    let start = Instant::now();
    // Leave both an incomplete callback block and a processed half-packet.
    for index in 0..6 {
        let at = start + Duration::from_millis(index * 5);
        packer.discard_capture_gap(at, /*rate*/ 48_000.0);
        assert!(packer.push(
            Frame {
                samples: [0.25; 256],
                len: 240,
                at,
                generation: 2
            },
            /*rate*/ 48_000.0,
            &queue
        ));
        while let Some(frame) = queue.pop() {
            assert!(processor.capture(&frame, || frame.at).unwrap().is_empty());
        }
    }
    assert!(!processor.pending.is_empty());
    let resumed = start + Duration::from_millis(/*millis*/ 100);
    packer.discard_capture_gap(resumed, /*rate*/ 48_000.0);
    assert!(packer.push(
        Frame {
            samples: [0.0; 256],
            len: 256,
            at: resumed,
            generation: 2
        },
        /*rate*/ 48_000.0,
        &queue
    ));
    let frame = queue.pop().unwrap();
    assert_eq!(
        (frame.samples, frame.len, frame.at, frame.generation),
        ([0.0; 256], 256, resumed, 2)
    );
    assert!(processor.capture(&frame, || resumed).unwrap().is_empty());
    assert!(processor.pending.is_empty());
}

#[test]
fn stale_echo_reference_cannot_cancel_capture_age() {
    for render_delay in [0, -750] {
        let mut processor = Processor::new(/*input_rate*/ 48_000, /*output_rate*/ 48_000).unwrap();
        processor.render_delay = render_delay;
        let start = Instant::now();
        let now = start + Duration::from_millis(/*millis*/ 750);
        let failure = (0..8).find_map(|index| {
            let frame = Frame {
                samples: [0.25; 256],
                len: 256,
                at: start + Duration::from_secs_f64((index * 256) as f64 / 48_000.0),
                generation: 2,
            };
            processor.capture(&frame, || now).err()
        });
        assert_eq!(failure, Some("voice processing fell behind"));
    }
}
