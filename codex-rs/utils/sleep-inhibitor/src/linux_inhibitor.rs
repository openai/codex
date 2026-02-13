use crate::PlatformSleepInhibitor;
use std::process::Child;
use std::process::Command;
use tracing::warn;

const ASSERTION_REASON: &str = "Codex is running an active turn";
const APP_ID: &str = "codex";
const BLOCKER_SLEEP_SECONDS: &str = "2147483647";

#[derive(Debug, Default)]
pub(crate) struct LinuxSleepInhibitor {
    state: InhibitState,
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
        if let InhibitState::Active { child, .. } = &mut self.state
            && child.try_wait().ok().flatten().is_none()
        {
            return;
        }

        self.state = InhibitState::Inactive;

        for backend in [
            LinuxBackend::SystemdInhibit,
            LinuxBackend::GnomeSessionInhibit,
        ] {
            match spawn_backend(backend) {
                Ok(child) => {
                    self.state = InhibitState::Active { backend, child };
                    return;
                }
                Err(error) => {
                    warn!(
                        ?backend,
                        reason = %error,
                        "Failed to start Linux sleep inhibitor backend"
                    );
                }
            }
        }

        warn!("No Linux sleep inhibitor backend is available");
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
                "--what=sleep",
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
                "suspend",
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
