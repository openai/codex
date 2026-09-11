use super::*;
use crate::devices::buffers::Playback;
use pretty_assertions::assert_eq;

fn active(rate: u32) -> (Arc<Buffers>, PlaybackPort, PlaybackWriter) {
    let buffers = Arc::new(Buffers::new(rate, rate));
    Buffers::set_disabled(&buffers.speaker, /*disabled*/ false).unwrap();
    let port = PlaybackPort::new(buffers.clone(), rate);
    let writer = port.writer();
    (buffers, port, writer)
}

#[test]
fn partial_writes_account_for_samples_until_the_device_consumes_them() {
    let (buffers, _, writer) = active(/*rate*/ 48000);
    let samples: Vec<_> = (0..BLOCK + 20).map(|i| i as f32 / 512.0).collect();
    let bytes: Vec<_> = samples.iter().flat_map(|v| v.to_le_bytes()).collect();
    let first = writer.write(&bytes).unwrap();
    assert_eq!(first, BLOCK * 4);
    assert_eq!(writer.write(&bytes[first..]).unwrap(), 20 * 4);
    assert_eq!(
        (writer.rate(), writer.delay()),
        (48000, (BLOCK + 20) as u32)
    );
    let mut playback = Playback::default();
    let actual: Vec<_> = (0..samples.len())
        .map(|_| playback.next(&buffers).unwrap())
        .collect();
    assert_eq!(actual, samples);
    assert_eq!(writer.delay(), 0);
    assert_eq!(playback.next(&buffers), None);
}

#[test]
fn suppression_cancels_a_full_writer_and_old_writers_cannot_resume() {
    let (buffers, port, writer) = active(/*rate*/ 8000);
    let bytes = vec![0; BLOCK * 4];
    for _ in 0..if cfg!(target_os = "linux") { 3 } else { 1 } {
        writer.write(&bytes).unwrap();
    }
    let waiting = std::thread::spawn(move || writer.write(&bytes));
    Buffers::set_disabled(&buffers.speaker, /*disabled*/ true).unwrap();
    assert_eq!(waiting.join().unwrap(), Err("speaker writer cancelled"));
    let stale = port.writer();
    Buffers::set_disabled(&buffers.speaker, /*disabled*/ false).unwrap();
    assert_eq!(stale.write(&[0; 4]), Err("speaker writer cancelled"));
    let mut playback = Playback::default();
    assert_eq!(playback.next(&buffers), None);
    assert_eq!(buffers.queued.load(Ordering::Acquire), 0);
    assert_eq!(port.writer().write(&[0; 4]), Ok(4));
}

#[test]
fn delay_includes_pending_dac_time_but_not_another_generation() {
    let (buffers, _, writer) = active(/*rate*/ 48000);
    writer.write(&[0; 40]).unwrap();
    let end = buffers.clock.elapsed() + Duration::from_millis(/*millis*/ 100);
    buffers
        .last_dac_ns
        .store(end.as_nanos() as u64, Ordering::Release);
    let delay = writer.delay();
    assert!((10..=4810).contains(&delay));
    assert!(delay > 10);
    Buffers::set_disabled(&buffers.speaker, /*disabled*/ true).unwrap();
    assert_eq!(writer.delay(), 0);
}

#[test]
fn retired_sink_failure_cannot_fail_the_next_speaker_epoch() {
    let (buffers, port, writer) = active(/*rate*/ 48_000);
    let transition = buffers.speaker_failure_gate.lock().unwrap();
    let (started, ready) = std::sync::mpsc::sync_channel(/*bound*/ 0);
    let failing = std::thread::spawn(move || {
        started.send(()).unwrap();
        writer.fail_if_current();
    });
    ready.recv().unwrap();
    // Hold the same transition gate while retiring the old writer, then let
    // the pending native failure inspect its now-stale epoch.
    Buffers::set_disabled(&buffers.speaker, /*disabled*/ true).unwrap();
    drop(transition);
    failing.join().unwrap();
    assert!(!buffers.failed.load(Ordering::Acquire));

    buffers.set_speaker_disabled(/*disabled*/ false).unwrap();
    port.writer().fail_if_current();
    assert!(buffers.failed.load(Ordering::Acquire));
}

#[test]
fn delay_contention_preserves_the_last_snapshot_without_failing_the_device() {
    let (buffers, port, writer) = active(/*rate*/ 48000);
    writer.write(&[0; 40]).unwrap();
    assert_eq!(writer.delay(), 10);
    buffers
        .callback_sequence
        .store(/*val*/ 1, Ordering::Release);
    let mut playback = Playback::default();
    let consumed: Vec<_> = (0..10).map(|_| playback.next(&buffers).unwrap()).collect();
    assert_eq!(consumed, vec![0.0; 10]);
    assert_eq!(
        (writer.delay(), buffers.failed.load(Ordering::Acquire)),
        (10, false)
    );
    buffers
        .callback_sequence
        .store(/*val*/ 2, Ordering::Release);
    assert_eq!(writer.delay(), 0);

    writer.write(&[0; 20]).unwrap();
    assert_eq!(writer.delay(), 5);
    buffers
        .callback_sequence
        .store(/*val*/ 3, Ordering::Release);
    Buffers::set_disabled(&buffers.speaker, /*disabled*/ true).unwrap();
    assert_eq!(writer.delay(), 0);
    Buffers::set_disabled(&buffers.speaker, /*disabled*/ false).unwrap();
    assert_eq!(
        (
            port.writer().delay(),
            buffers.failed.load(Ordering::Acquire)
        ),
        (0, false)
    );
}

#[test]
fn invalid_samples_fail_but_stalled_consumption_drops_and_resumes() {
    let (buffers, _, writer) = active(/*rate*/ 8000);
    assert_eq!(
        writer.write(&f32::NAN.to_le_bytes()),
        Err("invalid speaker sample")
    );
    let blocks = if cfg!(target_os = "linux") { 3 } else { 1 };
    for _ in 0..blocks {
        writer.write(&vec![0; BLOCK * 4]).unwrap();
    }
    assert_eq!(writer.write(&vec![0; BLOCK * 4]), Ok(BLOCK * 4));
    assert_eq!(
        buffers.queued.load(Ordering::Acquire),
        (blocks * BLOCK) as u32
    );
    let mut playback = Playback::default();
    for _ in 0..blocks * BLOCK {
        assert!(playback.next(&buffers).is_some());
    }
    assert_eq!(writer.write(&vec![0; BLOCK * 4]), Ok(BLOCK * 4));
    assert!(!buffers.failed.load(Ordering::Acquire));
}
