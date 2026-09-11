//! Resolve only the installed helper and retain the existing owned-process cleanup guard.

use std::collections::HashMap;
use std::ffi::OsString;
use std::time::Duration;

use anyhow::Result;
use anyhow::ensure;
use codex_install_context::CodexPackageLayout;
use codex_utils_pty::ProcessHandle;
use codex_utils_pty::SpawnedProcess;
use tokio::sync::oneshot;
use tokio::time::timeout;

use crate::HelperExitStage;
use crate::Message;
use crate::encode_frame;
use crate::message_reader::MessageReader;

const DEADLINE: Duration = Duration::from_secs(/*secs*/ 5);
const RUNTIME_INITIALIZATION_DEADLINE: Duration = Duration::from_secs(/*secs*/ 30);

/// Startup failures eligible for recovery remain distinct from all other failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionError {
    NegotiationTimedOut,
    Failed,
    HelperStartup,
    RuntimeInitialization,
    Transport,
    AudioDevices,
    AudioControls,
    AudioSession,
    Shutdown,
}
impl std::fmt::Display for ConnectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NegotiationTimedOut => "voice negotiation timed out",
            Self::Failed => "voice connection failed",
            Self::HelperStartup => "voice helper could not start",
            Self::RuntimeInitialization => "voice audio runtime could not initialize",
            Self::Transport => "voice transport could not connect",
            Self::AudioDevices => {
                "voice audio devices could not open; check microphone and speaker setup"
            }
            Self::AudioControls => "voice audio controls failed",
            Self::AudioSession => "voice audio session stopped unexpectedly",
            Self::Shutdown => "voice helper could not shut down cleanly",
        })
    }
}
impl std::error::Error for ConnectionError {}

/// Owns one helper. Dropping it terminates the process and leaves its waiter to reap it.
/// A successful handshake establishes compatibility only, not an active audio session.
pub struct VoiceHost {
    process: ProcessHandle,
    output: MessageReader,
    exit: oneshot::Receiver<i32>,
    observed_exit: Option<Option<i32>>,
}

impl VoiceHost {
    /// Open local devices only after answer negotiation. They initially remain muted/suppressed.
    pub async fn open_devices(mut self) -> Result<Self> {
        self.exchange(Message::OpenDevices {}, Message::DevicesOpened {}, DEADLINE)
            .await?;
        Ok(self)
    }

    /// Acknowledgement follows invalidation of the helper's previous capture/render generations.
    pub async fn set_audio_controls(&mut self, controls: crate::AudioControls) -> Result<()> {
        self.exchange(
            Message::SetAudioControls { controls },
            Message::AudioControlsApplied {},
            DEADLINE,
        )
        .await
    }

    /// Enqueue startup controls synchronously so the facade can order them with setters.
    /// The returned future owns only the acknowledgement wait, never the facade control lock.
    pub(crate) fn begin_audio_controls(
        &mut self,
        controls: crate::AudioControls,
    ) -> Result<impl std::future::Future<Output = Result<()>> + '_> {
        self.process
            .writer_sender()
            .try_send(encode_frame(&Message::SetAudioControls { controls })?)
            .map_err(|_| anyhow::anyhow!("voice helper input unavailable"))?;
        let deadline = tokio::time::Instant::now() + DEADLINE;
        Ok(async move {
            let response = tokio::time::timeout_at(deadline, self.next_response()).await??;
            ensure!(
                response == Message::AudioControlsApplied {},
                "unexpected voice helper response"
            );
            Ok(())
        })
    }

    /// Consume peaks and detect helper loss even when neither device is producing audio.
    pub async fn inspect_audio(&mut self) -> Result<crate::AudioState> {
        let response = self.request(Message::InspectAudio {}, DEADLINE).await?;
        let Message::AudioState { state } = response else {
            anyhow::bail!("unexpected voice helper response");
        };
        Ok(state)
    }

    /// Gather an offer in the helper. This establishes neither connectivity nor audio readiness.
    pub async fn start_transport(mut self) -> Result<(Self, crate::SessionDescription)> {
        let response = self
            .request(Message::StartTransport {}, Duration::from_secs(/*secs*/ 20))
            .await?;
        let Message::Offer { sdp } = response else {
            anyhow::bail!("unexpected voice helper response");
        };
        Ok((self, sdp))
    }

    /// Return only when the peer's ordered event channel has opened.
    pub async fn apply_answer(mut self, sdp: crate::SessionDescription) -> Result<Self> {
        let response = self
            .request(
                Message::ApplyAnswer { sdp },
                Duration::from_secs(/*secs*/ 20),
            )
            .await?;
        if response == (Message::TransportTimedOut {}) {
            self.process.terminate();
            // A lost exit notification or failed cleanup is not retryable.
            timeout(DEADLINE, &mut self.exit).await??;
            return Err(ConnectionError::NegotiationTimedOut.into());
        }
        ensure!(
            response == Message::TransportReady {},
            "unexpected voice helper response"
        );
        Ok(self)
    }

    /// Initialize the packaged native runtime without opening devices or starting a session.
    pub async fn initialize_runtime(mut self) -> Result<Self> {
        self.exchange(
            Message::InitializeRuntime {},
            Message::RuntimeReady {},
            RUNTIME_INITIALIZATION_DEADLINE,
        )
        .await?;
        Ok(self)
    }

    pub async fn connect(package: &CodexPackageLayout, build_commit: &str) -> Result<Self> {
        let root = package.package_dir.as_path().canonicalize()?;
        let name = if cfg!(windows) {
            "codex-voice-host.exe"
        } else {
            "codex-voice-host"
        };
        let path = root.join("codex-resources/voice/bin").join(name);
        ensure!(
            path.canonicalize()? == path,
            "voice helper must be inside the physical package"
        );
        let environment = child_environment(std::env::vars_os());
        #[cfg(target_os = "linux")]
        let environment = {
            let mut environment = environment;
            if let Some(directory) =
                crate::linux_alsa::plugin_directory(crate::linux_alsa::PLUGIN_DIRECTORIES)
            {
                environment.insert("ALSA_PLUGIN_DIR".to_owned(), directory.to_owned());
            }
            environment
        };
        let SpawnedProcess {
            session,
            stdout_rx,
            stderr_rx,
            exit_rx,
        } = codex_utils_pty::spawn_pipe_process(
            &path,
            &[],
            &root,
            &environment,
            /*arg0*/ &None,
            &[],
        )
        .await?;
        drop(stderr_rx); // Drain and discard diagnostics rather than logging untyped child output.
        let mut host = Self {
            process: session,
            output: MessageReader::new(stdout_rx),
            exit: exit_rx,
            observed_exit: None,
        };
        host.exchange(
            Message::Hello {
                protocol: 1,
                build_commit: build_commit.to_owned(),
            },
            Message::Ready {},
            // Startup-linked native libraries load before the helper can acknowledge Hello.
            RUNTIME_INITIALIZATION_DEADLINE,
        )
        .await?;
        Ok(host)
    }

    pub async fn close(mut self) -> Result<()> {
        let result = self
            .exchange(Message::Close {}, Message::Closed {}, DEADLINE)
            .await;
        if result.is_err() {
            self.process.terminate();
        }
        let code = match self.observed_exit {
            Some(Some(code)) => code,
            Some(None) => anyhow::bail!("voice helper exit status unavailable"),
            None => timeout(DEADLINE, &mut self.exit).await??,
        };
        result?;
        ensure!(code == 0, "voice helper failed during shutdown");
        Ok(())
    }

    async fn exchange(
        &mut self,
        request: Message,
        expected: Message,
        deadline: Duration,
    ) -> Result<()> {
        ensure!(
            self.request(request, deadline).await? == expected,
            "unexpected voice helper response"
        );
        Ok(())
    }

    async fn request(&mut self, request: Message, deadline: Duration) -> Result<Message> {
        timeout(deadline, async {
            self.process
                .writer_sender()
                .send(encode_frame(&request)?)
                .await
                .map_err(|_| anyhow::anyhow!("voice helper input closed"))?;
            self.next_response().await
        })
        .await?
    }

    async fn next_response(&mut self) -> Result<Message> {
        match self.output.next().await {
            Ok(response) => Ok(response),
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
                // The helper can exit just after closing stdout. Wait briefly for its
                // status; never log its untyped stderr, SDP, or native error text.
                let exit_code = match self.observed_exit {
                    Some(status) => status,
                    None => {
                        match timeout(Duration::from_millis(/*millis*/ 250), &mut self.exit).await {
                            Ok(status) => {
                                let status = status.ok();
                                self.observed_exit = Some(status);
                                status
                            }
                            Err(_) => None,
                        }
                    }
                };
                let phase = exit_code.and_then(HelperExitStage::from_code);
                tracing::warn!(?phase, ?exit_code, "voice helper output closed");
                Err(error.into())
            }
            Err(error) => Err(error.into()),
        }
    }
}

fn child_environment(vars: impl Iterator<Item = (OsString, OsString)>) -> HashMap<String, String> {
    vars.filter_map(|(key, value)| {
        let key = key.into_string().ok()?;
        matches!(
            key.to_ascii_uppercase().as_str(),
            "SYSTEMROOT"
                | "WINDIR"
                | "HOME"
                | "USERPROFILE"
                | "LOCALAPPDATA"
                | "APPDATA"
                | "TEMP"
                | "TMP"
                | "TMPDIR"
                | "XDG_RUNTIME_DIR"
                | "PULSE_SERVER"
                | "PULSE_COOKIE"
                | "PIPEWIRE_REMOTE"
                | "DBUS_SESSION_BUS_ADDRESS"
                | "HTTP_PROXY"
                | "HTTPS_PROXY"
                | "ALL_PROXY"
                | "NO_PROXY"
                | "SSL_CERT_FILE"
                | "SSL_CERT_DIR"
                | "REQUESTS_CA_BUNDLE"
                | "CURL_CA_BUNDLE"
        )
        .then(|| Some((key, value.into_string().ok()?)))
        .flatten()
    })
    .chain(
        crate::RUNTIME_ENVIRONMENT
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value.to_owned())),
    )
    .collect()
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
