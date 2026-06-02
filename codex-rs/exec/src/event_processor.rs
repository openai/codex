#[cfg(unix)]
use std::io::Write;
use std::path::Path;

use codex_app_server_protocol::ServerNotification;
use codex_core::config::Config;
use codex_protocol::protocol::SessionConfiguredEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexStatus {
    Running,
    InitiateShutdown,
}

pub(crate) trait EventProcessor {
    /// Print summary of effective configuration and user prompt.
    fn print_config_summary(
        &mut self,
        config: &Config,
        prompt: &str,
        session_configured: &SessionConfiguredEvent,
    );

    /// Handle a single typed app-server notification emitted by the agent.
    fn process_server_notification(&mut self, notification: ServerNotification) -> CodexStatus;

    /// Handle a local exec warning that is not represented as an app-server notification.
    fn process_warning(&mut self, message: String) -> CodexStatus;

    fn print_final_output(&mut self) {}
}

pub(crate) fn handle_last_message(last_agent_message: Option<&str>, output_file: &Path) {
    let message = last_agent_message.unwrap_or_default();
    write_last_message_file(message, Some(output_file));
    if last_agent_message.is_none() {
        eprintln!(
            "Warning: no last agent message; wrote empty content to {}",
            output_file.display()
        );
    }
}

fn write_last_message_file(contents: &str, last_message_path: Option<&Path>) {
    if let Some(path) = last_message_path
        && let Err(e) = write_last_message_to_path(path, contents)
    {
        eprintln!("Failed to write last message file {path:?}: {e}");
    }
}

#[cfg(unix)]
fn write_last_message_to_path(path: &Path, contents: &str) -> std::io::Result<()> {
    let mut file = codex_utils_path::open_file_for_write_no_follow(path)?;
    file.write_all(contents.as_bytes())
}

#[cfg(not(unix))]
fn write_last_message_to_path(path: &Path, contents: &str) -> std::io::Result<()> {
    std::fs::write(path, contents)
}

#[cfg(test)]
#[path = "event_processor_tests.rs"]
mod tests;
