//! Reports app-server daemon state without starting or stopping the daemon.
//!
//! The background-server check is deliberately passive. It reads the daemon
//! state directory, PID files, settings file, and control socket path, then
//! attempts only a local socket connection when a socket already exists. That
//! keeps doctor safe to run while the user is debugging startup or update-loop
//! issues.

use std::path::Path;

use codex_core::config::Config;

use super::CheckStatus;
use super::DoctorCheck;

const STATE_DIR_NAME: &str = "app-server-daemon";
const SETTINGS_FILE_NAME: &str = "settings.json";
const PID_FILE_NAME: &str = "app-server.pid";
const UPDATE_PID_FILE_NAME: &str = "app-server-updater.pid";

/// Builds the app-server status row from existing daemon state.
///
/// Missing files are expected for the ephemeral/not-running case and should not
/// be treated as failures. A stale socket is a warning because it can explain
/// client connection problems without proving the daemon itself is broken.
pub(super) fn background_server_check(config: &Config) -> DoctorCheck {
    let mut details = Vec::new();
    let state_dir = config.codex_home.join(STATE_DIR_NAME);
    details.push(format!("daemon state dir: {}", state_dir.display()));
    push_file_detail(
        &mut details,
        "settings",
        &state_dir.join(SETTINGS_FILE_NAME),
    );
    push_file_detail(&mut details, "pid file", &state_dir.join(PID_FILE_NAME));
    push_file_detail(
        &mut details,
        "update-loop pid file",
        &state_dir.join(UPDATE_PID_FILE_NAME),
    );

    let socket_path = match codex_app_server::app_server_control_socket_path(&config.codex_home) {
        Ok(socket_path) => socket_path,
        Err(err) => {
            return DoctorCheck::new(
                "app_server.status",
                "app-server",
                CheckStatus::Warning,
                "background server socket path could not be resolved",
            )
            .details(details)
            .detail(err.to_string());
        }
    };

    details.push(format!("control socket: {}", socket_path.display()));
    let status = socket_status(socket_path.as_path());
    details.push(format!("status: {}", status.detail_label()));
    details.push(format!("mode: {}", server_mode(&state_dir)));

    let mut check = DoctorCheck::new(
        "app_server.status",
        "app-server",
        status.check_status(),
        status.summary(),
    )
    .details(details);
    if status.check_status() == CheckStatus::Warning {
        check = check.remediation("Run codex app-server daemon version for more details.");
    }
    check
}

fn push_file_detail(details: &mut Vec<String>, label: &str, path: &Path) {
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => {
            details.push(format!("{label}: {} (file)", path.display()));
        }
        Ok(_) => {
            details.push(format!("{label}: {} (not a file)", path.display()));
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            details.push(format!("{label}: {} (missing)", path.display()));
        }
        Err(err) => details.push(format!("{label}: {} ({err})", path.display())),
    }
}

fn server_mode(state_dir: &Path) -> &'static str {
    if state_dir.join(SETTINGS_FILE_NAME).is_file() {
        "persistent"
    } else {
        "ephemeral"
    }
}

#[derive(Clone, Copy)]
enum SocketStatus {
    NotRunning,
    Running,
    #[cfg(unix)]
    StaleOrUnreachable,
}

impl SocketStatus {
    fn check_status(self) -> CheckStatus {
        match self {
            Self::NotRunning | Self::Running => CheckStatus::Ok,
            #[cfg(unix)]
            Self::StaleOrUnreachable => CheckStatus::Warning,
        }
    }

    fn summary(self) -> &'static str {
        match self {
            Self::NotRunning => "background server is not running",
            Self::Running => "background server is running",
            #[cfg(unix)]
            Self::StaleOrUnreachable => "background server socket is stale or unreachable",
        }
    }

    fn detail_label(self) -> &'static str {
        match self {
            Self::NotRunning => "not running",
            Self::Running => "running",
            #[cfg(unix)]
            Self::StaleOrUnreachable => "stale or unreachable",
        }
    }
}

fn socket_status(socket_path: &Path) -> SocketStatus {
    if !socket_path.exists() {
        return SocketStatus::NotRunning;
    }

    #[cfg(unix)]
    {
        match std::os::unix::net::UnixStream::connect(socket_path) {
            Ok(_) => SocketStatus::Running,
            Err(_) => SocketStatus::StaleOrUnreachable,
        }
    }

    #[cfg(not(unix))]
    {
        SocketStatus::Running
    }
}
