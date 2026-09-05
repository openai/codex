//! Verify control-first capture servicing and the real parent-loss watchdog during blocked startup.

use super::*;
use std::process::Stdio;

use pretty_assertions::assert_eq;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;

const STARTUP_BLOCKED: &[u8] = b"voice startup blocked\n";
const CHILD_ENV: &str = "CODEX_VOICE_WATCHDOG_TEST_CHILD";

#[test]
fn queued_privacy_controls_and_close_precede_capture_service() {
    for message in [
        Message::SetAudioControls {
            controls: codex_realtime_webrtc::AudioControls {
                microphone_muted: true,
                speaker_suppressed: true,
            },
        },
        Message::Close {},
    ] {
        let expected = encode_frame(&message).unwrap();
        let (sender, receiver) = mpsc::sync_channel(/*bound*/ 1);
        sender.send(message).unwrap();
        let next = wait_for_control(&receiver, || panic!("capture preceded queued control"))
            .unwrap()
            .unwrap();
        assert_eq!(encode_frame(&next).unwrap(), expected);
        drop(sender);
        assert!(
            wait_for_control(&receiver, || panic!("capture after parent loss"))
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn idle_service_rechecks_controls_before_another_batch() {
    let (sender, receiver) = mpsc::sync_channel(/*bound*/ 1);
    let mut services = 0;
    let next = wait_for_control(&receiver, || {
        services += 1;
        sender.send(Message::Close {}).unwrap();
        Ok(())
    })
    .unwrap()
    .unwrap();
    assert_eq!(next, Message::Close {});
    assert_eq!(services, 1);
}

#[test]
#[ignore = "subprocess fixture for the parent-loss watchdog test"]
fn blocked_startup_fixture() {
    if std::env::var_os(CHILD_ENV).as_deref() != Some(std::ffi::OsStr::new("1")) {
        return;
    }
    run(|_| {
        io::stderr().write_all(STARTUP_BLOCKED)?;
        io::stderr().flush()?;
        loop {
            std::thread::park();
        }
    })
    .unwrap();
}

#[tokio::test]
async fn parent_pipe_loss_terminates_helper_with_blocked_transport_startup() {
    timeout(Duration::from_secs(/*secs*/ 10), async {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "tests::blocked_startup_fixture",
                "--ignored",
                "--nocapture",
            ])
            .env(CHILD_ENV, "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mut input = child.stdin.take().unwrap();
        input
            .write_all(
                &encode_frame(&Message::Hello {
                    protocol: 1,
                    build_commit: BUILD_COMMIT.to_owned(),
                })
                .unwrap(),
            )
            .await
            .unwrap();
        let expected = encode_frame(&Message::Ready {}).unwrap();
        let mut prefix = Vec::new();
        // libtest writes a short harness prefix before the helper's framed output.
        while !prefix.ends_with(&expected) {
            assert!(prefix.len() < 512, "missing helper ready frame");
            prefix.push(child.stdout.as_mut().unwrap().read_u8().await.unwrap());
        }
        input
            .write_all(&encode_frame(&Message::StartTransport {}).unwrap())
            .await
            .unwrap();
        let mut marker = vec![0; STARTUP_BLOCKED.len()];
        child
            .stderr
            .as_mut()
            .unwrap()
            .read_exact(&mut marker)
            .await
            .unwrap();
        assert_eq!(marker, STARTUP_BLOCKED);
        assert!(child.try_wait().unwrap().is_none());
        drop(input);
        let output = child.wait_with_output().await.unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert_eq!((output.stdout, output.stderr), (vec![], vec![]));
    })
    .await
    .unwrap();
}
