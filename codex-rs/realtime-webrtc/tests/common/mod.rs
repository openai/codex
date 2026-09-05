//! Copies this test executable into a private package and dispatches its fake helper before libtest.
//! The fixture validates protocol order; it never loads native libraries or opens audio devices.

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use codex_realtime_webrtc::AudioState;
use codex_realtime_webrtc::Message;
use codex_realtime_webrtc::encode_frame;
use codex_realtime_webrtc::read_message;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

pub const BUILD_COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
pub const WAIT: Duration = Duration::from_secs(/*secs*/ 10);
const APP: &str = if cfg!(windows) { "codex.exe" } else { "codex" };
const HELPER: &str = if cfg!(windows) {
    "codex-voice-host.exe"
} else {
    "codex-voice-host"
};

pub fn wait_for(mut ready: impl FnMut() -> bool) -> Result<()> {
    let deadline = Instant::now() + WAIT;
    while !ready() {
        ensure!(Instant::now() < deadline, "fixture deadline expired");
        thread::sleep(Duration::from_millis(/*millis*/ 5));
    }
    Ok(())
}

#[cfg(unix)]
pub fn wait_for_helper_reaped(root: &std::path::Path) -> Result<()> {
    let pid: libc::pid_t = fs::read_to_string(root.join("helper-pid"))?.parse()?;
    wait_for(|| {
        // SAFETY: Signal zero checks process existence without delivering a signal.
        let result = unsafe {
            libc::kill(pid, /*sig*/ 0)
        };
        result == -1 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
    })
}

struct Package(PathBuf);
impl Drop for Package {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// The child discovers its installation through current_exe, exactly as the public facade does.
pub fn package(test: &str) -> Result<Option<PathBuf>> {
    let source = std::env::current_exe()?;
    if source.file_name().is_some_and(|name| name == APP) {
        codex_build_info::BuildInfo::initialize(BUILD_COMMIT);
        return Ok(Some(
            source
                .parent()
                .context("bin")?
                .parent()
                .context("package")?
                .to_owned(),
        ));
    }
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let package =
        Package(std::env::temp_dir().join(format!("voice-actor-{}-{nonce}", std::process::id())));
    let root = &package.0;
    fs::create_dir_all(root.join("bin"))?;
    fs::create_dir_all(root.join("codex-resources/voice/bin"))?;
    fs::write(root.join("codex-package.json"), "{}")?;
    fs::copy(&source, root.join("bin").join(APP))?;
    fs::copy(&source, root.join("codex-resources/voice/bin").join(HELPER))?;
    let mut child = Command::new(root.join("bin").join(APP))
        .args(["--exact", test, "--nocapture"])
        .current_dir(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let deadline = Instant::now() + WAIT * 3;
    while child.try_wait()?.is_none() {
        if Instant::now() >= deadline {
            child.kill()?;
            child.wait()?;
            anyhow::bail!("packaged test timed out");
        }
        thread::sleep(Duration::from_millis(/*millis*/ 10));
    }
    let output = child.wait_with_output()?;
    ensure!(
        output.status.success(),
        "packaged test failed: {}\n{}\nfixture: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
        fs::read_to_string(root.join("fixture-error")).unwrap_or_default()
    );
    Ok(None)
}

#[ctor::ctor]
fn dispatch_helper() {
    if std::env::current_exe()
        .ok()
        .and_then(|path| path.file_name().map(std::ffi::OsStr::to_owned))
        .is_some_and(|name| name == HELPER)
    {
        if let Err(error) = helper() {
            let _ = fs::write("fixture-error", format!("{error:#}"));
            std::process::exit(1);
        }
        std::process::exit(0);
    }
}

#[derive(PartialEq)]
enum Stage {
    Hello,
    Initialize,
    Offer,
    Answer,
    Devices,
    Controls,
    Running,
}

fn helper() -> Result<()> {
    let root = std::env::current_dir()?;
    fs::write(root.join("helper-pid"), std::process::id().to_string())?;
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    let mut stage = Stage::Hello;
    let mut controls_history = Vec::new();
    while let Some(request) = read_message(&mut input)? {
        let response = match request {
            Message::Hello {
                protocol: 1,
                build_commit,
            } if stage == Stage::Hello => {
                ensure!(build_commit == BUILD_COMMIT, "wrong executable build stamp");
                stage = Stage::Initialize;
                Message::Ready {}
            }
            Message::InitializeRuntime {} if stage == Stage::Initialize => {
                fs::write(root.join("initializing"), [])?;
                if root.join("hold-initialization").exists() {
                    wait_for(|| root.join("release").exists())?;
                }
                stage = Stage::Offer;
                Message::RuntimeReady {}
            }
            Message::StartTransport {} if stage == Stage::Offer => {
                stage = Stage::Answer;
                Message::Offer {
                    sdp: "synthetic-offer"
                        .to_owned()
                        .try_into()
                        .map_err(anyhow::Error::msg)?,
                }
            }
            Message::ApplyAnswer { sdp } if stage == Stage::Answer => {
                ensure!(
                    sdp.into_sdp() == "synthetic-answer",
                    "unexpected fixture answer"
                );
                fs::write(root.join("answer"), [])?;
                wait_for(|| root.join("release").exists())?;
                stage = Stage::Devices;
                Message::TransportReady {}
            }
            Message::OpenDevices {} if stage == Stage::Devices => {
                stage = Stage::Controls;
                Message::DevicesOpened {}
            }
            Message::SetAudioControls { controls }
                if stage == Stage::Controls || stage == Stage::Running =>
            {
                controls_history.push(controls);
                fs::write(
                    root.join("controls"),
                    serde_json::to_vec(&controls_history)?,
                )?;
                stage = Stage::Running;
                Message::AudioControlsApplied {}
            }
            Message::InspectAudio {} if stage == Stage::Answer || stage == Stage::Running => {
                if root.join("exit").exists() {
                    return Ok(());
                }
                Message::AudioState {
                    state: if stage == Stage::Running {
                        AudioState {
                            microphone_peak: 123,
                            speaker_peak: 456,
                        }
                    } else {
                        AudioState::default()
                    },
                }
            }
            Message::Close {} => {
                output.write_all(&encode_frame(&Message::Closed {})?)?;
                output.flush()?;
                return Ok(());
            }
            _ => anyhow::bail!("unexpected fixture protocol sequence"),
        };
        output.write_all(&encode_frame(&response)?)?;
        output.flush()?;
    }
    Ok(())
}
