use serde::Serialize;
use tracing::error;
use tracing::warn;

#[derive(Debug)]
pub(crate) struct UserNotifier {
    notify_command: Option<Vec<String>>,
    #[cfg(target_os = "macos")]
    native: macos::MacNotifier,
}

impl UserNotifier {
    pub(crate) fn notify(&self, notification: &UserNotification) {
        if let Some(notify_command) = &self.notify_command
            && !notify_command.is_empty()
        {
            if self.invoke_notify(notify_command, notification) {
                return;
            }
        }

        #[cfg(target_os = "macos")]
        self.native.notify(notification);
    }

    fn invoke_notify(&self, notify_command: &[String], notification: &UserNotification) -> bool {
        let Ok(json) = serde_json::to_string(&notification) else {
            error!("failed to serialise notification payload");
            return false;
        };

        let mut command = std::process::Command::new(&notify_command[0]);
        if notify_command.len() > 1 {
            command.args(&notify_command[1..]);
        }
        command.arg(json);

        // Fire-and-forget – we do not wait for completion.
        match command.spawn() {
            Ok(_) => true,
            Err(e) => {
                warn!("failed to spawn notifier '{}': {e}", notify_command[0]);
                false
            }
        }
    }

    pub(crate) fn new(notify: Option<Vec<String>>) -> Self {
        Self {
            notify_command: notify,
            #[cfg(target_os = "macos")]
            native: macos::MacNotifier::new(),
        }
    }
}

impl Default for UserNotifier {
    fn default() -> Self {
        Self::new(None)
    }
}

/// User can configure a program that will receive notifications. Each
/// notification is serialized as JSON and passed as an argument to the
/// program.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub(crate) enum UserNotification {
    #[serde(rename_all = "kebab-case")]
    AgentTurnComplete {
        thread_id: String,
        turn_id: String,
        cwd: String,

        /// Messages that the user sent to the agent to initiate the turn.
        input_messages: Vec<String>,

        /// The last message sent by the assistant in the turn.
        last_assistant_message: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn test_user_notification() -> Result<()> {
        let notification = UserNotification::AgentTurnComplete {
            thread_id: "b5f6c1c2-1111-2222-3333-444455556666".to_string(),
            turn_id: "12345".to_string(),
            cwd: "/Users/example/project".to_string(),
            input_messages: vec!["Rename `foo` to `bar` and update the callsites.".to_string()],
            last_assistant_message: Some(
                "Rename complete and verified `cargo build` succeeds.".to_string(),
            ),
        };
        let serialized = serde_json::to_string(&notification)?;
        assert_eq!(
            serialized,
            r#"{"type":"agent-turn-complete","thread-id":"b5f6c1c2-1111-2222-3333-444455556666","turn-id":"12345","cwd":"/Users/example/project","input-messages":["Rename `foo` to `bar` and update the callsites."],"last-assistant-message":"Rename complete and verified `cargo build` succeeds."}"#
        );
        Ok(())
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::UserNotification;
    use crate::config;
    use anyhow::Context;
    use anyhow::Result;
    use anyhow::anyhow;
    use mac_notification_sys::Notification;
    use mac_notification_sys::NotificationResponse;
    use serde_json;
    use std::env;
    use std::fs;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::path::PathBuf;
    use std::process::Command;
    use tempfile::NamedTempFile;
    use tracing::debug;
    use tracing::warn;

    const HELPER_ENV: &str = "CODEX_NOTIFIER_APP";
    const HELPER_BUNDLE_NAME: &str = "CodexNotifier.app";
    const HELPER_EXECUTABLE: &str = "Contents/MacOS/codex-notifier";
    const INFO_PLIST_TEMPLATE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../notifier/Resources/Info.plist"
    ));
    const ICON_BYTES: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../notifier/Resources/Codex.icns"
    ));

    #[derive(Debug)]
    pub(crate) struct MacNotifier {
        helper: Option<HelperApp>,
        legacy: LegacyNotifier,
    }

    impl MacNotifier {
        pub(crate) fn new() -> Self {
            let helper = match HelperApp::discover() {
                Ok(app) => {
                    debug!(
                        "using Codex helper notifier at {}",
                        app.bundle_path.display()
                    );
                    Some(app)
                }
                Err(err) => {
                    debug!("no helper notifier available: {err:?}");
                    None
                }
            };

            Self {
                helper,
                legacy: LegacyNotifier::new(),
            }
        }

        pub(crate) fn notify(&self, notification: &UserNotification) {
            if env::var("CODEX_SANDBOX").is_ok() {
                return;
            }
            if env::var("CI").is_ok() {
                return;
            }

            if let Some(helper) = &self.helper {
                if let Err(err) = helper.notify(notification) {
                    warn!("failed to send macOS notification via helper: {err:?}");
                } else {
                    return;
                }
            }

            self.legacy.notify(notification);
        }
    }

    #[derive(Debug)]
    struct HelperApp {
        bundle_path: PathBuf,
    }

    impl HelperApp {
        fn discover() -> Result<Self> {
            if let Ok(custom) = env::var(HELPER_ENV) {
                let candidate = PathBuf::from(custom);
                if Self::is_valid_bundle(&candidate) {
                    return Ok(Self {
                        bundle_path: candidate,
                    });
                }
            }

            for path in Self::candidate_paths()? {
                if Self::is_valid_bundle(&path) {
                    return Ok(Self { bundle_path: path });
                }
            }

            if let Ok(installed) = Self::install_to_codex_home() {
                if Self::is_valid_bundle(&installed) {
                    return Ok(Self {
                        bundle_path: installed,
                    });
                }
            }

            Err(anyhow!("helper bundle not found"))
        }

        fn candidate_paths() -> Result<Vec<PathBuf>> {
            let mut candidates = Vec::new();

            if let Ok(home) = config::find_codex_home() {
                candidates.push(home.join(HELPER_BUNDLE_NAME));
                candidates.push(home.join("bin").join(HELPER_BUNDLE_NAME));
            }

            if let Ok(exe) = env::current_exe() {
                if let Some(parent) = exe.parent() {
                    candidates.push(parent.join(HELPER_BUNDLE_NAME));
                    if let Some(grand) = parent.parent() {
                        candidates.push(grand.join(HELPER_BUNDLE_NAME));
                    }
                }
            }

            Ok(candidates)
        }

        fn install_to_codex_home() -> Result<PathBuf> {
            let mut base = config::find_codex_home()?;
            base.push("bin");
            fs::create_dir_all(&base)?;
            let bundle_path = base.join(HELPER_BUNDLE_NAME);
            if !Self::is_valid_bundle(&bundle_path) {
                Self::write_bundle(&bundle_path)?;
            }
            Ok(bundle_path)
        }

        fn write_bundle(destination: &Path) -> Result<()> {
            let contents = destination.join("Contents");
            let macos_dir = contents.join("MacOS");
            let resources_dir = contents.join("Resources");
            fs::create_dir_all(&macos_dir)?;
            fs::create_dir_all(&resources_dir)?;

            fs::write(contents.join("Info.plist"), INFO_PLIST_TEMPLATE)
                .context("failed to write Info.plist")?;
            fs::write(resources_dir.join("Codex.icns"), ICON_BYTES)
                .context("failed to write Codex.icns")?;

            let binary_src = Self::locate_binary().context("codex-notifier binary not found")?;
            let binary_dest = macos_dir.join("codex-notifier");
            fs::copy(&binary_src, &binary_dest)
                .with_context(|| format!("failed to copy notifier binary from {binary_src:?}"))?;

            let mut perms = fs::metadata(&binary_dest)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&binary_dest, perms)?;

            Ok(())
        }

        fn locate_binary() -> Result<PathBuf> {
            for candidate in Self::binary_candidate_paths()? {
                if candidate.is_file() {
                    return Ok(candidate);
                }
                if candidate.is_dir() {
                    let nested = candidate.join("codex-notifier");
                    if nested.is_file() {
                        return Ok(nested);
                    }
                }
            }
            Err(anyhow!("codex-notifier binary unavailable"))
        }

        fn binary_candidate_paths() -> Result<Vec<PathBuf>> {
            let mut paths = Vec::new();

            if let Ok(exe) = env::current_exe() {
                if let Some(parent) = exe.parent() {
                    paths.push(parent.join("codex-notifier"));
                    paths.push(parent.join("../codex-notifier"));
                    paths.push(parent.join("../../codex-notifier"));
                    if let Some(grand) = parent.parent() {
                        paths.push(grand.join("codex-notifier"));
                        paths.push(grand.join("release").join("codex-notifier"));
                    }
                }
            }

            if let Ok(home) = config::find_codex_home() {
                paths.push(home.join("bin").join("codex-notifier"));
            }

            Ok(paths)
        }

        fn is_valid_bundle(path: &Path) -> bool {
            let exec = path.join(HELPER_EXECUTABLE);
            exec.is_file()
        }

        fn notify(&self, notification: &UserNotification) -> Result<()> {
            let payload = serde_json::to_vec(notification)?;

            let mut temp = NamedTempFile::new().context("failed to create temp payload file")?;
            temp.as_file_mut()
                .write_all(&payload)
                .context("failed to write payload")?;
            temp.flush().ok();

            let temp_path = temp.into_temp_path();
            let payload_path = temp_path
                .keep()
                .context("failed to persist payload file for helper app")?;

            let status = Command::new("open")
                .arg("-n")
                .arg(&self.bundle_path)
                .arg("--args")
                .arg("--payload-file")
                .arg(&payload_path)
                .status()
                .with_context(|| {
                    format!("failed to invoke open for {}", self.bundle_path.display())
                })?;

            if !status.success() {
                let _ = fs::remove_file(&payload_path);
                return Err(anyhow!("open exited with status {status:?}"));
            }

            Ok(())
        }
    }

    #[derive(Debug)]
    struct LegacyNotifier {
        icon_path: Option<PathBuf>,
    }

    impl LegacyNotifier {
        fn new() -> Self {
            if let Err(err) = mac_notification_sys::set_application("com.openai.codex") {
                warn!("failed to register bundle id for notifications: {err}");
            }

            let icon_path = Self::ensure_icon()
                .map_err(|err| {
                    warn!("failed to prepare macOS notification icon: {err}");
                })
                .ok();

            Self { icon_path }
        }

        fn notify(&self, notification: &UserNotification) {
            let (title, subtitle, message) = match notification {
                UserNotification::AgentTurnComplete {
                    last_assistant_message,
                    input_messages,
                    ..
                } => {
                    let title = "Codex CLI";
                    let subtitle = last_assistant_message
                        .as_ref()
                        .map(std::string::String::as_str)
                        .unwrap_or("Turn complete");
                    let message = if input_messages.is_empty() {
                        String::from("Agent turn finished")
                    } else {
                        input_messages.join(" ")
                    };
                    (title.to_string(), subtitle.to_string(), message)
                }
            };

            let mut payload = Notification::new();
            payload.title(&title);
            payload.maybe_subtitle(Some(&subtitle));
            payload.message(&message);
            payload.default_sound();

            if let Some(icon_path) = self.icon_path.as_ref().and_then(|p| p.to_str()) {
                payload.app_icon(icon_path);
            }

            match payload.send() {
                Ok(NotificationResponse::ActionButton(action)) => {
                    debug!("Codex notification action pressed: {action}");
                }
                Ok(NotificationResponse::CloseButton(label)) => {
                    debug!("Codex notification dismissed via '{label}' button");
                }
                Ok(NotificationResponse::Reply(body)) => {
                    debug!("Codex notification reply entered (ignored): {body}");
                }
                Ok(NotificationResponse::Click) => {
                    debug!("Codex notification clicked");
                }
                Ok(NotificationResponse::None) => {}
                Err(err) => warn!("failed to deliver macOS notification via legacy path: {err}"),
            }
        }

        fn ensure_icon() -> anyhow::Result<PathBuf> {
            const ICON_BYTES: &[u8] = include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/codex-notification.png"
            ));

            let mut path = config::find_codex_home()?;
            path.push("assets");
            fs::create_dir_all(&path)?;
            path.push("codex-notification.png");

            let needs_write = match fs::read(&path) {
                Ok(existing) => existing != ICON_BYTES,
                Err(_) => true,
            };

            if needs_write {
                fs::write(&path, ICON_BYTES)?;
                let mut perms = fs::metadata(&path)?.permissions();
                perms.set_mode(0o644);
                fs::set_permissions(&path, perms)?;
            }

            Ok(path)
        }
    }
}
