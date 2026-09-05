use super::*;
use pretty_assertions::assert_eq;

fn handles() -> (
    RealtimeWebrtcSessionHandle,
    mpsc::Receiver<Command>,
    AbortRegistration,
) {
    let (sender, receiver) = mpsc::channel(/*buffer*/ 8);
    let (stop, stopped) = AbortHandle::new_pair();
    let handle = RealtimeWebrtcSessionHandle(Arc::new(Owner {
        sender,
        stop,
        state: Arc::new(State::default()),
        controls: Arc::new(Mutex::new(AudioControls {
            microphone_muted: false,
            speaker_suppressed: false,
        })),
    }));
    (handle, receiver, stopped)
}

#[test]
fn controls_preserve_order_and_independent_settings_and_overflow_closes() {
    let (handle, mut commands, _) = handles();
    handle.set_microphone_muted(/*muted*/ true).unwrap();
    handle.set_speaker_suppressed(/*suppressed*/ true);
    handle.set_microphone_muted(/*muted*/ false).unwrap();
    for expected in [(true, false), (true, true), (false, true)] {
        let Command::Controls(controls) = commands.try_recv().unwrap() else {
            panic!("wrong command")
        };
        assert_eq!(
            controls,
            AudioControls {
                microphone_muted: expected.0,
                speaker_suppressed: expected.1
            }
        );
    }
    for _ in 0..8 {
        handle.set_microphone_muted(/*muted*/ true).unwrap();
    }
    assert!(handle.set_microphone_muted(/*muted*/ false).is_err());
    assert!(handle.0.stop.is_aborted());
    assert!(handle.take_error().is_some());
}

#[tokio::test]
async fn only_final_owner_drop_cancels_and_explicit_close_cancels_all_clones() {
    let (handle, _commands, stopped) = handles();
    let clone = handle.clone();
    drop(handle);
    assert!(!clone.0.stop.is_aborted());
    drop(clone);
    assert!(
        Abortable::new(std::future::pending::<()>(), stopped)
            .await
            .is_err()
    );
    let (handle, _commands, stopped) = handles();
    let clone = handle.clone();
    handle.close();
    assert!(clone.0.stop.is_aborted());
    assert!(
        Abortable::new(std::future::pending::<()>(), stopped)
            .await
            .is_err()
    );
}

#[test]
fn offer_debug_redacts_credentials_and_peaks_are_consumed() {
    let (handle, _commands, _) = handles();
    handle.0.state.microphone.store(123, Ordering::Release);
    handle.0.state.speaker.store(456, Ordering::Release);
    assert_eq!(
        (handle.take_microphone_peak(), handle.take_speaker_peak()),
        (123, 456)
    );
    assert_eq!(
        (handle.take_microphone_peak(), handle.take_speaker_peak()),
        (0, 0)
    );
    let started = StartedRealtimeWebrtcSession {
        offer_sdp: "synthetic-secret".into(),
        handle,
    };
    assert!(!format!("{started:?}").contains("synthetic-secret"));
}

#[test]
fn controls_queued_during_negotiation_precede_first_device_enable() {
    let (handle, mut commands, _) = handles();
    // The actor has consumed Answer and is waiting for negotiation/device startup.
    handle.set_microphone_muted(/*muted*/ true).unwrap();
    handle.set_speaker_suppressed(/*suppressed*/ true);
    let controls = startup_controls(&mut commands, &handle.0.controls, Ok).unwrap();
    assert_eq!(
        controls,
        AudioControls {
            microphone_muted: true,
            speaker_suppressed: true
        }
    );
    assert!(commands.is_empty());
    let (complete, _) = blocking::sync_channel(/*bound*/ 1);
    handle
        .send(Command::Answer(
            "synthetic-answer".to_owned().try_into().unwrap(),
            complete,
        ))
        .unwrap();
    assert!(startup_controls(&mut commands, &handle.0.controls, Ok).is_err());
}

#[test]
fn package_availability_requires_helper_and_native_runtime() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("voice-package-{}-{unique}", std::process::id()));
    let voice = root.join("codex-resources/voice");
    std::fs::create_dir_all(voice.join("bin")).unwrap();
    std::fs::create_dir_all(voice.join("lib")).unwrap();
    assert!(!package_has_runtime(&root));
    let helper = voice.join(if cfg!(windows) {
        "bin/codex-voice-host.exe"
    } else {
        "bin/codex-voice-host"
    });
    std::fs::write(&helper, b"synthetic helper").unwrap();
    assert!(!package_has_runtime(&root));
    let runtime = voice.join(if cfg!(target_os = "macos") {
        "lib/libgstreamer-1.0.0.dylib"
    } else if cfg!(windows) {
        "bin/gstreamer-1.0-0.dll"
    } else {
        "lib/libgstreamer-1.0.so.0"
    });
    std::fs::write(&runtime, b"synthetic runtime").unwrap();
    assert!(package_has_runtime(&root));
    std::fs::remove_file(helper).unwrap();
    assert!(!package_has_runtime(&root));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn startup_dispatch_is_ordered_but_acknowledgement_does_not_block_setters() {
    let (handle, mut commands, _) = handles();
    let applied = startup_controls(&mut commands, &handle.0.controls, |initial| {
        // This is the request enqueue boundary. A setter cannot commit a newer
        // snapshot until the initial request is already ordered before it.
        assert!(matches!(
            handle.0.controls.try_lock(),
            Err(std::sync::TryLockError::WouldBlock)
        ));
        assert_eq!(
            initial,
            AudioControls {
                microphone_muted: false,
                speaker_suppressed: false,
            }
        );
        Ok(std::future::pending::<()>())
    })
    .unwrap();
    // The acknowledgement future has not completed or even been polled.
    handle.set_microphone_muted(/*muted*/ true).unwrap();
    let Command::Controls(next) = commands.try_recv().unwrap() else {
        panic!("expected queued mute")
    };
    assert_eq!(
        next,
        AudioControls {
            microphone_muted: true,
            speaker_suppressed: false,
        }
    );
    drop(applied);
}
