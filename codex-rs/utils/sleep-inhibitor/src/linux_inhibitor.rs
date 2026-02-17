use crate::PlatformSleepInhibitor;
use std::process::Child;
use std::process::Command;
use tracing::warn;

const ASSERTION_REASON: &str = "Codex is running an active turn";
const APP_ID: &str = "codex";
// Keep the blocker process alive "long enough" without needing restarts.
// This is `i32::MAX` seconds, which is accepted by common `sleep` implementations.
const BLOCKER_SLEEP_SECONDS: &str = "2147483647";

#[derive(Debug, Default)]
pub(crate) struct LinuxSleepInhibitor {
    state: InhibitState,
    preferred_backend: Option<LinuxBackend>,
    missing_backend_logged: bool,
}

#[derive(Debug, Default)]
enum InhibitState {
    #[default]
    Inactive,
    Active {
        backend: LinuxBackend,
        child: Child,
    },
}

#[derive(Debug, Clone, Copy)]
enum LinuxBackend {
    SystemdInhibit,
    GnomeSessionInhibit,
}

impl LinuxSleepInhibitor {
    pub(crate) fn new() -> Self {
        Self::default()
    }
}

impl PlatformSleepInhibitor for LinuxSleepInhibitor {
    fn acquire(&mut self) {
        if let InhibitState::Active { backend, child } = &mut self.state {
            match child.try_wait() {
                Ok(None) => return,
                Ok(Some(status)) => {
                    warn!(
                        ?backend,
                        ?status,
                        "Linux sleep inhibitor backend exited unexpectedly; attempting fallback"
                    );
                }
                Err(error) => {
                    warn!(
                        ?backend,
                        reason = %error,
                        "Failed to query Linux sleep inhibitor backend status"
                    );
                    return;
                }
            }
        }

        self.state = InhibitState::Inactive;
        let should_log_backend_failures = !self.missing_backend_logged;
        let backends = match self.preferred_backend {
            Some(LinuxBackend::SystemdInhibit) => [
                LinuxBackend::SystemdInhibit,
                LinuxBackend::GnomeSessionInhibit,
            ],
            Some(LinuxBackend::GnomeSessionInhibit) => [
                LinuxBackend::GnomeSessionInhibit,
                LinuxBackend::SystemdInhibit,
            ],
            None => [
                LinuxBackend::SystemdInhibit,
                LinuxBackend::GnomeSessionInhibit,
            ],
        };

        for backend in backends {
            match spawn_backend(backend) {
                Ok(mut child) => match child.try_wait() {
                    Ok(None) => {
                        self.state = InhibitState::Active { backend, child };
                        self.preferred_backend = Some(backend);
                        self.missing_backend_logged = false;
                        return;
                    }
                    Ok(Some(status)) => {
                        if should_log_backend_failures {
                            warn!(
                                ?backend,
                                ?status,
                                "Linux sleep inhibitor backend exited immediately"
                            );
                        }
                    }
                    Err(error) => {
                        if should_log_backend_failures {
                            warn!(
                                ?backend,
                                reason = %error,
                                "Failed to query Linux sleep inhibitor backend status after spawn"
                            );
                        }
                    }
                },
                Err(error) => {
                    if should_log_backend_failures && error.kind() != std::io::ErrorKind::NotFound {
                        warn!(
                            ?backend,
                            reason = %error,
                            "Failed to start Linux sleep inhibitor backend"
                        );
                    }
                }
            }
        }

        if should_log_backend_failures {
            warn!("No Linux sleep inhibitor backend is available");
            self.missing_backend_logged = true;
        }
    }

    fn release(&mut self) {
        match std::mem::take(&mut self.state) {
            InhibitState::Inactive => {}
            InhibitState::Active { backend, mut child } => {
                if let Err(error) = child.kill()
                    && !child_exited(&error)
                {
                    warn!(?backend, reason = %error, "Failed to stop Linux sleep inhibitor backend");
                }
                if let Err(error) = child.wait()
                    && !child_exited(&error)
                {
                    warn!(?backend, reason = %error, "Failed to reap Linux sleep inhibitor backend");
                }
            }
        }
    }
}

impl Drop for LinuxSleepInhibitor {
    fn drop(&mut self) {
        self.release();
    }
}

fn spawn_backend(backend: LinuxBackend) -> Result<Child, std::io::Error> {
    match backend {
        LinuxBackend::SystemdInhibit => Command::new("systemd-inhibit")
            .args([
                "--what=idle",
                "--mode=block",
                "--who",
                APP_ID,
                "--why",
                ASSERTION_REASON,
                "--",
                "sleep",
                BLOCKER_SLEEP_SECONDS,
            ])
            .spawn(),
        LinuxBackend::GnomeSessionInhibit => Command::new("gnome-session-inhibit")
            .args([
                "--inhibit",
                "idle",
                "--reason",
                ASSERTION_REASON,
                "sleep",
                BLOCKER_SLEEP_SECONDS,
            ])
            .spawn(),
    }
}

fn child_exited(error: &std::io::Error) -> bool {
    matches!(error.kind(), std::io::ErrorKind::InvalidInput)
}
