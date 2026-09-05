//! Exercises the public session actor against a separate framed-IPC helper, without native audio.

mod common;

use anyhow::Result;
use codex_realtime_webrtc::AudioControls;
use codex_realtime_webrtc::RealtimeWebrtcSession;
use futures::future::AbortHandle;
use pretty_assertions::assert_eq;
use std::fs;
use std::thread;

#[test]
fn startup_controls_meters_and_helper_loss() -> Result<()> {
    let Some(root) = common::package("startup_controls_meters_and_helper_loss")? else {
        return Ok(());
    };
    #[cfg(target_os = "linux")]
    {
        assert!(!RealtimeWebrtcSession::is_supported());
        let libraries = root.join("codex-resources/voice/lib");
        fs::create_dir_all(&libraries)?;
        fs::write(libraries.join("libgstreamer-1.0.so.0"), b"fixture runtime")?;
        assert!(RealtimeWebrtcSession::is_supported());
    }
    let (_abort, registration) = AbortHandle::new_pair();
    let started = RealtimeWebrtcSession::start(registration)?;
    assert_eq!(started.offer_sdp, "synthetic-offer");
    let handle = started.handle;
    handle.set_microphone_muted(/*muted*/ true)?;
    let answer_handle = handle.clone();
    let answer = thread::spawn(move || answer_handle.apply_answer_sdp("synthetic-answer".into()));
    common::wait_for(|| root.join("answer").exists())?;
    handle.set_speaker_suppressed(/*suppressed*/ true);
    fs::write(root.join("release"), [])?;
    answer.join().expect("answer thread")?;
    let initial: Vec<AudioControls> = serde_json::from_slice(&fs::read(root.join("controls"))?)?;
    assert_eq!(
        initial,
        vec![AudioControls {
            microphone_muted: true,
            speaker_suppressed: true
        }]
    );
    handle.set_microphone_muted(/*muted*/ false)?;
    handle.set_speaker_suppressed(/*suppressed*/ false);
    let expected = serde_json::to_vec(&vec![
        AudioControls {
            microphone_muted: true,
            speaker_suppressed: true,
        },
        AudioControls {
            microphone_muted: false,
            speaker_suppressed: true,
        },
        AudioControls {
            microphone_muted: false,
            speaker_suppressed: false,
        },
    ])?;
    common::wait_for(|| fs::read(root.join("controls")).is_ok_and(|data| data == expected))?;
    common::wait_for(|| handle.take_microphone_peak() == 123)?;
    common::wait_for(|| handle.take_speaker_peak() == 456)?;
    fs::write(root.join("exit"), [])?;
    let mut error = None;
    common::wait_for(|| {
        error = handle.take_error();
        error.is_some()
    })?;
    assert_eq!(error.as_deref(), Some("Voice helper stopped unexpectedly."));
    assert_eq!(handle.take_error(), None);
    Ok(())
}

#[test]
fn external_cancellation_interrupts_startup() -> Result<()> {
    let Some(root) = common::package("external_cancellation_interrupts_startup")? else {
        return Ok(());
    };
    fs::write(root.join("hold-initialization"), [])?;
    let (abort, registration) = AbortHandle::new_pair();
    let (result, received) = std::sync::mpsc::sync_channel(/*bound*/ 1);
    let startup = thread::spawn(move || {
        let _ = result.send(RealtimeWebrtcSession::start(registration));
    });
    common::wait_for(|| root.join("initializing").exists())?;
    abort.abort();
    assert!(
        received
            .recv_timeout(std::time::Duration::from_secs(/*secs*/ 2))?
            .is_err()
    );
    startup.join().expect("startup thread");
    #[cfg(unix)]
    common::wait_for_helper_reaped(&root)?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn last_owner_drop_reaps_helper() -> Result<()> {
    let Some(root) = common::package("last_owner_drop_reaps_helper")? else {
        return Ok(());
    };
    let (_abort, registration) = AbortHandle::new_pair();
    let started = RealtimeWebrtcSession::start(registration)?;
    drop(started);
    common::wait_for_helper_reaped(&root)
}
