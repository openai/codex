//! Synchronous UI handles for one helper-owning actor. Closing any handle or dropping the last
//! owner cancels pending I/O; bounded, ordered commands preserve every mute transition.
//! A shared process-lifetime runtime keeps child reapers alive after an actor stops.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicU16;
use std::sync::atomic::Ordering;
use std::sync::mpsc as blocking;
use std::time::Duration;

use anyhow::Result;
use futures::future::AbortHandle;
use futures::future::AbortRegistration;
use futures::future::Abortable;
use tokio::sync::mpsc;

use crate::AudioControls;
use crate::SessionDescription;
use crate::VoiceHost;

// Handshake (5s), runtime initialization (30s), offer gathering (20s), plus overhead.
const STARTUP_WAIT: Duration = Duration::from_secs(/*secs*/ 60);
// An in-flight request (5s), answer (20s), devices (5s), controls (5s), plus overhead.
const ANSWER_WAIT: Duration = Duration::from_secs(/*secs*/ 40);
static RUNTIME: Mutex<Option<Arc<tokio::runtime::Runtime>>> = Mutex::new(None);

enum Command {
    Answer(SessionDescription, blocking::SyncSender<()>),
    Controls(AudioControls),
}

#[derive(Default)]
struct State {
    microphone: AtomicU16,
    speaker: AtomicU16,
    error: Mutex<Option<String>>,
}

struct Owner {
    sender: mpsc::Sender<Command>,
    stop: AbortHandle,
    state: Arc<State>,
    controls: Arc<Mutex<AudioControls>>,
}

impl Drop for Owner {
    fn drop(&mut self) {
        self.stop.abort();
    }
}

/// A cloned handle shares ownership; native media remains entirely inside the child process.
#[derive(Clone)]
pub struct RealtimeWebrtcSessionHandle(Arc<Owner>);

impl std::fmt::Debug for RealtimeWebrtcSessionHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RealtimeWebrtcSessionHandle")
    }
}

/// An offer is not proof of connectivity or working devices.
pub struct StartedRealtimeWebrtcSession {
    pub offer_sdp: String,
    pub handle: RealtimeWebrtcSessionHandle,
}

impl std::fmt::Debug for StartedRealtimeWebrtcSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StartedRealtimeWebrtcSession { offer_sdp: [REDACTED] }")
    }
}

pub struct RealtimeWebrtcSession;

impl RealtimeWebrtcSession {
    /// Check package availability without loading native code or touching any audio device.
    /// Linux packages may pair a musl application with a GNU helper; startup checks host loading.
    pub fn is_supported() -> bool {
        cfg!(any(
            target_os = "macos",
            target_os = "linux",
            all(windows, target_env = "msvc")
        )) && codex_install_context::InstallContext::current()
            .package_layout
            .as_ref()
            .is_some_and(|package| package_has_runtime(package.package_dir.as_path()))
    }

    /// Called off the UI thread. Cancellation also owns startup before a handle is returned.
    pub fn start(abort: AbortRegistration) -> Result<StartedRealtimeWebrtcSession> {
        let package = codex_install_context::InstallContext::current()
            .package_layout
            .clone()
            .ok_or_else(|| anyhow::anyhow!("voice package unavailable"))?;
        let build_commit = codex_build_info::BuildInfo::get().build_commit().to_owned();
        let (sender, receiver) = mpsc::channel(/*buffer*/ 8);
        let (offer, result) = blocking::sync_channel(/*bound*/ 1);
        let (stop, stopped) = AbortHandle::new_pair();
        let state = Arc::new(State::default());
        let controls = Arc::new(Mutex::new(AudioControls {
            microphone_muted: false,
            speaker_suppressed: false,
        }));
        let handle = RealtimeWebrtcSessionHandle(Arc::new(Owner {
            sender,
            stop,
            state: state.clone(),
            controls: controls.clone(),
        }));
        let runtime = {
            let mut runtime = RUNTIME
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(runtime) = runtime.as_ref() {
                runtime.clone()
            } else {
                let shared = Arc::new(
                    tokio::runtime::Builder::new_multi_thread()
                        .worker_threads(/*val*/ 1)
                        .enable_all()
                        .build()?,
                );
                *runtime = Some(shared.clone());
                shared
            }
        };
        std::thread::Builder::new()
            .name("voice-session".into())
            .spawn(move || {
                let task = async {
                    let host = VoiceHost::connect(&package, &build_commit)
                        .await?
                        .initialize_runtime()
                        .await?;
                    let (host, sdp) = host.start_transport().await?;
                    offer
                        .send(sdp.into_sdp())
                        .map_err(|_| anyhow::anyhow!("voice startup cancelled"))?;
                    run(host, receiver, &state, &controls).await
                };
                let result = runtime.block_on(Abortable::new(Abortable::new(task, abort), stopped));
                if !matches!(result, Err(_) | Ok(Err(_)) | Ok(Ok(Ok(())))) {
                    *state
                        .error
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner) =
                        Some("Voice helper stopped unexpectedly.".into());
                }
            })?;
        let offer_sdp = result
            .recv_timeout(STARTUP_WAIT)
            .map_err(|_| anyhow::anyhow!("voice startup failed"))?;
        Ok(StartedRealtimeWebrtcSession { offer_sdp, handle })
    }
}

// Availability is not proof of runtime integrity, device access, or session connectivity.
fn package_has_runtime(package: &std::path::Path) -> bool {
    let voice = package.join("codex-resources/voice");
    let (helper, runtime) = if cfg!(target_os = "macos") {
        ("bin/codex-voice-host", "lib/libgstreamer-1.0.0.dylib")
    } else if cfg!(windows) {
        ("bin/codex-voice-host.exe", "bin/gstreamer-1.0-0.dll")
    } else {
        ("bin/codex-voice-host", "lib/libgstreamer-1.0.so.0")
    };
    voice.join(helper).is_file() && voice.join(runtime).is_file()
}

impl RealtimeWebrtcSessionHandle {
    pub fn close(&self) {
        self.0.stop.abort();
    }

    /// Queue an ordered privacy transition; saturation terminates the session instead of dropping it.
    pub fn set_microphone_muted(&self, muted: bool) -> Result<()> {
        let mut controls = self
            .0
            .controls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        controls.microphone_muted = muted;
        self.send(Command::Controls(*controls))
    }

    pub fn set_speaker_suppressed(&self, suppressed: bool) {
        let mut controls = self
            .0
            .controls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        controls.speaker_suppressed = suppressed;
        let _ = self.send(Command::Controls(*controls));
    }

    /// Called off the UI thread; return only after negotiation, device startup and restored controls.
    pub fn apply_answer_sdp(&self, answer: String) -> Result<()> {
        let sdp = SessionDescription::try_from(answer).map_err(anyhow::Error::msg)?;
        let (complete, result) = blocking::sync_channel(/*bound*/ 1);
        self.send(Command::Answer(sdp, complete))?;
        if result.recv_timeout(ANSWER_WAIT).is_err() {
            self.close();
            anyhow::bail!("voice connection failed");
        }
        Ok(())
    }

    pub fn take_error(&self) -> Option<String> {
        self.0
            .state
            .error
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    }

    pub fn take_microphone_peak(&self) -> u16 {
        self.0.state.microphone.swap(/*val*/ 0, Ordering::AcqRel)
    }

    pub fn take_speaker_peak(&self) -> u16 {
        self.0.state.speaker.swap(/*val*/ 0, Ordering::AcqRel)
    }

    fn send(&self, command: Command) -> Result<()> {
        if self.0.sender.try_send(command).is_err() {
            *self
                .0
                .state
                .error
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) =
                Some("Voice control channel unavailable.".into());
            self.close();
            anyhow::bail!("voice control channel unavailable");
        }
        Ok(())
    }
}

async fn run(
    mut host: VoiceHost,
    mut commands: mpsc::Receiver<Command>,
    state: &State,
    controls: &Mutex<AudioControls>,
) -> Result<()> {
    let mut connected = false;
    let mut poll = tokio::time::interval(Duration::from_millis(/*millis*/ 50));
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            biased;
            command = commands.recv() => match command {
                Some(Command::Answer(sdp, complete)) if !connected => {
                    host = host.apply_answer(sdp).await?.open_devices().await?;
                    let applied = startup_controls(&mut commands, controls, |initial| {
                        host.begin_audio_controls(initial)
                    })?;
                    applied.await?;
                    connected = true;
                    let _ = complete.send(());
                }
                Some(Command::Controls(next)) => {
                    if connected {
                        host.set_audio_controls(next).await?;
                    }
                }
                Some(Command::Answer(..)) => anyhow::bail!("voice answer already applied"),
                None => return host.close().await,
            },
            _ = poll.tick() => {
                let audio = host.inspect_audio().await?;
                state.microphone.fetch_max(audio.microphone_peak, Ordering::Release);
                state.speaker.fetch_max(audio.speaker_peak, Ordering::Release);
            }
        }
    }
}

// Devices are still disabled. The snapshot and request enqueue share the setters' lock;
// later setters remain ordered after this request without waiting for its acknowledgement.
fn startup_controls<T>(
    commands: &mut mpsc::Receiver<Command>,
    controls: &Mutex<AudioControls>,
    dispatch: impl FnOnce(AudioControls) -> Result<T>,
) -> Result<T> {
    let controls = controls
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for _ in 0..commands.max_capacity() {
        match commands.try_recv() {
            Ok(Command::Controls(_)) => {}
            Err(mpsc::error::TryRecvError::Empty) => return dispatch(*controls),
            Ok(Command::Answer(..)) | Err(mpsc::error::TryRecvError::Disconnected) => {
                anyhow::bail!("voice startup control sequence invalid");
            }
        }
    }
    anyhow::ensure!(commands.is_empty(), "voice startup controls overloaded");
    dispatch(*controls)
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
