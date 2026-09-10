use super::super::buffers::BLOCK;
use super::super::buffers::Frame;
use super::*;
use crate::audio_track::AudioTrack;
use crate::transport::Transport;
use codex_realtime_webrtc::AudioControls;
use futures::future::BoxFuture;
use pretty_assertions::assert_eq;
use rtc::interceptor::Interceptor;
use rtc::interceptor::NoopInterceptor;
use rtc::interceptor::Packet;
use rtc::interceptor::StreamInfo;
use rtc::interceptor::TaggedPacket;
use rtc::interceptor::interceptor;
use rtc::sansio;
use rtc::shared::error::Error;
use std::time::Duration;
use tokio::sync::Notify;
use tokio::sync::mpsc;
use tokio::time::timeout;
use webrtc::peer_connection::PeerConnection;
use webrtc::peer_connection::PeerConnectionBuilder;
use webrtc::peer_connection::PeerConnectionEventHandler;
use webrtc::peer_connection::RTCIceGatheringState;
use webrtc::peer_connection::RTCSessionDescription;

// Consume remote RTP before the upstream track queue: receipt proves the packet
// traversed negotiation, encryption, the loopback socket, and remote decryption.
#[derive(Interceptor)]
struct RemoteRtp {
    #[next]
    next: NoopInterceptor,
    sender: mpsc::Sender<rtc::rtp::packet::Packet>,
}

#[interceptor]
impl RemoteRtp {
    #[overrides]
    fn handle_read(&mut self, message: TaggedPacket) -> Result<(), Self::Error> {
        if let Packet::Rtp(packet) = message.message {
            self.sender.try_send(packet).unwrap();
        }
        Ok(())
    }
}

struct Gathered(Arc<Notify>);

impl PeerConnectionEventHandler for Gathered {
    fn on_ice_gathering_state_change<'a, 'async_trait>(
        &'a self,
        state: RTCIceGatheringState,
    ) -> BoxFuture<'async_trait, ()>
    where
        'a: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            if state == RTCIceGatheringState::Complete {
                self.0.notify_one();
            }
        })
    }
}

// Synthetic callback output enters the same bounded queue as the real cpal
// callback. No hardware availability or physical microphone behavior is claimed.
fn enqueue(worker: &CaptureWorker, samples: usize, generation: u64, at: Instant) {
    for offset in (0..samples).step_by(BLOCK) {
        let mut frame = Frame {
            samples: [0.0; BLOCK],
            len: BLOCK.min(samples - offset),
            at: at + Duration::from_secs_f64(offset as f64 / 48_000.0),
            generation,
        };
        for (index, sample) in frame.samples.iter_mut().enumerate() {
            *sample =
                (std::f32::consts::TAU * 440.0 * (offset + index) as f32 / 48_000.0).sin() * 0.25;
        }
        assert!(worker.buffers.capture.push(frame).is_ok());
    }
}

#[test]
fn combined_mute_and_suppression_cancel_old_speaker_and_capture_work() {
    let buffers = Arc::new(Buffers::new(
        /*input_rate*/ 48_000, /*output_rate*/ 48_000,
    ));
    let port = super::super::playback::PlaybackPort::new(buffers.clone(), /*rate*/ 48_000);
    let mut worker = CaptureWorker {
        buffers: buffers.clone(),
        processor: processing::Processor::new(
            /*input_rate*/ 48_000, /*output_rate*/ 48_000,
        )
        .unwrap(),
        pending: VecDeque::new(),
    };
    worker
        .set_controls(AudioControls {
            microphone_muted: false,
            speaker_suppressed: false,
        })
        .unwrap();
    worker.pending.push_back(crate::audio_track::EncodedAudio {
        data: vec![1],
        at: Instant::now(),
    });
    // A failed speaker transition must not start microphone reset first.
    buffers.speaker.store(u64::MAX, Ordering::Release);
    assert!(
        worker
            .set_controls(AudioControls {
                microphone_muted: true,
                speaker_suppressed: false,
            })
            .is_err()
    );
    assert_eq!(
        (
            buffers.speaker.load(Ordering::Acquire),
            buffers.microphone.load(Ordering::Acquire),
            worker.pending.len(),
        ),
        (u64::MAX, 2, 1)
    );
    buffers.speaker.store(/*val*/ 2, Ordering::Release);
    let writer = port.writer();
    worker
        .set_controls(AudioControls {
            microphone_muted: true,
            speaker_suppressed: true,
        })
        .unwrap();
    assert_eq!(
        (
            buffers.speaker.load(Ordering::Acquire),
            buffers.microphone.load(Ordering::Acquire),
            worker.pending.len(),
            writer.write(&[0; 4]),
        ),
        (3, 3, 0, Err("speaker writer cancelled"))
    );
}

async fn receive_capture(
    worker: &mut CaptureWorker,
    local: &mut Transport,
    received: &mut mpsc::Receiver<rtc::rtp::packet::Packet>,
    now: Instant,
) -> Vec<rtc::rtp::packet::Packet> {
    let mut count = 0;
    loop {
        let sent = worker.service(&mut local.audio, || now).await.unwrap();
        if sent == 0 {
            break;
        }
        assert_eq!(sent, 1);
        count += sent;
    }
    assert!(count > 0, "fresh capture must produce audio");
    let mut packets = Vec::new();
    let mut decoder = opus::Decoder::new(/*sample_rate*/ 48_000, opus::Channels::Mono).unwrap();
    let mut energy = 0.0_f32;
    for _ in 0..count {
        let packet = received.recv().await.unwrap();
        assert_eq!(packet.header.payload_type, 111);
        let mut decoded = [0.0; 960];
        assert_eq!(
            decoder
                .decode_float(&packet.payload, &mut decoded, /*fec*/ false)
                .unwrap(),
            960
        );
        assert!(decoded.iter().all(|sample| sample.is_finite()));
        energy += decoded.iter().map(|sample| sample * sample).sum::<f32>();
        packets.push(packet);
    }
    assert!(
        energy > 0.01,
        "decoded microphone signal must not be silence"
    );
    packets
}

#[tokio::test]
async fn capture_reaches_remote_rtp_and_mute_discards_queued_and_partial_audio() {
    timeout(Duration::from_secs(/*secs*/ 30), async {
        let (sender, mut received) = mpsc::channel(/*buffer*/ 32);
        let gathered = Arc::new(Notify::new());
        let (media, _) = AudioTrack::new().unwrap();
        let mut settings = webrtc::peer_connection::SettingEngine::default();
        settings.set_lite(/*lite*/ true);
        let remote = PeerConnectionBuilder::new()
            .with_media_engine(media)
            .with_setting_engine(settings)
            .with_handler(Arc::new(Gathered(gathered.clone())))
            .with_interceptor_registry(rtc::interceptor::Registry::from(RemoteRtp {
                next: NoopInterceptor::new(),
                sender,
            }))
            .with_udp_addrs(vec!["0.0.0.0:0"])
            .build()
            .await
            .unwrap();
        let mut local = Transport::new().await.unwrap();
        remote
            .set_remote_description(
                RTCSessionDescription::offer(local.offer().await.unwrap()).unwrap(),
            )
            .await
            .unwrap();
        let answer = remote.create_answer(/*options*/ None).await.unwrap();
        remote.set_local_description(answer).await.unwrap();
        gathered.notified().await;
        local
            .apply_answer(remote.local_description().await.unwrap().sdp)
            .await
            .unwrap();

        let mut worker = CaptureWorker {
            buffers: Arc::new(Buffers::new(
                /*input_rate*/ 48_000, /*output_rate*/ 48_000,
            )),
            pending: Default::default(),
            processor: processing::Processor::new(
                /*input_rate*/ 48_000, /*output_rate*/ 44_100,
            )
            .unwrap(),
        };
        let unmuted = || AudioControls {
            microphone_muted: false,
            speaker_suppressed: false,
        };
        let muted = || AudioControls {
            microphone_muted: true,
            speaker_suppressed: true,
        };
        worker.set_controls(unmuted()).unwrap();
        let generation = worker.buffers.microphone.load(Ordering::Acquire);
        // Freeze processing time so this transport test does not benchmark the CI worker.
        // Processor tests separately exercise the real backlog cutoff.
        let start = Instant::now();
        let mut now = start;
        // Feed a distinct speaker tone through the real render resampler and APM
        // before capture; this exercises a different output rate without hardware.
        for offset in (0..2_646).step_by(BLOCK) {
            let mut frame = Frame {
                samples: [0.0; BLOCK],
                len: BLOCK.min(2_646 - offset),
                at: start + Duration::from_secs_f64(offset as f64 / 44_100.0),
                generation: 0,
            };
            for (index, sample) in frame.samples.iter_mut().enumerate() {
                *sample = (std::f32::consts::TAU * 220.0 * (offset + index) as f32 / 44_100.0)
                    .sin()
                    * 0.1;
            }
            assert!(worker.buffers.rendered.push(frame).is_ok());
        }
        enqueue(&worker, /*samples*/ 2_880, generation, now);
        let before = receive_capture(&mut worker, &mut local, &mut received, now).await;
        assert!(worker.buffers.rendered.is_empty());

        // Start with a clean processor, then retain less than one Opus packet.
        // If either resampler or partial encoder history survives the next mute,
        // the equally short fresh segment below would emit audio too early.
        worker.set_controls(muted()).unwrap();
        worker.set_controls(unmuted()).unwrap();
        let old_generation = worker.buffers.microphone.load(Ordering::Acquire);
        now = Instant::now().max(start + Duration::from_millis(/*millis*/ 100));
        enqueue(&worker, /*samples*/ 720, old_generation, now);
        assert_eq!(worker.service(&mut local.audio, || now).await.unwrap(), 0);
        enqueue(&worker, /*samples*/ 2_880, old_generation, now);
        worker.set_controls(muted()).unwrap();
        assert_eq!(worker.service(&mut local.audio, || now).await.unwrap(), 0);
        let silence = received.recv().await.unwrap();
        let mut decoder = opus::Decoder::new(/*sample_rate*/ 48_000, opus::Channels::Mono).unwrap();
        let mut decoded = [1.0; 960];
        assert_eq!(
            decoder
                .decode_float(&silence.payload, &mut decoded, /*fec*/ false)
                .unwrap(),
            960
        );
        assert!(decoded.iter().all(|sample| sample.abs() < 0.0001));
        assert_eq!(
            silence.header.sequence_number,
            before
                .last()
                .unwrap()
                .header
                .sequence_number
                .wrapping_add(1)
        );
        assert!(received.try_recv().is_err());

        // A callback that began before mute may enqueue its old generation only
        // after unmute; a newly started callback may still carry an old device
        // timestamp. Exercise both independently through the actual service loop.
        let pre_unmute = Instant::now() - Duration::from_millis(/*millis*/ 100);
        worker.set_controls(unmuted()).unwrap();
        let generation = worker.buffers.microphone.load(Ordering::Acquire);
        let resumed = Instant::now() + Duration::from_secs(/*secs*/ 1);
        now = resumed;
        enqueue(&worker, /*samples*/ 2_880, old_generation, now);
        assert_eq!(worker.service(&mut local.audio, || now).await.unwrap(), 0);
        enqueue(&worker, /*samples*/ 2_880, generation, pre_unmute);
        assert_eq!(worker.service(&mut local.audio, || now).await.unwrap(), 0);
        enqueue(&worker, /*samples*/ 720, generation, now);
        assert_eq!(worker.service(&mut local.audio, || now).await.unwrap(), 0);

        now += Duration::from_millis(/*millis*/ 15);
        enqueue(&worker, /*samples*/ 2_880, generation, now);
        let after = receive_capture(&mut worker, &mut local, &mut received, now).await;
        let clock_gap = after[0]
            .header
            .timestamp
            .wrapping_sub(before[0].header.timestamp);
        let expected_gap = (resumed.duration_since(start).as_secs_f64() * 48_000.0) as u32;
        // Allow 1 ms for sample rounding without imposing a wall-clock throughput requirement.
        assert!(
            clock_gap.abs_diff(expected_gap) <= 48,
            "received RTP clock gap: {clock_gap}, expected {expected_gap}"
        );
        assert_eq!(
            after[0].header.sequence_number,
            silence.header.sequence_number.wrapping_add(1)
        );
        assert!(received.try_recv().is_err());

        // A mute received during a send must discard the rest of that encoded
        // batch before the next service call, including after a quick unmute.
        worker.set_controls(muted()).unwrap();
        worker.set_controls(unmuted()).unwrap();
        let generation = worker.buffers.microphone.load(Ordering::Acquire);
        now = Instant::now() + Duration::from_secs(/*secs*/ 1);
        enqueue(&worker, /*samples*/ 2_880, generation, now);
        assert_eq!(worker.service(&mut local.audio, || now).await.unwrap(), 1);
        received.recv().await.unwrap();
        assert!(!worker.pending.is_empty());
        let pending = worker.pending.len();
        worker
            .set_controls(AudioControls {
                microphone_muted: false,
                speaker_suppressed: true,
            })
            .unwrap();
        assert_eq!(worker.pending.len(), pending);
        worker.set_controls(muted()).unwrap();
        worker.set_controls(unmuted()).unwrap();
        assert!(worker.pending.is_empty());
        assert_eq!(worker.service(&mut local.audio, || now).await.unwrap(), 0);
        assert!(received.try_recv().is_err());

        // Waiting behind a slow send discards stale audio without ending the session.
        let generation = worker.buffers.microphone.load(Ordering::Acquire);
        now = Instant::now() + Duration::from_secs(/*secs*/ 1);
        enqueue(&worker, /*samples*/ 2_880, generation, now);
        assert_eq!(worker.service(&mut local.audio, || now).await.unwrap(), 1);
        received.recv().await.unwrap();
        let stale = now + Duration::from_secs(/*secs*/ 1);
        assert_eq!(worker.service(&mut local.audio, || stale).await.unwrap(), 0);
        assert!(received.try_recv().is_err());
        now = stale + Duration::from_millis(/*millis*/ 10);
        enqueue(&worker, /*samples*/ 2_880, generation, now);
        let resumed = receive_capture(&mut worker, &mut local, &mut received, now).await;
        assert!(!resumed.is_empty());

        // A saturated callback queue also retires partial processing state, then
        // admits new capture once the worker catches up.
        enqueue(&worker, /*samples*/ 2_880, generation, now);
        worker
            .buffers
            .capture_dropped
            .store(true, Ordering::Release);
        assert_eq!(worker.service(&mut local.audio, || now).await.unwrap(), 0);
        assert!(received.try_recv().is_err());
        now += Duration::from_millis(/*millis*/ 70);
        enqueue(&worker, /*samples*/ 2_880, generation, now);
        assert!(
            !receive_capture(&mut worker, &mut local, &mut received, now)
                .await
                .is_empty()
        );

        // A delayed worker must discard a full render-reference queue before
        // it feeds stale speaker audio back into echo cancellation.
        let mut output = [0.0_f32; BLOCK];
        let mut output_state = super::super::OutputState::default();
        for _ in 0..=worker.buffers.rendered.capacity() {
            super::super::render_output(
                &mut output,
                /*channels*/ 1,
                /*rate*/ 44_100.0,
                now,
                &worker.buffers,
                &mut output_state,
            );
        }
        assert_eq!(
            worker.buffers.rendered.len(),
            worker.buffers.rendered.capacity()
        );
        assert!(worker.buffers.render_dropped.load(Ordering::Acquire));
        assert_eq!(worker.service(&mut local.audio, || now).await.unwrap(), 0);
        assert!(worker.buffers.rendered.is_empty());
        assert!(!worker.buffers.render_dropped.load(Ordering::Acquire));
        assert!(!worker.buffers.failed.load(Ordering::Acquire));

        now += Duration::from_millis(/*millis*/ 70);
        super::super::render_output(
            &mut output,
            /*channels*/ 1,
            /*rate*/ 44_100.0,
            now,
            &worker.buffers,
            &mut output_state,
        );
        enqueue(&worker, /*samples*/ 2_880, generation, now);
        assert!(
            !receive_capture(&mut worker, &mut local, &mut received, now)
                .await
                .is_empty()
        );
        assert!(worker.buffers.rendered.is_empty());
        local.close().await.unwrap();
        remote.close().await.unwrap();
    })
    .await
    .unwrap();
}
