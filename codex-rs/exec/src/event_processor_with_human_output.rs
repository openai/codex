use std::io::IsTerminal;
use std::path::PathBuf;

use codex_app_server_protocol::CommandExecutionStatus;
use codex_app_server_protocol::McpToolCallStatus;
use codex_app_server_protocol::PatchApplyStatus;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadTokenUsage;
use codex_app_server_protocol::TurnStatus;
use codex_core::config::Config;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionConfiguredEvent;
use owo_colors::OwoColorize;
use owo_colors::Style;

use crate::event_processor::CodexStatus;
use crate::event_processor::EventProcessor;
use crate::event_processor::handle_last_message;

pub(crate) struct EventProcessorWithHumanOutput {
    bold: Style,
    cyan: Style,
    dimmed: Style,
    green: Style,
    red: Style,
    yellow: Style,
    show_agent_reasoning: bool,
    show_raw_agent_reasoning: bool,
    last_message_path: Option<PathBuf>,
    final_message: Option<String>,
    final_message_rendered: bool,
    last_total_token_usage: Option<ThreadTokenUsage>,
}

impl EventProcessorWithHumanOutput {
    pub(crate) fn create_with_ansi(
        with_ansi: bool,
        _cursor_ansi: bool,
        config: &Config,
        last_message_path: Option<PathBuf>,
    ) -> Self {
        let style = |styled: Style, plain: Style| if with_ansi { styled } else { plain };
        Self {
            bold: style(Style::new().bold(), Style::new()),
            cyan: style(Style::new().cyan(), Style::new()),
            dimmed: style(Style::new().dimmed(), Style::new()),
            green: style(Style::new().green(), Style::new()),
            red: style(Style::new().red(), Style::new()),
            yellow: style(Style::new().yellow(), Style::new()),
            show_agent_reasoning: !config.hide_agent_reasoning,
            show_raw_agent_reasoning: config.show_raw_agent_reasoning,
            last_message_path,
            final_message: None,
            final_message_rendered: false,
            last_total_token_usage: None,
        }
    }

    fn print_usage(&self) {
        if let Some(usage) = &self.last_total_token_usage {
            eprintln!(
                "{} input={} cached={} output={}",
                "usage:".style(self.dimmed),
                usage.total.input_tokens,
                usage.total.cached_input_tokens,
                usage.total.output_tokens
            );
        }
    }

    fn render_item_started(&self, item: &ThreadItem) {
        match item {
            ThreadItem::CommandExecution { command, cwd, .. } => {
                eprintln!(
                    "{} {} {}",
                    "exec:".style(self.bold),
                    command.style(self.cyan),
                    format!("({})", cwd.display()).style(self.dimmed)
                );
            }
            ThreadItem::McpToolCall { server, tool, .. } => {
                eprintln!(
                    "{} {} {}",
                    "mcp:".style(self.bold),
                    format!("{server}/{tool}").style(self.cyan),
                    "started".style(self.dimmed)
                );
            }
            ThreadItem::WebSearch { query, .. } => {
                eprintln!("{} {}", "web search:".style(self.bold), query);
            }
            ThreadItem::FileChange { .. } => {
                eprintln!("{}", "apply patch".style(self.bold));
            }
            ThreadItem::CollabAgentToolCall { tool, .. } => {
                eprintln!("{} {:?}", "collab:".style(self.bold), tool);
            }
            _ => {}
        }
    }

    fn render_item_completed(&mut self, item: ThreadItem) {
        match item {
            ThreadItem::AgentMessage { text, .. } => {
                eprintln!("{}\n{}", "assistant".style(self.cyan), text);
                self.final_message = Some(text);
                self.final_message_rendered = true;
            }
            ThreadItem::Reasoning {
                summary, content, ..
            } => {
                if self.show_agent_reasoning
                    && let Some(text) =
                        reasoning_text(&summary, &content, self.show_raw_agent_reasoning)
                    && !text.trim().is_empty()
                {
                    eprintln!("{}", text.style(self.dimmed));
                }
            }
            ThreadItem::CommandExecution {
                command,
                aggregated_output,
                exit_code,
                status,
                ..
            } => {
                let status_text = match status {
                    CommandExecutionStatus::Completed => "completed".style(self.green),
                    CommandExecutionStatus::Failed => "failed".style(self.red),
                    CommandExecutionStatus::Declined => "declined".style(self.yellow),
                    CommandExecutionStatus::InProgress => "in_progress".style(self.dimmed),
                };
                eprintln!(
                    "{} {} {}",
                    "exec:".style(self.bold),
                    command.style(self.cyan),
                    format!("({status_text})").style(self.dimmed)
                );
                if let Some(exit_code) = exit_code {
                    eprintln!("{}", format!("exit code: {exit_code}").style(self.dimmed));
                }
                if let Some(output) = aggregated_output
                    && !output.trim().is_empty()
                {
                    eprintln!("{output}");
                }
            }
            ThreadItem::FileChange {
                changes, status, ..
            } => {
                let status_text = match status {
                    PatchApplyStatus::Completed => "completed",
                    PatchApplyStatus::Failed => "failed",
                    PatchApplyStatus::Declined => "declined",
                    PatchApplyStatus::InProgress => "in_progress",
                };
                eprintln!("{} {}", "patch:".style(self.bold), status_text);
                for change in changes {
                    eprintln!("{}", change.path.style(self.dimmed));
                }
            }
            ThreadItem::McpToolCall {
                server,
                tool,
                status,
                error,
                ..
            } => {
                let status_text = match status {
                    McpToolCallStatus::Completed => "completed".style(self.green),
                    McpToolCallStatus::Failed => "failed".style(self.red),
                    McpToolCallStatus::InProgress => "in_progress".style(self.dimmed),
                };
                eprintln!(
                    "{} {} {}",
                    "mcp:".style(self.bold),
                    format!("{server}/{tool}").style(self.cyan),
                    format!("({status_text})").style(self.dimmed)
                );
                if let Some(error) = error {
                    eprintln!("{}", error.message.style(self.red));
                }
            }
            ThreadItem::WebSearch { query, .. } => {
                eprintln!("{} {}", "web search:".style(self.bold), query);
            }
            ThreadItem::ContextCompaction { .. } => {
                eprintln!("{}", "context compacted".style(self.dimmed));
            }
            _ => {}
        }
    }

    fn sandbox_label(config: &Config) -> &'static str {
        match config.permissions.sandbox_policy.get() {
            SandboxPolicy::DangerFullAccess => "danger-full-access",
            SandboxPolicy::ReadOnly { .. } => "read-only",
            SandboxPolicy::ExternalSandbox { .. } => "external-sandbox",
            SandboxPolicy::WorkspaceWrite { .. } => "workspace-write",
        }
    }
}

impl EventProcessor for EventProcessorWithHumanOutput {
    fn print_config_summary(
        &mut self,
        config: &Config,
        prompt: &str,
        session_configured_event: &SessionConfiguredEvent,
    ) {
        const VERSION: &str = env!("CARGO_PKG_VERSION");
        eprintln!("OpenAI Codex v{VERSION} (research preview)\n--------");
        eprintln!(
            "{} {}",
            "model:".style(self.bold),
            session_configured_event.model
        );
        eprintln!(
            "{} {}",
            "provider:".style(self.bold),
            session_configured_event.model_provider_id
        );
        eprintln!(
            "{} {}",
            "sandbox:".style(self.bold),
            Self::sandbox_label(config)
        );
        eprintln!(
            "{} {}",
            "session id:".style(self.bold),
            session_configured_event.session_id
        );
        eprintln!("--------");
        eprintln!("{}\n{}", "user".style(self.cyan), prompt);
    }

    fn process_server_notification(&mut self, notification: ServerNotification) -> CodexStatus {
        match notification {
            ServerNotification::ConfigWarning(notification) => {
                let details = notification
                    .details
                    .map(|details| format!(" ({details})"))
                    .unwrap_or_default();
                eprintln!(
                    "{} {}{}",
                    "warning:".style(self.yellow).style(self.bold),
                    notification.summary,
                    details
                );
                CodexStatus::Running
            }
            ServerNotification::Error(notification) => {
                eprintln!(
                    "{} {}",
                    "ERROR:".style(self.red).style(self.bold),
                    notification.error
                );
                CodexStatus::Running
            }
            ServerNotification::DeprecationNotice(notification) => {
                eprintln!(
                    "{} {}",
                    "deprecated:".style(self.yellow).style(self.bold),
                    notification.summary
                );
                if let Some(details) = notification.details {
                    eprintln!("{}", details.style(self.dimmed));
                }
                CodexStatus::Running
            }
            ServerNotification::HookStarted(notification) => {
                eprintln!(
                    "{} {}",
                    "hook:".style(self.bold),
                    format!("{:?}", notification.run.event_name).style(self.dimmed)
                );
                CodexStatus::Running
            }
            ServerNotification::HookCompleted(notification) => {
                eprintln!(
                    "{} {} {:?}",
                    "hook:".style(self.bold),
                    format!("{:?}", notification.run.event_name).style(self.dimmed),
                    notification.run.status
                );
                CodexStatus::Running
            }
            ServerNotification::ItemStarted(notification) => {
                self.render_item_started(&notification.item);
                CodexStatus::Running
            }
            ServerNotification::ItemCompleted(notification) => {
                self.render_item_completed(notification.item);
                CodexStatus::Running
            }
            ServerNotification::ModelRerouted(notification) => {
                eprintln!(
                    "{} {} -> {}",
                    "model rerouted:".style(self.yellow).style(self.bold),
                    notification.from_model,
                    notification.to_model
                );
                CodexStatus::Running
            }
            ServerNotification::ThreadTokenUsageUpdated(notification) => {
                self.last_total_token_usage = Some(notification.token_usage);
                CodexStatus::Running
            }
            ServerNotification::TurnCompleted(notification) => match notification.turn.status {
                TurnStatus::Completed => {
                    let rendered_message = self
                        .final_message_rendered
                        .then(|| self.final_message.clone())
                        .flatten();
                    if let Some(final_message) =
                        final_message_from_turn_items(notification.turn.items.as_slice())
                    {
                        self.final_message_rendered =
                            rendered_message.as_deref() == Some(final_message.as_str());
                        self.final_message = Some(final_message);
                    }
                    self.print_usage();
                    CodexStatus::InitiateShutdown
                }
                TurnStatus::Failed => {
                    if let Some(error) = notification.turn.error {
                        eprintln!("{} {}", "ERROR:".style(self.red).style(self.bold), error);
                    }
                    CodexStatus::InitiateShutdown
                }
                TurnStatus::Interrupted => {
                    eprintln!("{}", "turn interrupted".style(self.dimmed));
                    CodexStatus::InitiateShutdown
                }
                TurnStatus::InProgress => CodexStatus::Running,
            },
            ServerNotification::TurnDiffUpdated(notification) => {
                if !notification.diff.trim().is_empty() {
                    eprintln!("{}", notification.diff);
                }
                CodexStatus::Running
            }
            ServerNotification::TurnPlanUpdated(notification) => {
                if let Some(explanation) = notification.explanation {
                    eprintln!("{}", explanation.style(self.dimmed));
                }
                for step in notification.plan {
                    eprintln!("- {:?} {}", step.status, step.step);
                }
                CodexStatus::Running
            }
            ServerNotification::TurnStarted(_) => CodexStatus::Running,
            _ => CodexStatus::Running,
        }
    }

    fn process_warning(&mut self, message: String) -> CodexStatus {
        eprintln!(
            "{} {message}",
            "warning:".style(self.yellow).style(self.bold)
        );
        CodexStatus::Running
    }

    fn print_final_output(&mut self) {
        if let Some(path) = self.last_message_path.as_deref() {
            handle_last_message(self.final_message.as_deref(), path);
        }

        #[allow(clippy::print_stdout)]
        if should_print_final_message_to_stdout(
            self.final_message.as_deref(),
            std::io::stdout().is_terminal(),
            std::io::stderr().is_terminal(),
        ) && let Some(message) = self.final_message.as_deref()
        {
            println!("{message}");
        } else if should_print_final_message_to_tty(
            self.final_message.as_deref(),
            self.final_message_rendered,
            std::io::stdout().is_terminal(),
            std::io::stderr().is_terminal(),
        ) && let Some(message) = self.final_message.as_deref()
        {
            eprintln!("{}\n{}", "assistant".style(self.cyan), message);
        }
    }
}

fn reasoning_text(
    summary: &[String],
    content: &[String],
    show_raw_agent_reasoning: bool,
) -> Option<String> {
    let entries = if show_raw_agent_reasoning && !content.is_empty() {
        content
    } else {
        summary
    };
    if entries.is_empty() {
        None
    } else {
        Some(entries.join("\n"))
    }
}

fn final_message_from_turn_items(items: &[ThreadItem]) -> Option<String> {
    items
        .iter()
        .rev()
        .find_map(|item| match item {
            ThreadItem::AgentMessage { text, .. } => Some(text.clone()),
            _ => None,
        })
        .or_else(|| {
            items.iter().rev().find_map(|item| match item {
                ThreadItem::Plan { text, .. } => Some(text.clone()),
                _ => None,
            })
        })
}

fn should_print_final_message_to_stdout(
    final_message: Option<&str>,
    stdout_is_terminal: bool,
    stderr_is_terminal: bool,
) -> bool {
    final_message.is_some() && !(stdout_is_terminal && stderr_is_terminal)
}

fn should_print_final_message_to_tty(
    final_message: Option<&str>,
    final_message_rendered: bool,
    stdout_is_terminal: bool,
    stderr_is_terminal: bool,
) -> bool {
    final_message.is_some() && !final_message_rendered && stdout_is_terminal && stderr_is_terminal
}

#[cfg(test)]
mod tests {
    use codex_app_server_protocol::ThreadItem;
    use codex_app_server_protocol::Turn;
    use codex_app_server_protocol::TurnStatus;
    use owo_colors::Style;

    use super::EventProcessorWithHumanOutput;
    use super::final_message_from_turn_items;
    use super::reasoning_text;
    use super::should_print_final_message_to_stdout;
    use super::should_print_final_message_to_tty;
    use crate::event_processor::EventProcessor;
    use codex_app_server_protocol::ServerNotification;

    #[test]
    fn suppresses_final_stdout_message_when_both_streams_are_terminals() {
        assert!(!should_print_final_message_to_stdout(
            Some("hello"),
            true,
            true
        ));
    }

    #[test]
    fn prints_final_stdout_message_when_stdout_is_not_terminal() {
        assert!(should_print_final_message_to_stdout(
            Some("hello"),
            false,
            true
        ));
    }

    #[test]
    fn prints_final_stdout_message_when_stderr_is_not_terminal() {
        assert!(should_print_final_message_to_stdout(
            Some("hello"),
            true,
            false
        ));
    }

    #[test]
    fn suppresses_final_stdout_message_when_missing() {
        assert!(!should_print_final_message_to_stdout(None, false, false));
    }

    #[test]
    fn prints_final_tty_message_when_not_yet_rendered() {
        assert!(should_print_final_message_to_tty(
            Some("hello"),
            false,
            true,
            true
        ));
    }

    #[test]
    fn suppresses_final_tty_message_when_already_rendered() {
        assert!(!should_print_final_message_to_tty(
            Some("hello"),
            true,
            true,
            true
        ));
    }

    #[test]
    fn reasoning_text_prefers_summary_when_raw_reasoning_is_hidden() {
        let text = reasoning_text(
            &["summary".to_string()],
            &["raw".to_string()],
            /*show_raw_agent_reasoning*/ false,
        );

        assert_eq!(text.as_deref(), Some("summary"));
    }

    #[test]
    fn reasoning_text_uses_raw_content_when_enabled() {
        let text = reasoning_text(
            &["summary".to_string()],
            &["raw".to_string()],
            /*show_raw_agent_reasoning*/ true,
        );

        assert_eq!(text.as_deref(), Some("raw"));
    }

    #[test]
    fn final_message_from_turn_items_uses_latest_agent_message() {
        let message = final_message_from_turn_items(&[
            ThreadItem::AgentMessage {
                id: "msg-1".to_string(),
                text: "first".to_string(),
                phase: None,
                memory_citation: None,
            },
            ThreadItem::Plan {
                id: "plan-1".to_string(),
                text: "plan".to_string(),
            },
            ThreadItem::AgentMessage {
                id: "msg-2".to_string(),
                text: "second".to_string(),
                phase: None,
                memory_citation: None,
            },
        ]);

        assert_eq!(message.as_deref(), Some("second"));
    }

    #[test]
    fn final_message_from_turn_items_falls_back_to_latest_plan() {
        let message = final_message_from_turn_items(&[
            ThreadItem::Reasoning {
                id: "reasoning-1".to_string(),
                summary: vec!["inspect".to_string()],
                content: Vec::new(),
            },
            ThreadItem::Plan {
                id: "plan-1".to_string(),
                text: "first plan".to_string(),
            },
            ThreadItem::Plan {
                id: "plan-2".to_string(),
                text: "final plan".to_string(),
            },
        ]);

        assert_eq!(message.as_deref(), Some("final plan"));
    }

    #[test]
    fn turn_completed_recovers_final_message_from_turn_items() {
        let mut processor = EventProcessorWithHumanOutput {
            bold: Style::new(),
            cyan: Style::new(),
            dimmed: Style::new(),
            green: Style::new(),
            red: Style::new(),
            yellow: Style::new(),
            show_agent_reasoning: true,
            show_raw_agent_reasoning: false,
            last_message_path: None,
            final_message: None,
            final_message_rendered: false,
            last_total_token_usage: None,
        };

        let status = processor.process_server_notification(ServerNotification::TurnCompleted(
            codex_app_server_protocol::TurnCompletedNotification {
                thread_id: "thread-1".to_string(),
                turn: Turn {
                    id: "turn-1".to_string(),
                    items: vec![ThreadItem::AgentMessage {
                        id: "msg-1".to_string(),
                        text: "final answer".to_string(),
                        phase: None,
                        memory_citation: None,
                    }],
                    status: TurnStatus::Completed,
                    error: None,
                },
            },
        ));

        assert_eq!(
            status,
            crate::event_processor::CodexStatus::InitiateShutdown
        );
        assert_eq!(processor.final_message.as_deref(), Some("final answer"));
    }

    #[test]
    fn turn_completed_overwrites_stale_final_message_from_turn_items() {
        let mut processor = EventProcessorWithHumanOutput {
            bold: Style::new(),
            cyan: Style::new(),
            dimmed: Style::new(),
            green: Style::new(),
            red: Style::new(),
            yellow: Style::new(),
            show_agent_reasoning: true,
            show_raw_agent_reasoning: false,
            last_message_path: None,
            final_message: Some("stale answer".to_string()),
            final_message_rendered: true,
            last_total_token_usage: None,
        };

        let status = processor.process_server_notification(ServerNotification::TurnCompleted(
            codex_app_server_protocol::TurnCompletedNotification {
                thread_id: "thread-1".to_string(),
                turn: Turn {
                    id: "turn-1".to_string(),
                    items: vec![ThreadItem::AgentMessage {
                        id: "msg-1".to_string(),
                        text: "final answer".to_string(),
                        phase: None,
                        memory_citation: None,
                    }],
                    status: TurnStatus::Completed,
                    error: None,
                },
            },
        ));

        assert_eq!(
            status,
            crate::event_processor::CodexStatus::InitiateShutdown
        );
        assert_eq!(processor.final_message.as_deref(), Some("final answer"));
        assert!(!processor.final_message_rendered);
    }

    #[test]
    fn turn_completed_preserves_streamed_final_message_when_turn_items_are_empty() {
        let mut processor = EventProcessorWithHumanOutput {
            bold: Style::new(),
            cyan: Style::new(),
            dimmed: Style::new(),
            green: Style::new(),
            red: Style::new(),
            yellow: Style::new(),
            show_agent_reasoning: true,
            show_raw_agent_reasoning: false,
            last_message_path: None,
            final_message: Some("streamed answer".to_string()),
            final_message_rendered: false,
            last_total_token_usage: None,
        };

        let status = processor.process_server_notification(ServerNotification::TurnCompleted(
            codex_app_server_protocol::TurnCompletedNotification {
                thread_id: "thread-1".to_string(),
                turn: Turn {
                    id: "turn-1".to_string(),
                    items: Vec::new(),
                    status: TurnStatus::Completed,
                    error: None,
                },
            },
        ));

        assert_eq!(
            status,
            crate::event_processor::CodexStatus::InitiateShutdown
        );
        assert_eq!(processor.final_message.as_deref(), Some("streamed answer"));
    }
}
