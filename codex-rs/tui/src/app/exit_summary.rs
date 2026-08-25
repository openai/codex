//! Exit output distinguishes a disconnected client from a stopped embedded server.

use super::*;
use crate::RemoteAppServerEndpoint;
use crate::exec_command::escape_command;
use crate::status::remote_connection::sanitized_websocket_url;

/// Reconnection and stop guidance for a task owned by a persistent app server.
#[derive(Debug, Clone)]
pub struct DisconnectInfo {
    /// CLI arguments identifying the server, without credential values.
    pub command: Vec<String>,
    pub stop_hint: String,
}

impl App {
    pub(super) fn exit_info(&self, exit_reason: ExitReason) -> AppExitInfo {
        let thread_id = match exit_reason {
            ExitReason::ThreadRemoved => None,
            ExitReason::UserRequested | ExitReason::TurnInterrupted | ExitReason::Fatal(_) => {
                self.chat_widget.thread_id().or(self.primary_thread_id)
            }
        };
        let disconnect_info = thread_id.and_then(|_| {
            let command = match &self.app_server_target {
                AppServerTarget::Embedded => return None,
                AppServerTarget::LocalDaemon { .. } => vec!["codex".to_string()],
                AppServerTarget::Remote { endpoint } => {
                    let address = match endpoint {
                        RemoteAppServerEndpoint::WebSocket { websocket_url, .. } => {
                            sanitized_websocket_url(websocket_url)
                                .and_then(|url| {
                                    let host = url.host()?;
                                    let port = url.port_or_known_default()?;
                                    // The CLI requires an explicit port, including 80/443.
                                    Some(format!("{}://{host}:{port}{}", url.scheme(), url.path()))
                                })
                                .unwrap_or_else(|| "<server-address>".to_string())
                        }
                        RemoteAppServerEndpoint::UnixSocket { socket_path } => {
                            format!("unix://{}", socket_path.display())
                        }
                    };
                    vec!["codex".to_string(), "--remote".to_string(), address]
                }
            };
            let stop_hint = self
                .keymap
                .agents
                .primary_hint("stop", &self.keymap.agents.stop)
                .map_or_else(
                    || "use the configured stop shortcut".to_string(),
                    |key| format!("press {}", key.display_label()),
                );
            Some(DisconnectInfo { command, stop_hint })
        });
        AppExitInfo {
            token_usage: self.token_usage(),
            thread_id,
            resume_hint: resume_hint_for_resumable_thread(
                thread_id,
                self.chat_widget.thread_name(),
                self.chat_widget.rollout_path().as_deref(),
            ),
            disconnect_info,
            update_action: self.pending_update_action,
            exit_reason,
        }
    }
}

impl AppExitInfo {
    /// Format the terminal summary for both the CLI and standalone TUI binaries.
    pub fn format_exit_messages(self, color_enabled: bool) -> Vec<String> {
        let color_command = |command: String| {
            if color_enabled {
                format!("\u{1b}[36m{command}\u{1b}[39m")
            } else {
                command
            }
        };
        let mut lines = Vec::new();
        if let Some(disconnect) = self.disconnect_info
            && let Some(thread_id) = self.thread_id
        {
            let turn_interrupted = matches!(self.exit_reason, ExitReason::TurnInterrupted);
            let message = match self.exit_reason {
                ExitReason::UserRequested | ExitReason::ThreadRemoved => {
                    "Disconnected from this task. Any running work continues."
                }
                ExitReason::Fatal(_) => "Disconnected from this task. Work may still be running.",
                ExitReason::TurnInterrupted => {
                    "Disconnected from this task. The current turn was stopped."
                }
            };
            lines.push(message.to_string());
            let mut resume_command = disconnect.command.clone();
            resume_command.extend(["resume".to_string(), thread_id.to_string()]);
            lines.push(format!(
                "Reconnect: {}",
                color_command(escape_command(&resume_command)),
            ));
            if !turn_interrupted {
                let mut agents_command = disconnect.command;
                agents_command.push("agents".to_string());
                lines.push(format!(
                    "Stop the current turn: run {}, select this task, and {}.",
                    color_command(escape_command(&agents_command)),
                    disconnect.stop_hint,
                ));
            }
            if !self.token_usage.is_zero() {
                let usage = self.token_usage.to_string();
                lines.push(if turn_interrupted {
                    usage
                } else {
                    usage.replacen("Token usage:", "Token usage so far:", /*count*/ 1)
                });
            }
            return lines;
        }

        if !self.token_usage.is_zero() {
            lines.push(self.token_usage.to_string());
        }
        if let Some(resume_command) = self.resume_hint {
            lines.push(format!(
                "To continue this session, run {}",
                color_command(resume_command),
            ));
        } else if matches!(self.exit_reason, ExitReason::Fatal(_))
            && let Some(thread_id) = self.thread_id
        {
            lines.push(format!("Session ID: {thread_id}"));
        }
        lines
    }
}
