#[cfg(unix)]
use std::fs::OpenOptions;
#[cfg(unix)]
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
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
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    file.write_all(contents.as_bytes())
}

#[cfg(not(unix))]
fn write_last_message_to_path(path: &Path, contents: &str) -> std::io::Result<()> {
    std::fs::write(path, contents)
}

#[cfg(test)]
mod tests {
    use super::write_last_message_to_path;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    #[test]
    fn writes_last_message_to_regular_file() {
        let temp_dir = tempdir().expect("tempdir");
        let output_path = temp_dir.path().join("output.md");

        write_last_message_to_path(&output_path, "hello").expect("write output");

        assert_eq!(
            std::fs::read_to_string(&output_path).expect("read output"),
            "hello"
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_last_message_path() {
        let temp_dir = tempdir().expect("tempdir");
        let target_path = temp_dir.path().join("target.md");
        let output_path = temp_dir.path().join("output.md");
        std::fs::write(&target_path, "original").expect("write target");
        std::os::unix::fs::symlink(&target_path, &output_path).expect("create symlink");

        let err =
            write_last_message_to_path(&output_path, "hello").expect_err("symlink should fail");

        assert_eq!(err.raw_os_error(), Some(libc::ELOOP));
        assert_eq!(
            std::fs::read_to_string(&target_path).expect("read target"),
            "original"
        );
    }
}
