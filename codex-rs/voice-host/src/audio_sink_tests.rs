//! Verify that sink backpressure preserves PCM and native flushes cancel speaker writes.

use super::*;
use crate::devices::buffers::Buffers;
use crate::devices::buffers::Playback;
use crate::devices::playback::PlaybackPort;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

#[test]
fn queued_audio_survives_a_pause_before_the_next_buffer() {
    gst::init().unwrap();
    let buffers = Arc::new(Buffers::new(
        /*input_rate*/ 48_000, /*output_rate*/ 48_000,
    ));
    buffers.set_speaker_disabled(/*disabled*/ false).unwrap();
    let port = PlaybackPort::new(buffers.clone(), /*rate*/ 48_000);
    let source = gstreamer_app::AppSrc::builder()
        .is_live(true)
        .format(gst::Format::Time)
        .caps(
            &gst::Caps::builder("audio/x-raw")
                .field("format", "F32LE")
                .field("layout", "interleaved")
                .field("channels", 1i32)
                .field("rate", 48_000i32)
                .build(),
        )
        .build();
    let sink = Sink::new(port.writer());
    let pipeline = gst::Pipeline::new();
    pipeline
        .add_many([source.upcast_ref::<gst::Element>(), sink.upcast_ref()])
        .unwrap();
    source.link(&sink).unwrap();
    pipeline.set_state(gst::State::Playing).unwrap();
    let first_samples = 960;
    source
        .push_buffer(gst::Buffer::from_slice(
            0.25_f32.to_le_bytes().repeat(first_samples),
        ))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(/*secs*/ 3);
    while buffers.queued.load(Ordering::Acquire) < first_samples as u32 && Instant::now() < deadline
    {
        std::thread::park_timeout(Duration::from_millis(/*millis*/ 1));
    }
    // Leave room for both buffers so a slow test runner cannot trigger the
    // writer's bounded-wait timeout. An independent output ring must not
    // snapshot silence over the next buffer's samples during the pause.
    std::thread::sleep(Duration::from_millis(/*millis*/ 20));
    source
        .push_buffer(gst::Buffer::from_slice(0.5_f32.to_le_bytes().repeat(960)))
        .unwrap();
    std::thread::sleep(Duration::from_millis(/*millis*/ 20));
    let mut playback = Playback::default();
    let mut actual = Vec::new();
    while actual.len() < first_samples + 960 && Instant::now() < deadline {
        for _ in 0..480 {
            if let Some(sample) = playback.next(&buffers) {
                actual.push(sample);
            }
        }
        std::thread::park_timeout(Duration::from_millis(/*millis*/ 1));
    }
    buffers.set_speaker_disabled(/*disabled*/ true).unwrap();
    pipeline.set_state(gst::State::Null).unwrap();
    let expected: Vec<_> = std::iter::repeat_n(0.25, first_samples)
        .chain(std::iter::repeat_n(0.5, 960))
        .collect();
    assert_eq!(actual, expected);
    assert!(!buffers.failed.load(Ordering::Acquire));
}

#[test]
fn unexpected_flush_cancels_a_blocked_speaker_write() {
    gst::init().unwrap();
    let buffers = Arc::new(Buffers::new(
        /*input_rate*/ 48_000, /*output_rate*/ 48_000,
    ));
    buffers.set_speaker_disabled(/*disabled*/ false).unwrap();
    let port = PlaybackPort::new(buffers.clone(), /*rate*/ 48_000);
    let sink = Sink::new(port.writer());
    sink.set_state(gst::State::Playing).unwrap();
    let capacity = if cfg!(target_os = "linux") {
        4_800
    } else {
        1_920
    };
    sink.imp()
        .render(&gst::Buffer::from_slice(vec![0u8; capacity * 4]))
        .unwrap();
    let blocked_sink = sink.clone();
    let blocked = std::thread::spawn(move || {
        blocked_sink
            .imp()
            .render(&gst::Buffer::from_slice([0u8; 4]))
    });
    // The writer holds this lock while waiting for space in the full queue.
    let deadline = Instant::now() + Duration::from_secs(/*secs*/ 1);
    while !matches!(
        port.0.producer.try_lock(),
        Err(std::sync::TryLockError::WouldBlock)
    ) {
        assert!(Instant::now() < deadline, "speaker write did not block");
        std::thread::yield_now();
    }
    assert!(
        sink.static_pad("sink")
            .unwrap()
            .send_event(gst::event::FlushStart::new())
    );
    let result = blocked.join().unwrap();
    sink.set_state(gst::State::Null).unwrap();
    assert_eq!(result, Err(gst::FlowError::Error));
    assert!(buffers.failed.load(Ordering::Acquire));
}
