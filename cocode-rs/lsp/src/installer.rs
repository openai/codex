//! LSP server installer module
//!
//! Provides functionality to install built-in LSP servers with progress streaming.
//! Supports rustup (Rust), go install (Go), and npm (Node) installation methods.

use crate::config::BuiltinServer;
use crate::config::command_exists;
use crate::error::LspErr;
use crate::error::Result;
use std::process::Stdio;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Command;
use tokio::sync::mpsc;
use tracing::debug;
use tracing::info;
use tracing::warn;

/// Installation method detected from install_hint
///
/// Maps to current BUILTIN_SERVERS templates:
///   - rust-analyzer → Rustup ("rustup component add rust-analyzer")
///   - gopls → Go ("go install golang.org/x/tools/gopls@latest")
///   - pyright → Npm ("npm install -g pyright")
///   - typescript-language-server → Npm ("npm install -g typescript-language-server typescript")
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallerType {
    /// rustup component add ...
    Rustup,
    /// go install ...
    Go,
    /// npm install -g ...
    Npm,
    /// Execute as shell command (for custom servers)
    Unknown,
}

impl std::fmt::Display for InstallerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallerType::Rustup => write!(f, "rustup"),
            InstallerType::Go => write!(f, "go"),
            InstallerType::Npm => write!(f, "npm"),
            InstallerType::Unknown => write!(f, "shell"),
        }
    }
}

/// Progress events during installation
#[derive(Debug, Clone)]
pub enum InstallEvent {
    /// Installation started
    Started {
        server_id: String,
        method: InstallerType,
    },
    /// Output line from command (stdout or stderr)
    Output(String),
    /// Installation completed successfully
    Completed { server_id: String },
    /// Installation failed
    Failed { server_id: String, error: String },
}

/// Main installer struct
pub struct LspInstaller {
    /// Progress event sender
    progress_tx: Option<mpsc::Sender<InstallEvent>>,
}

impl LspInstaller {
    /// Create new installer with optional progress channel
    pub fn new(progress_tx: Option<mpsc::Sender<InstallEvent>>) -> Self {
        Self { progress_tx }
    }

    /// Check if a server is installed (using `which`)
    pub async fn is_installed(server_id: &str) -> bool {
        // Find the builtin server to get its command
        let Some(builtin) = BuiltinServer::find_by_id(server_id) else {
            return false;
        };

        // Get the command (first word of the first command)
        let command = builtin
            .commands
            .first()
            .and_then(|c| c.split_whitespace().next())
            .unwrap_or("");

        if command.is_empty() {
            return false;
        }

        command_exists(command).await
    }

    /// Parse install_hint to determine installer type
    pub fn parse_installer_type(install_hint: &str) -> InstallerType {
        let hint = install_hint.to_lowercase();
        if hint.starts_with("rustup ") {
            InstallerType::Rustup
        } else if hint.starts_with("go install ") {
            InstallerType::Go
        } else if hint.starts_with("npm install ") || hint.starts_with("npm i ") {
            InstallerType::Npm
        } else {
            InstallerType::Unknown
        }
    }

    /// Install a server by ID (looks up from BUILTIN_SERVERS)
    ///
    /// This only installs the binary - it does NOT modify any config files.
    /// Use `LspServersConfig::add_server_to_file()` separately to add to config.
    pub async fn install_server(&self, server_id: &str) -> Result<()> {
        let builtin = BuiltinServer::find_by_id(server_id).ok_or_else(|| {
            LspErr::InstallError(format!(
                "Server '{server_id}' not found in built-in servers"
            ))
        })?;

        self.install_with_hint(server_id, builtin.install_hint)
            .await
    }

    /// Install with explicit install_hint (for custom servers)
    ///
    /// This only installs the binary - it does NOT modify any config files.
    pub async fn install_with_hint(&self, server_id: &str, install_hint: &str) -> Result<()> {
        let method = Self::parse_installer_type(install_hint);

        info!(
            server = server_id,
            install_hint = install_hint,
            method = %method,
            "Starting LSP server installation"
        );

        // Send started event
        self.send_event(InstallEvent::Started {
            server_id: server_id.to_string(),
            method,
        })
        .await;

        // Execute the installation
        let result = match method {
            InstallerType::Rustup => self.install_rustup(install_hint).await,
            InstallerType::Go => self.install_go(install_hint).await,
            InstallerType::Npm => self.install_npm(install_hint).await,
            InstallerType::Unknown => self.install_shell(install_hint).await,
        };

        match result {
            Ok(()) => {
                info!(server = server_id, "LSP server installation completed");
                self.send_event(InstallEvent::Completed {
                    server_id: server_id.to_string(),
                })
                .await;
                Ok(())
            }
            Err(e) => {
                warn!(server = server_id, error = %e, "LSP server installation failed");
                self.send_event(InstallEvent::Failed {
                    server_id: server_id.to_string(),
                    error: e.to_string(),
                })
                .await;
                Err(e)
            }
        }
    }

    /// Install via rustup
    async fn install_rustup(&self, install_hint: &str) -> Result<()> {
        // Parse: "rustup component add rust-analyzer"
        let parts: Vec<&str> = install_hint.split_whitespace().collect();
        if parts.len() < 4 || parts[0] != "rustup" || parts[1] != "component" || parts[2] != "add" {
            return Err(LspErr::InstallError(format!(
                "Invalid rustup install hint: {install_hint}"
            )));
        }

        let component = parts[3];
        self.execute_command("rustup", &["component", "add", component])
            .await
    }

    /// Install via go install
    async fn install_go(&self, install_hint: &str) -> Result<()> {
        // Parse: "go install golang.org/x/tools/gopls@latest"
        let parts: Vec<&str> = install_hint.split_whitespace().collect();
        if parts.len() < 3 || parts[0] != "go" || parts[1] != "install" {
            return Err(LspErr::InstallError(format!(
                "Invalid go install hint: {install_hint}"
            )));
        }

        let package = parts[2];
        self.execute_command("go", &["install", package]).await
    }

    /// Install via npm
    async fn install_npm(&self, install_hint: &str) -> Result<()> {
        // Parse: "npm install -g typescript-language-server typescript"
        let parts: Vec<&str> = install_hint.split_whitespace().collect();
        if parts.len() < 4 || parts[0] != "npm" {
            return Err(LspErr::InstallError(format!(
                "Invalid npm install hint: {install_hint}"
            )));
        }

        // Reconstruct args (skip "npm")
        let args: Vec<&str> = parts[1..].to_vec();
        self.execute_command("npm", &args).await
    }

    /// Install via shell command
    async fn install_shell(&self, install_hint: &str) -> Result<()> {
        self.execute_command("sh", &["-c", install_hint]).await
    }

    /// Execute a command with streaming output
    async fn execute_command(&self, cmd: &str, args: &[&str]) -> Result<()> {
        debug!(cmd = cmd, args = ?args, "Executing install command");

        self.send_event(InstallEvent::Output(format!(
            "$ {} {}",
            cmd,
            args.join(" ")
        )))
        .await;

        let mut child = Command::new(cmd)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // Stream stdout
        if let Some(stdout) = child.stdout.take() {
            let tx = self.progress_tx.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if let Some(ref tx) = tx {
                        let _ = tx.send(InstallEvent::Output(line)).await;
                    }
                }
            });
        }

        // Stream stderr
        if let Some(stderr) = child.stderr.take() {
            let tx = self.progress_tx.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if let Some(ref tx) = tx {
                        let _ = tx.send(InstallEvent::Output(line)).await;
                    }
                }
            });
        }

        let status = child.wait().await?;

        if !status.success() {
            return Err(LspErr::InstallError(format!(
                "Command failed with exit code: {:?}",
                status.code()
            )));
        }

        Ok(())
    }

    /// Send an event to the progress channel
    async fn send_event(&self, event: InstallEvent) {
        if let Some(ref tx) = self.progress_tx {
            let _ = tx.send(event).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_installer_type_rustup() {
        assert_eq!(
            LspInstaller::parse_installer_type("rustup component add rust-analyzer"),
            InstallerType::Rustup
        );
    }

    #[test]
    fn test_parse_installer_type_go() {
        assert_eq!(
            LspInstaller::parse_installer_type("go install golang.org/x/tools/gopls@latest"),
            InstallerType::Go
        );
    }

    #[test]
    fn test_parse_installer_type_npm() {
        assert_eq!(
            LspInstaller::parse_installer_type("npm install -g pyright"),
            InstallerType::Npm
        );
        assert_eq!(
            LspInstaller::parse_installer_type(
                "npm install -g typescript-language-server typescript"
            ),
            InstallerType::Npm
        );
    }

    #[test]
    fn test_parse_installer_type_unknown() {
        assert_eq!(
            LspInstaller::parse_installer_type("brew install something"),
            InstallerType::Unknown
        );
        assert_eq!(
            LspInstaller::parse_installer_type("apt-get install lsp"),
            InstallerType::Unknown
        );
    }

    #[test]
    fn test_installer_type_display() {
        assert_eq!(format!("{}", InstallerType::Rustup), "rustup");
        assert_eq!(format!("{}", InstallerType::Go), "go");
        assert_eq!(format!("{}", InstallerType::Npm), "npm");
        assert_eq!(format!("{}", InstallerType::Unknown), "shell");
    }

    #[tokio::test]
    async fn test_is_installed_unknown_server() {
        // Unknown server should return false
        assert!(!LspInstaller::is_installed("unknown-server").await);
    }
}
