//! Reports app-server daemon state without starting or stopping the daemon.
//!
//! The background-server check is deliberately passive. It reads the daemon
//! state directory, PID files, settings file, and control socket path, then
//! attempts only a bounded initialize probe when a socket already exists. That
//! keeps doctor safe to run while the user is debugging startup or update-loop
//! issues.

use std::path::Path;

use codex_core::config::Config;

use super::CheckStatus;
use super::DoctorCheck;

const MAX_PROBE_ERROR_CHARS: usize = 120;
const STATE_DIR_NAME: &str = "app-server-daemon";
const SETTINGS_FILE_NAME: &str = "settings.json";
const PID_FILE_NAME: &str = "app-server.pid";
const UPDATE_PID_FILE_NAME: &str = "app-server-updater.pid";

/// Builds the app-server status row from existing daemon state.
///
/// Missing files are expected for the ephemeral/not-running case and should not
/// be treated as failures. A stale socket is a warning because it can explain
/// client connection problems without proving the daemon itself is broken.
pub(super) async fn background_server_check(config: &Config) -> DoctorCheck {
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
    let status = socket_status(socket_path.as_path()).await;
    details.push(format!("status: {}", status.detail_label()));
    if let Some(version_detail) = status.app_server_version_detail() {
        details.push(version_detail);
    }
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

enum SocketStatus {
    NotRunning,
    Running(String),
    StaleOrUnreachable(String),
}

impl SocketStatus {
    fn check_status(&self) -> CheckStatus {
        match self {
            Self::NotRunning | Self::Running(_) => CheckStatus::Ok,
            Self::StaleOrUnreachable(_) => CheckStatus::Warning,
        }
    }

    fn summary(&self) -> &'static str {
        match self {
            Self::NotRunning => "background server is not running",
            Self::Running(_) => "background server is running",
            Self::StaleOrUnreachable(_) => "background server socket is stale or unreachable",
        }
    }

    fn detail_label(&self) -> &'static str {
        match self {
            Self::NotRunning => "not running",
            Self::Running(_) => "running",
            Self::StaleOrUnreachable(_) => "stale or unreachable",
        }
    }

    fn app_server_version_detail(&self) -> Option<String> {
        match self {
            Self::NotRunning => None,
            Self::Running(app_server_version) => {
                Some(format!("app-server version: {app_server_version}"))
            }
            Self::StaleOrUnreachable(error) => {
                Some(format!("app-server version: unavailable ({error})"))
            }
        }
    }
}

async fn socket_status(socket_path: &Path) -> SocketStatus {
    if !socket_path.exists() {
        return SocketStatus::NotRunning;
    }

    match codex_app_server_daemon::probe_app_server_version(socket_path).await {
        Ok(app_server_version) => SocketStatus::Running(app_server_version),
        Err(err) => SocketStatus::StaleOrUnreachable(concise_probe_error(&err, socket_path)),
    }
}

fn concise_probe_error(err: &anyhow::Error, socket_path: &Path) -> String {
    let socket_path = socket_path.display().to_string();
    let message = err
        .to_string()
        .replace(&socket_path, "control socket")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if message.is_empty() {
        return "unknown error".to_string();
    }
    let mut chars = message.chars();
    let truncated = chars
        .by_ref()
        .take(MAX_PROBE_ERROR_CHARS)
        .collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        message
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use codex_core::config::ConfigBuilder;
    use pretty_assertions::assert_eq;

    use super::*;

    async fn test_config(codex_home: PathBuf) -> Config {
        ConfigBuilder::default()
            .codex_home(codex_home)
            .build()
            .await
            .expect("config")
    }

    fn create_socket_placeholder(config: &Config) {
        let socket_path = codex_app_server::app_server_control_socket_path(&config.codex_home)
            .expect("socket path");
        std::fs::create_dir_all(socket_path.parent().expect("socket parent"))
            .expect("create socket dir");
        std::fs::write(socket_path, "").expect("create socket placeholder");
    }

    #[tokio::test]
    async fn not_running_background_server_stays_ok_without_version() {
        let temp = tempfile::tempdir().expect("tempdir");
        let config = test_config(temp.path().to_path_buf()).await;

        let check = background_server_check(&config).await;

        assert_eq!(check.status, CheckStatus::Ok);
        assert_eq!(check.summary, "background server is not running");
        assert!(check.details.contains(&"status: not running".to_string()));
        assert!(
            !check
                .details
                .iter()
                .any(|detail| detail.starts_with("app-server version:"))
        );
    }

    #[test]
    fn running_background_server_reports_app_server_version() {
        let status = SocketStatus::Running("1.2.3".to_string());

        assert_eq!(status.check_status(), CheckStatus::Ok);
        assert_eq!(status.summary(), "background server is running");
        assert_eq!(status.detail_label(), "running");
        assert_eq!(
            status.app_server_version_detail(),
            Some("app-server version: 1.2.3".to_string())
        );
    }

    #[tokio::test]
    async fn failed_version_probe_reports_unavailable() {
        let temp = tempfile::tempdir().expect("tempdir");
        let config = test_config(temp.path().to_path_buf()).await;
        create_socket_placeholder(&config);

        let check = background_server_check(&config).await;

        assert_eq!(check.status, CheckStatus::Warning);
        assert_eq!(
            check.summary,
            "background server socket is stale or unreachable"
        );
        assert!(
            check
                .details
                .contains(&"status: stale or unreachable".to_string())
        );
        assert!(
            check
                .details
                .iter()
                .any(|detail| detail.starts_with("app-server version: unavailable ("))
        );
    }
}
