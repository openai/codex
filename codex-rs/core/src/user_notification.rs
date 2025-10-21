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
    use mac_notification_sys::Notification;
    use mac_notification_sys::NotificationResponse;
    use std::fs;
    use std::path::PathBuf;
    use tracing::debug;
    use tracing::warn;

    #[derive(Debug)]
    pub(crate) struct MacNotifier {
        icon_path: Option<PathBuf>,
    }

    impl MacNotifier {
        pub(crate) fn new() -> Self {
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

        pub(crate) fn notify(&self, notification: &UserNotification) {
            if std::env::var("CODEX_SANDBOX").is_ok() {
                // Avoid firing real notifications when running inside our sandboxed Seatbelt harness.
                return;
            }

            if std::env::var("CI").is_ok() {
                // Skip macOS notifications when running in CI environments.
                return;
            }

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
                Err(err) => warn!("failed to deliver macOS notification: {err}"),
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
            }

            Ok(path)
        }
    }
}
