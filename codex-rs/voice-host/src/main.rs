//! Same-build helper lifecycle with privately owned runtime, transport and opt-in local devices.
//! Queued privacy controls take priority over starting another capture batch.

// Non-native helper targets negotiate transport but have no device backend to send audio.
#[cfg_attr(
    not(any(
        target_os = "macos",
        all(target_os = "linux", target_env = "gnu"),
        all(windows, target_env = "msvc")
    )),
    allow(dead_code)
)]
mod audio_track;
#[cfg_attr(
    not(any(
        target_os = "macos",
        all(target_os = "linux", target_env = "gnu"),
        all(windows, target_env = "msvc")
    )),
    path = "devices_unavailable.rs"
)]
mod devices;
mod incoming;
mod runtime;
mod service_failure;
mod transport;
mod transport_runtime;

use std::cell::Cell;
use std::io;
use std::io::Write;
use std::sync::mpsc;
use std::time::Duration;

use codex_realtime_webrtc::HelperExitStage;
use codex_realtime_webrtc::Message;
use codex_realtime_webrtc::encode_frame;
use codex_realtime_webrtc::read_message;
use service_failure::ServiceFailure;

const DEVICE_SERVICE_INTERVAL: Duration = Duration::from_millis(/*millis*/ 5);

const BUILD_COMMIT: &str = match option_env!("STABLE_GIT_COMMIT") {
    Some(commit) => commit,
    None => "dev",
};

fn main() {
    codex_process_hardening::pre_main_hardening();
    let mut args = std::env::args_os().skip(/*n*/ 1);
    match (args.next(), args.next()) {
        (Some(arg), None) if arg == "--build-commit" => println!("{BUILD_COMMIT}"),
        (None, None) => {
            let phase = Cell::new(HelperExitStage::ControlSequence);
            if run(&phase, |executor| {
                phase.set(HelperExitStage::Transport);
                executor
                    .block_on(transport::Transport::new())
                    .map_err(io::Error::other)
            })
            .is_err()
            {
                std::process::exit(phase.get().code());
            }
        }
        _ => std::process::exit(/*code*/ 2),
    }
}

fn run(
    phase: &Cell<HelperExitStage>,
    start_transport: impl Fn(&tokio::runtime::Runtime) -> io::Result<transport::Transport>,
) -> io::Result<()> {
    let (sender, receiver) = mpsc::sync_channel(/*bound*/ 1);
    std::thread::Builder::new()
        .name("voice-control".into())
        .spawn(move || {
            let mut input = io::stdin().lock();
            loop {
                match read_message(&mut input) {
                    Ok(Some(message)) => {
                        if sender.try_send(message).is_err() {
                            std::process::exit(HelperExitStage::ControlQueue.code());
                        }
                    }
                    Ok(None) => break,
                    Err(_) => std::process::exit(HelperExitStage::ControlRead.code()),
                }
            }
            drop(sender);
            // Independent of the main worker or a blocked stdout write, including after parent death.
            std::thread::sleep(Duration::from_secs(/*secs*/ 2));
            std::process::exit(HelperExitStage::ParentGone.code());
        })?;
    let Ok(hello) = receiver.recv() else {
        return Ok(());
    };
    if hello
        != (Message::Hello {
            protocol: 1,
            build_commit: BUILD_COMMIT.to_owned(),
        })
    {
        return Err(io::Error::other("incompatible voice helper"));
    }
    phase.set(HelperExitStage::Reply);
    let mut output = io::stdout().lock();
    output.write_all(&encode_frame(&Message::Ready {})?)?;
    output.flush()?;
    let mut runtime = None;
    let executor = tokio::runtime::Runtime::new()?;
    let mut transport: Option<transport::Transport> = None;
    let mut answered = false;
    let mut devices: Option<devices::Devices> = None;
    loop {
        phase.set(HelperExitStage::ControlSequence);
        let message = if let Some(devices) = &mut devices {
            let peer = transport
                .as_mut()
                .ok_or_else(|| io::Error::other("voice peer not started"))?;
            wait_for_control(&receiver, || {
                // Bound one pass to the ingress queue capacity so new media
                // cannot indefinitely postpone a waiting privacy control.
                for _ in 0..64 {
                    phase.set(HelperExitStage::AudioIngress);
                    let Some(packet) = peer.incoming.take().map_err(io::Error::other)? else {
                        break;
                    };
                    phase.set(HelperExitStage::Playout);
                    devices.receive(packet)?;
                }
                phase.set(HelperExitStage::AudioService);
                if let Err(error) = executor.block_on(devices.service(&mut peer.audio)) {
                    if let Some(failure) = error
                        .get_ref()
                        .and_then(|error| error.downcast_ref::<ServiceFailure>())
                    {
                        phase.set(match failure {
                            ServiceFailure::Playout => HelperExitStage::Playout,
                            ServiceFailure::Render => HelperExitStage::Render,
                            ServiceFailure::Capture => HelperExitStage::Capture,
                            ServiceFailure::Device => HelperExitStage::Device,
                            ServiceFailure::Send => HelperExitStage::Send,
                        });
                    }
                    return Err(error);
                }
                Ok(())
            })?
            .ok_or(mpsc::RecvTimeoutError::Disconnected)
        } else {
            receiver
                .recv()
                .map_err(|_| mpsc::RecvTimeoutError::Disconnected)
        };
        let reply = match message {
            Ok(Message::InspectAudio {}) => {
                phase.set(HelperExitStage::InspectAudio);
                if answered && transport.as_ref().is_none_or(|peer| !*peer.ready.borrow()) {
                    return Err(io::Error::other("voice connection closed"));
                }
                Message::AudioState {
                    state: devices
                        .as_ref()
                        .map(devices::Devices::take_state)
                        .transpose()
                        .inspect_err(|_| phase.set(HelperExitStage::Device))?
                        .unwrap_or_default(),
                }
            }
            Ok(Message::OpenDevices {}) if devices.is_none() && runtime.is_some() && answered => {
                phase.set(HelperExitStage::OpenDevices);
                devices = Some(devices::Devices::open()?);
                Message::DevicesOpened {}
            }
            Ok(Message::SetAudioControls { controls }) => {
                phase.set(HelperExitStage::AudioControls);
                let peer = transport
                    .as_ref()
                    .ok_or_else(|| io::Error::other("voice peer not started"))?;
                if controls.speaker_suppressed {
                    peer.incoming
                        .set_suppressed(controls.speaker_suppressed)
                        .map_err(io::Error::other)?;
                }
                devices
                    .as_mut()
                    .ok_or_else(|| io::Error::other("audio devices not open"))?
                    .set_controls(controls)?;
                if !controls.speaker_suppressed {
                    peer.incoming
                        .set_suppressed(controls.speaker_suppressed)
                        .map_err(io::Error::other)?;
                }
                Message::AudioControlsApplied {}
            }
            Ok(Message::StartTransport {}) if transport.is_none() => {
                phase.set(HelperExitStage::Transport);
                let peer = start_transport(&executor)?;
                let sdp = executor.block_on(peer.offer()).map_err(io::Error::other)?;
                transport = Some(peer);
                Message::Offer {
                    sdp: sdp.try_into().map_err(io::Error::other)?,
                }
            }
            Ok(Message::ApplyAnswer { sdp }) if !answered => {
                phase.set(HelperExitStage::Transport);
                let Some(peer) = transport.as_ref() else {
                    return Err(io::Error::other("voice transport not started"));
                };
                let outcome = executor
                    .block_on(peer.apply_answer(sdp.into_sdp()))
                    .map_err(io::Error::other)?;
                if outcome == transport::AnswerOutcome::TimedOut {
                    // The client reaps this helper before considering a fresh negotiation.
                    output.write_all(&encode_frame(&Message::TransportTimedOut {})?)?;
                    output.flush()?;
                    return Ok(());
                }
                answered = true;
                Message::TransportReady {}
            }
            Ok(Message::InitializeRuntime {}) => {
                phase.set(HelperExitStage::Runtime);
                if runtime.is_some() {
                    return Err(io::Error::other("runtime already initialized"));
                }
                runtime = Some(runtime::Runtime::initialize()?);
                Message::RuntimeReady {}
            }
            Ok(Message::Close {}) => {
                phase.set(HelperExitStage::Shutdown);
                let _ = devices.take();
                if let Some(mut peer) = transport.take() {
                    executor.block_on(peer.close()).map_err(io::Error::other)?;
                }
                output.write_all(&encode_frame(&Message::Closed {})?)?;
                return output.flush();
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
            Ok(
                Message::Hello { .. }
                | Message::Ready {}
                | Message::RuntimeReady {}
                | Message::StartTransport {}
                | Message::ApplyAnswer { .. }
                | Message::Offer { .. }
                | Message::TransportReady {}
                | Message::TransportTimedOut {}
                | Message::OpenDevices {}
                | Message::DevicesOpened {}
                | Message::AudioControlsApplied {}
                | Message::AudioState { .. }
                | Message::Closed {},
            ) => return Err(io::Error::other("invalid voice control sequence")),
        };
        phase.set(HelperExitStage::Reply);
        output.write_all(&encode_frame(&reply)?)?;
        output.flush()?;
    }
}

// Pending privacy controls and shutdown always precede the next capture batch.
// A command arriving after service begins cannot retract an already in-flight packet.
fn wait_for_control(
    receiver: &mpsc::Receiver<Message>,
    mut service: impl FnMut() -> io::Result<()>,
) -> io::Result<Option<Message>> {
    loop {
        match receiver.recv_timeout(DEVICE_SERVICE_INTERVAL) {
            Ok(message) => return Ok(Some(message)),
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(None),
            Err(mpsc::RecvTimeoutError::Timeout) => service()?,
        }
    }
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
