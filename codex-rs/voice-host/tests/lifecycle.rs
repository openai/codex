//! Exercise the real helper through installed paths and bounded process I/O.

use std::fs;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use codex_install_context::CodexPackageLayout;
use codex_install_context::InstallContext;
use codex_realtime_webrtc::Message;
use codex_realtime_webrtc::SessionDescription;
use codex_realtime_webrtc::VoiceHost;
use codex_realtime_webrtc::decode_frame;
use codex_realtime_webrtc::encode_frame;
use codex_utils_cargo_bin::cargo_bin;
use futures::future::BoxFuture;
use pretty_assertions::assert_eq;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::process::Child;
use tokio::process::Command;
use tokio::sync::Notify;
use tokio::sync::mpsc;
use tokio::time::timeout;
use webrtc::data_channel::DataChannel;
use webrtc::peer_connection::MediaEngine;
use webrtc::peer_connection::PeerConnection;
use webrtc::peer_connection::PeerConnectionBuilder;
use webrtc::peer_connection::PeerConnectionEventHandler;
use webrtc::peer_connection::RTCIceConnectionState;
use webrtc::peer_connection::RTCIceGatheringState;
use webrtc::peer_connection::RTCPeerConnectionState;
use webrtc::peer_connection::RTCSessionDescription;
use webrtc::peer_connection::SettingEngine;

const DEADLINE: Duration = Duration::from_secs(/*secs*/ 10);

fn spawn() -> Result<Child> {
    Ok(Command::new(cargo_bin("codex-voice-host")?)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?)
}

async fn build_commit() -> Result<String> {
    let output = timeout(
        DEADLINE,
        Command::new(cargo_bin("codex-voice-host")?)
            .arg("--build-commit")
            .kill_on_drop(true)
            .output(),
    )
    .await??;
    assert!(output.status.success());
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

async fn handshake(child: &mut Child) -> Result<()> {
    child
        .stdin
        .as_mut()
        .context("stdin")?
        .write_all(&encode_frame(&Message::Hello {
            protocol: 1,
            build_commit: build_commit().await?,
        })?)
        .await?;
    let expected = encode_frame(&Message::Ready {})?;
    let mut reply = vec![0; expected.len()];
    timeout(
        DEADLINE,
        child
            .stdout
            .as_mut()
            .context("stdout")?
            .read_exact(&mut reply),
    )
    .await??;
    assert_eq!(reply, expected);
    Ok(())
}

#[tokio::test]
async fn closes_after_acknowledgement_and_on_parent_pipe_loss() -> Result<()> {
    for explicit_close in [true, false] {
        let mut child = spawn()?;
        handshake(&mut child).await?;
        if explicit_close {
            child
                .stdin
                .as_mut()
                .unwrap()
                .write_all(&encode_frame(&Message::Close {})?)
                .await?;
        }
        drop(child.stdin.take());
        let output = timeout(DEADLINE, child.wait_with_output()).await??;
        assert!(output.status.success());
        assert_eq!(
            output.stdout,
            if explicit_close {
                encode_frame(&Message::Closed {})?
            } else {
                vec![]
            }
        );
        assert!(output.stderr.is_empty());
    }
    Ok(())
}

#[tokio::test]
async fn exits_after_parent_pipe_loss_following_transport_start_request() -> Result<()> {
    // The binary unit test separately forces startup to block and checks watchdog termination.
    let mut child = spawn()?;
    handshake(&mut child).await?;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&encode_frame(&Message::StartTransport {})?)
        .await?;
    let output = timeout(Duration::from_secs(/*secs*/ 5), child.wait_with_output()).await??;
    assert!(output.stderr.is_empty());
    Ok(())
}

#[tokio::test]
async fn rejects_invalid_input_without_echoing_it() -> Result<()> {
    assert!(decode_frame(b"\0\0\0\x16{\"type\":\"close\",\"x\":0}").is_err());
    let mut invalid_json = 22_u32.to_be_bytes().to_vec();
    invalid_json.extend_from_slice(b"sensitive-invalid-json");
    for frame in [
        u32::MAX.to_be_bytes().to_vec(),
        vec![0, 0],
        invalid_json,
        encode_frame(&Message::Hello {
            protocol: 1,
            build_commit: "wrong-build".into(),
        })?,
        encode_frame(&Message::Hello {
            protocol: 99,
            build_commit: build_commit().await?,
        })?,
        encode_frame(&Message::Close {})?,
    ] {
        let mut child = spawn()?;
        child.stdin.take().unwrap().write_all(&frame).await?;
        let output = timeout(DEADLINE, child.wait_with_output()).await??;
        assert!(!output.status.success());
        assert_eq!((output.stdout, output.stderr), (vec![], vec![]));
    }
    Ok(())
}
fn install_helper(root: &Path) -> Result<CodexPackageLayout> {
    let bin = root.join("bin");
    let helper_dir = root.join("codex-resources/voice/bin");
    fs::create_dir_all(&bin)?;
    fs::create_dir_all(&helper_dir)?;
    fs::write(root.join("codex-package.json"), "{}")?;
    let app = bin.join(if cfg!(windows) { "codex.exe" } else { "codex" });
    fs::write(&app, [])?;
    let source = cargo_bin("codex-voice-host")?;
    let helper = helper_dir.join(source.file_name().context("helper binary file name")?);
    fs::copy(&source, &helper)?;
    InstallContext::from_exe(
        /*is_macos*/ cfg!(target_os = "macos"),
        Some(&app),
        /*method_override*/ None,
    )
    .package_layout
    .context("package layout")
}

#[tokio::test]
async fn installed_client_rejects_mixed_builds_and_missing_helper() -> Result<()> {
    let directory = tempfile::Builder::new()
        .prefix("voice package ")
        .tempdir()?;
    let package = install_helper(directory.path())?;
    let bin = directory.path().join("bin");
    let source = cargo_bin("codex-voice-host")?;
    let helper = directory
        .path()
        .join("codex-resources/voice/bin")
        .join(source.file_name().unwrap());
    VoiceHost::connect(&package, &build_commit().await?)
        .await?
        .close()
        .await?;
    assert!(VoiceHost::connect(&package, "wrong-build").await.is_err());
    // Helper-only installations still handshake, but cannot claim native readiness.
    let host = VoiceHost::connect(&package, &build_commit().await?).await?;
    assert!(host.initialize_runtime().await.is_err());
    // The same executable elsewhere in the package must not become a fallback.
    fs::rename(&helper, bin.join(source.file_name().unwrap()))?;
    assert!(
        VoiceHost::connect(&package, &build_commit().await?)
            .await
            .is_err()
    );
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&source, &helper)?;
        assert!(
            VoiceHost::connect(&package, &build_commit().await?)
                .await
                .is_err()
        );
    }
    Ok(())
}

enum RemoteState {
    Ice(RTCIceConnectionState),
    Peer(RTCPeerConnectionState),
    Channel,
}

struct RemoteEvents {
    channels: mpsc::Sender<Arc<dyn DataChannel>>,
    gathered: Arc<Notify>,
    diagnostics: mpsc::Sender<(Duration, RemoteState)>,
    started: Instant,
}

impl RemoteEvents {
    fn record(&self, state: RemoteState) {
        // Keep only the first 32 events; diagnostics must never wait on the connection driver.
        let _ = self.diagnostics.try_send((self.started.elapsed(), state));
    }
}

impl PeerConnectionEventHandler for RemoteEvents {
    fn on_ice_connection_state_change<'a, 'async_trait>(
        &'a self,
        state: RTCIceConnectionState,
    ) -> BoxFuture<'async_trait, ()>
    where
        'a: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move { self.record(RemoteState::Ice(state)) })
    }

    fn on_connection_state_change<'a, 'async_trait>(
        &'a self,
        state: RTCPeerConnectionState,
    ) -> BoxFuture<'async_trait, ()>
    where
        'a: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move { self.record(RemoteState::Peer(state)) })
    }

    fn on_data_channel<'a, 'async_trait>(
        &'a self,
        channel: Arc<dyn DataChannel>,
    ) -> BoxFuture<'async_trait, ()>
    where
        'a: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            self.record(RemoteState::Channel);
            assert!(
                self.channels.try_send(channel).is_ok(),
                "receiver must accept the event channel"
            );
        })
    }

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
                self.gathered.notify_one();
            }
        })
    }
}

#[tokio::test]
async fn installed_client_negotiates_and_closes_over_udp_and_tcp() -> Result<()> {
    let directory = tempfile::Builder::new()
        .prefix("voice package ")
        .tempdir()?;
    let package = install_helper(directory.path())?;
    let commit = build_commit().await?;
    for tcp in [false, true] {
        let started = Instant::now();
        let (diagnostics, mut states) = mpsc::channel(/*buffer*/ 32);
        let negotiation = timeout(Duration::from_secs(/*secs*/ 30), async {
            let (sender, mut channels) = mpsc::channel(/*buffer*/ 1);
            let gathered = Arc::new(Notify::new());
            let mut settings = SettingEngine::default();
            settings.set_lite(/*lite*/ true);
            let mut media = MediaEngine::default();
            media.register_default_codecs()?;
            let builder = PeerConnectionBuilder::new()
                .with_media_engine(media)
                .with_setting_engine(settings)
                .with_handler(Arc::new(RemoteEvents {
                    channels: sender,
                    gathered: gathered.clone(),
                    diagnostics,
                    started,
                }));
            let remote = if tcp {
                builder.with_tcp_addrs(vec!["0.0.0.0:0"])
            } else {
                builder.with_udp_addrs(vec!["0.0.0.0:0"])
            }
            .build()
            .await?;
            let result: Result<()> = async {
                let host = VoiceHost::connect(&package, &commit).await?;
                let (host, offer) = host.start_transport().await?;
                remote
                    .set_remote_description(RTCSessionDescription::offer(offer.into_sdp())?)
                    .await?;
                let answer = remote.create_answer(/*options*/ None).await?;
                remote.set_local_description(answer).await?;
                gathered.notified().await;
                let answer = remote.local_description().await.context("local answer")?;
                let host = host
                    .apply_answer(
                        SessionDescription::try_from(answer.sdp).map_err(anyhow::Error::msg)?,
                    )
                    .await
                    .with_context(|| format!("tcp={tcp}: apply voice answer"))?;
                let channel = channels.recv().await.context("event channel")?;
                assert_eq!(
                    (channel.label().await?, channel.ordered().await?),
                    ("oai-events".into(), true)
                );
                // This checks the Closed acknowledgement and successful child exit as well.
                host.close().await
            }
            .await;
            remote.close().await?;
            result
        });
        let mut heartbeat = tokio::time::interval(Duration::from_millis(/*millis*/ 100));
        heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut ticks = 0_u32;
        let mut last_poll = started;
        let mut max_gap = Duration::ZERO;
        let mut max_gap_at = Duration::ZERO;
        let result = {
            tokio::pin!(negotiation);
            loop {
                let outcome = tokio::select! {
                    result = &mut negotiation => Some(result),
                    _ = heartbeat.tick() => {
                        ticks += 1;
                        None
                    }
                };
                // Include the final polling gap even when negotiation or its timeout wins.
                let now = Instant::now();
                let gap = now.duration_since(last_poll);
                if gap > max_gap {
                    max_gap = gap;
                    max_gap_at = now.duration_since(started);
                }
                last_poll = now;
                if let Some(result) = outcome {
                    break result;
                }
            }
        };
        result
            .with_context(|| format!("tcp={tcp}: helper negotiation timed out"))
            .and_then(std::convert::identity)
            .with_context(|| {
                let events: Vec<_> = std::iter::from_fn(|| states.try_recv().ok())
                    .map(|(elapsed, state)| match state {
                        RemoteState::Ice(state) => format!("{elapsed:?}: ICE {state}"),
                        RemoteState::Peer(state) => format!("{elapsed:?}: peer {state}"),
                        RemoteState::Channel => format!("{elapsed:?}: channel arrived"),
                    })
                    .collect();
                format!(
                    "tcp={tcp}: remote connection events (first 32): {events:?}; \
                     test runtime heartbeat: ticks={ticks}, max polling gap={max_gap:?} \
                     observed at {max_gap_at:?}"
                )
            })?;
    }
    Ok(())
}

// Linux permits raw-byte filenames; macOS filesystems reject this name themselves.
#[cfg(target_os = "linux")]
#[tokio::test]
async fn installed_client_accepts_non_utf8_package_path() -> Result<()> {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let directory = tempfile::tempdir()?;
    let root = directory
        .path()
        .join(OsString::from_vec(b"voice-\xff".to_vec()));
    let bin = root.join("bin");
    let helper_dir = root.join("codex-resources/voice/bin");
    fs::create_dir_all(&bin)?;
    fs::create_dir_all(&helper_dir)?;
    fs::write(root.join("codex-package.json"), "{}")?;
    let app = bin.join("codex");
    fs::write(&app, [])?;
    let source = cargo_bin("codex-voice-host")?;
    fs::copy(&source, helper_dir.join(source.file_name().unwrap()))?;
    let package = InstallContext::from_exe(
        /*is_macos*/ cfg!(target_os = "macos"),
        Some(&app),
        /*method_override*/ None,
    )
    .package_layout
    .context("package layout")?;
    VoiceHost::connect(&package, &build_commit().await?)
        .await?
        .close()
        .await
}
