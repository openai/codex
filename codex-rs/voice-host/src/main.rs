//! Same-build helper lifecycle with privately owned runtime, transport and opt-in local devices.

#[cfg_attr(
    not(any(
        target_os = "macos",
        all(target_os = "linux", target_env = "gnu"),
        all(windows, target_env = "msvc")
    )),
    path = "devices_unavailable.rs"
)]
mod devices;
mod runtime;
mod transport;
mod transport_runtime;

use std::io;
use std::io::Write;
use std::sync::mpsc;
use std::time::Duration;

use codex_realtime_webrtc::Message;
use codex_realtime_webrtc::encode_frame;
use codex_realtime_webrtc::read_message;

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
            if run(|executor| {
                executor
                    .block_on(transport::Transport::new())
                    .map_err(io::Error::other)
            })
            .is_err()
            {
                std::process::exit(/*code*/ 1);
            }
        }
        _ => std::process::exit(/*code*/ 2),
    }
}

fn run(
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
                            std::process::exit(/*code*/ 1);
                        }
                    }
                    Ok(None) => break,
                    Err(_) => std::process::exit(/*code*/ 1),
                }
            }
            drop(sender);
            // Independent of the main worker or a blocked stdout write, including after parent death.
            std::thread::sleep(Duration::from_secs(/*secs*/ 2));
            std::process::exit(/*code*/ 1);
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
    let mut output = io::stdout().lock();
    output.write_all(&encode_frame(&Message::Ready {})?)?;
    output.flush()?;
    let mut runtime = None;
    let executor = tokio::runtime::Runtime::new()?;
    let mut transport = None;
    let mut answered = false;
    let mut devices: Option<devices::Devices> = None;
    loop {
        let message = if let Some(devices) = &devices {
            devices.service()?;
            receiver.recv_timeout(DEVICE_SERVICE_INTERVAL)
        } else {
            receiver
                .recv()
                .map_err(|_| mpsc::RecvTimeoutError::Disconnected)
        };
        let reply = match message {
            Ok(Message::OpenDevices {}) if devices.is_none() && runtime.is_some() && answered => {
                devices = Some(devices::Devices::open()?);
                Message::DevicesOpened {}
            }
            Ok(Message::SetAudioControls { controls }) => {
                devices
                    .as_ref()
                    .ok_or_else(|| io::Error::other("audio devices not open"))?
                    .set_controls(controls)?;
                Message::AudioControlsApplied {}
            }
            Ok(Message::StartTransport {}) if transport.is_none() => {
                let peer = start_transport(&executor)?;
                let sdp = executor.block_on(peer.offer()).map_err(io::Error::other)?;
                transport = Some(peer);
                Message::Offer {
                    sdp: sdp.try_into().map_err(io::Error::other)?,
                }
            }
            Ok(Message::ApplyAnswer { sdp }) if !answered => {
                let Some(peer) = transport.as_ref() else {
                    return Err(io::Error::other("voice transport not started"));
                };
                executor
                    .block_on(peer.apply_answer(sdp.into_sdp()))
                    .map_err(io::Error::other)?;
                answered = true;
                Message::TransportReady {}
            }
            Ok(Message::InitializeRuntime {}) => {
                if runtime.is_some() {
                    return Err(io::Error::other("runtime already initialized"));
                }
                runtime = Some(runtime::Runtime::initialize()?);
                Message::RuntimeReady {}
            }
            Ok(Message::Close {}) => {
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
                | Message::OpenDevices {}
                | Message::DevicesOpened {}
                | Message::AudioControlsApplied {}
                | Message::Closed {},
            ) => return Err(io::Error::other("invalid voice control sequence")),
        };
        output.write_all(&encode_frame(&reply)?)?;
        output.flush()?;
    }
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
