use super::*;
use pretty_assertions::assert_eq;

#[test]
fn unmute_rejects_backlog_until_capture_reaches_the_following_callback() {
    let mut boundary = CaptureBoundary::default();
    // Times are offsets in the same device clock. The first callback's timestamp
    // can precede unmute, so even capture after that timestamp must be rejected.
    for (generation, callback, capture, expected) in [
        (1, 20, 10, false), // muted
        (2, 21, 15, false), // first observed unmute
        (2, 25, 22, false), // next callback establishes cutoff 25
        (2, 26, 24, false), // another old/crossing buffer
        (2, 27, 25, true),
        (2, 28, 26, true),  // same generation must not move the cutoff
        (2, 29, 25, false), // backwards device capture is a discontinuity
        (2, 30, 26, true),  // forward capture can resume without moving the cutoff
        (4, 30, 29, false), // mute/unmute happened without a callback
        (4, 34, 31, false),
        (4, 35, 33, false),
        (4, 36, 34, true),
        (5, 37, 35, false), // observed mute
        (6, 38, 36, false),
        (6, 40, 40, true),
        (6, 101, 100, true),
        (6, 102, 90, false),
        (6, 103, 95, false), // rejection must not lower the last accepted timestamp
        (6, 104, 101, true),
    ] {
        assert_eq!(
            boundary.accepts(
                generation,
                Duration::from_millis(callback),
                Duration::from_millis(capture),
            ),
            expected,
            "generation={generation}, callback={callback}, capture={capture}",
        );
    }
}

#[test]
fn suppression_discards_partial_and_queued_previous_generations() {
    let buffers = Buffers::new(/*input_rate*/ 48_000, /*output_rate*/ 48_000);
    let mut playback = Playback::default();
    Buffers::set_disabled(&buffers.speaker, /*disabled*/ false).unwrap();
    let generation = buffers.speaker.load(Ordering::Acquire);
    for _ in 0..2 {
        assert!(
            buffers
                .playback
                .push(Frame {
                    samples: [0.5; BLOCK],
                    len: BLOCK,
                    at: Instant::now(),
                    generation
                })
                .is_ok()
        );
    }
    assert_eq!(playback.next(&buffers), 0.5);
    Buffers::set_disabled(&buffers.speaker, /*disabled*/ true).unwrap();
    assert_eq!(playback.next(&buffers), 0.0);
    Buffers::set_disabled(&buffers.speaker, /*disabled*/ false).unwrap();
    assert_eq!(playback.next(&buffers), 0.0);
    assert!(buffers.playback.is_empty());
}

#[test]
fn output_is_finite_and_bounded_and_underflow_is_silence() {
    let buffers = Buffers::new(/*input_rate*/ 48_000, /*output_rate*/ 48_000);
    Buffers::set_disabled(&buffers.speaker, /*disabled*/ false).unwrap();
    let generation = buffers.speaker.load(Ordering::Acquire);
    let mut samples = [0.0; BLOCK];
    samples[..4].copy_from_slice(&[f32::NAN, f32::INFINITY, -2.0, 2.0]);
    assert!(
        buffers
            .playback
            .push(Frame {
                samples,
                len: 4,
                at: Instant::now(),
                generation
            })
            .is_ok()
    );
    let mut playback = Playback::default();
    let output: Vec<_> = (0..5).map(|_| playback.next(&buffers)).collect();
    assert_eq!(output, [0.0, 0.0, -1.0, 1.0, 0.0]);
}

#[test]
fn repeated_controls_preserve_epoch_but_transitions_invalidate_capture() {
    let buffers = Buffers::new(/*input_rate*/ 48_000, /*output_rate*/ 48_000);
    Buffers::set_disabled(&buffers.microphone, /*disabled*/ false).unwrap();
    let first = buffers.microphone.load(Ordering::Acquire);
    Buffers::set_disabled(&buffers.microphone, /*disabled*/ false).unwrap();
    assert_eq!(buffers.microphone.load(Ordering::Acquire), first);
    Buffers::set_disabled(&buffers.microphone, /*disabled*/ true).unwrap();
    Buffers::set_disabled(&buffers.microphone, /*disabled*/ false).unwrap();
    assert_ne!(buffers.microphone.load(Ordering::Acquire), first);
}

#[test]
fn tiny_callbacks_share_slots_and_preserve_the_oldest_timestamp() {
    let buffers = Buffers::new(/*input_rate*/ 48_000, /*output_rate*/ 48_000);
    let start = Instant::now();
    for queue in [&buffers.capture, &buffers.rendered] {
        let mut packer = FramePacker::default();
        // Five milliseconds at 384 kHz arrives as 120 sixteen-frame callbacks.
        for callback in 0..120 {
            assert!(packer.push(
                Frame {
                    samples: [callback as f32; BLOCK],
                    len: 16,
                    at: start + Duration::from_secs_f64((callback * 16) as f64 / 384_000.0),
                    generation: 2,
                },
                /*rate*/ 384_000.0,
                queue,
            ));
        }
        assert_eq!(queue.len(), 7);
        for block in 0..7 {
            let frame = queue.pop().unwrap();
            assert_eq!(
                (frame.samples, frame.len, frame.at, frame.generation),
                (
                    std::array::from_fn(|sample| (block * 16 + sample / 16) as f32),
                    BLOCK,
                    start + Duration::from_secs_f64((block * BLOCK) as f64 / 384_000.0),
                    2,
                ),
            );
        }
    }
}

#[test]
fn packing_drops_partial_audio_on_generation_changes_and_capture_rejection() {
    let queue = ArrayQueue::new(QUEUE_CAPACITY);
    let mut packer = FramePacker::default();
    let start = Instant::now();
    for (generation, sample) in [(2, 0.1), (4, 0.2), (4, 0.3)] {
        assert!(packer.push(
            Frame {
                samples: [sample; BLOCK],
                len: BLOCK / 2,
                at: start,
                generation,
            },
            /*rate*/ 48_000.0,
            &queue,
        ));
    }
    let frame = queue.pop().unwrap();
    assert_eq!(
        (frame.samples, frame.len, frame.at, frame.generation),
        (
            std::array::from_fn(|i| if i < BLOCK / 2 { 0.2 } else { 0.3 }),
            BLOCK,
            start,
            4
        ),
    );
    for sample in [0.4, 0.5] {
        assert!(packer.push(
            Frame {
                samples: [sample; BLOCK],
                len: BLOCK / 2,
                at: start,
                generation: 4
            },
            /*rate*/ 48_000.0,
            &queue,
        ));
        packer.reset();
    }
    assert!(queue.is_empty());
}

#[test]
fn packing_preserves_a_remainder_timestamp_across_uneven_callbacks() {
    let queue = ArrayQueue::new(QUEUE_CAPACITY);
    let mut packer = FramePacker::default();
    let start = Instant::now();
    let second = start + Duration::from_millis(10);
    for (len, sample, at) in [(100, 0.1, start), (200, 0.2, second), (212, 0.3, second)] {
        assert!(packer.push(
            Frame {
                samples: [sample; BLOCK],
                len,
                at,
                generation: 2
            },
            /*rate*/ 48_000.0,
            &queue,
        ));
    }
    let first = queue.pop().unwrap();
    let remainder = queue.pop().unwrap();
    assert_eq!(
        (first.samples, first.at, remainder.samples, remainder.at),
        (
            std::array::from_fn(|i| if i < 100 { 0.1 } else { 0.2 }),
            start,
            std::array::from_fn(|i| if i < 44 { 0.2 } else { 0.3 }),
            second + Duration::from_secs_f64(156.0 / 48_000.0),
        ),
    );
    assert!(queue.is_empty());
}

#[test]
fn capture_queue_retains_high_rate_audio_during_bounded_service_pause() {
    let buffers = Buffers::new(/*input_rate*/ 384_000, /*output_rate*/ 48_000);
    let mut packer = FramePacker::default();
    let start = Instant::now();
    // 630 ms covers a pending batch's 500 ms deadline, an in-flight 100 ms
    // send and callback/service margins. Consumption is deliberately paused.
    let samples = 384_000 * 63 / 100;
    for offset in (0..samples).step_by(17) {
        assert!(packer.push(
            Frame {
                samples: [0.25; BLOCK],
                len: (samples - offset).min(17),
                at: start + Duration::from_secs_f64(offset as f64 / 384_000.0),
                generation: 2,
            },
            /*rate*/ 384_000.0,
            &buffers.capture,
        ));
    }
    assert_eq!(buffers.capture.len(), samples / BLOCK);
    // Enlarging the queue must not turn overflow into unbounded accumulation.
    for _ in buffers.capture.len()..=buffers.capture.capacity() {
        let available = buffers.capture.len() < buffers.capture.capacity();
        assert_eq!(
            packer.push(
                Frame {
                    samples: [0.25; BLOCK],
                    len: BLOCK,
                    at: start,
                    generation: 2,
                },
                /*rate*/ 384_000.0,
                &buffers.capture,
            ),
            available,
        );
    }
}
