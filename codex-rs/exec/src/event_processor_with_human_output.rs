use std::io::IsTerminal;
use std::path::Path;
use std::path::PathBuf;

use codex_app_server_protocol::CommandExecutionStatus;
use codex_app_server_protocol::McpToolCallStatus;
use codex_app_server_protocol::PatchApplyStatus;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadTokenUsage;
use codex_app_server_protocol::TurnStatus;
use codex_core::config::Config;
use codex_model_provider_info::WireApi;
use codex_protocol::models::PermissionProfile;
use codex_protocol::num_format::format_with_separators;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::SessionConfiguredEvent;
use codex_utils_absolute_path::canonicalize_preserving_symlinks;
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
    italic: Style,
    magenta: Style,
    red: Style,
    yellow: Style,
    show_agent_reasoning: bool,
    show_raw_agent_reasoning: bool,
    last_message_path: Option<PathBuf>,
    final_message: Option<String>,
    final_message_rendered: bool,
    emit_final_message_on_shutdown: bool,
    last_total_token_usage: Option<ThreadTokenUsage>,
}

impl EventProcessorWithHumanOutput {
    pub(crate) fn create_with_ansi(
        with_ansi: bool,
        config: &Config,
        last_message_path: Option<PathBuf>,
    ) -> Self {
        let style = |styled: Style, plain: Style| if with_ansi { styled } else { plain };
        Self {
            bold: style(Style::new().bold(), Style::new()),
            cyan: style(Style::new().cyan(), Style::new()),
            dimmed: style(Style::new().dimmed(), Style::new()),
            green: style(Style::new().green(), Style::new()),
            italic: style(Style::new().italic(), Style::new()),
            magenta: style(Style::new().magenta(), Style::new()),
            red: style(Style::new().red(), Style::new()),
            yellow: style(Style::new().yellow(), Style::new()),
            show_agent_reasoning: !config.hide_agent_reasoning,
            show_raw_agent_reasoning: config.show_raw_agent_reasoning,
            last_message_path,
            final_message: None,
            final_message_rendered: false,
            emit_final_message_on_shutdown: false,
            last_total_token_usage: None,
        }
    }

    fn render_item_started(&self, item: &ThreadItem) {
        match item {
            ThreadItem::CommandExecution { command, cwd, .. } => {
                eprintln!(
                    "{}\n{} in {}",
                    "exec".style(self.italic).style(self.magenta),
                    command.style(self.bold),
                    cwd.display()
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
                eprintln!(
                    "{}\n{}",
                    "codex".style(self.italic).style(self.magenta),
                    text
                );
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
                command: _,
                aggregated_output,
                exit_code,
                status,
                duration_ms,
                ..
            } => {
                let duration_suffix = duration_ms
                    .map(|duration_ms| format!(" in {duration_ms}ms"))
                    .unwrap_or_default();
                match status {
                    CommandExecutionStatus::Completed => {
                        eprintln!(
                            "{}",
                            format!(" succeeded{duration_suffix}:").style(self.green)
                        );
                    }
                    CommandExecutionStatus::Failed => {
                        let exit_code = exit_code.unwrap_or(1);
                        eprintln!(
                            "{}",
                            format!(" exited {exit_code}{duration_suffix}:").style(self.red)
                        );
                    }
                    CommandExecutionStatus::Declined => {
                        eprintln!(
                            "{}",
                            format!(" declined{duration_suffix}:").style(self.yellow)
                        );
                    }
                    CommandExecutionStatus::InProgress => {
                        eprintln!(
                            "{}",
                            format!(" in progress{duration_suffix}:").style(self.dimmed)
                        );
                    }
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
        for (key, value) in config_summary_entries(config, session_configured_event) {
            eprintln!("{} {}", format!("{key}:").style(self.bold), value);
        }
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
            ServerNotification::ModelVerification(_) => CodexStatus::Running,
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
                    self.emit_final_message_on_shutdown = true;
                    CodexStatus::InitiateShutdown
                }
                TurnStatus::Failed => {
                    self.final_message = None;
                    self.final_message_rendered = false;
                    self.emit_final_message_on_shutdown = false;
                    if let Some(error) = notification.turn.error {
                        eprintln!("{} {}", "ERROR:".style(self.red).style(self.bold), error);
                    }
                    CodexStatus::InitiateShutdown
                }
                TurnStatus::Interrupted => {
                    self.final_message = None;
                    self.final_message_rendered = false;
                    self.emit_final_message_on_shutdown = false;
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
                    eprintln!("{}", explanation.style(self.italic));
                }
                for step in notification.plan {
                    match step.status {
                        codex_app_server_protocol::TurnPlanStepStatus::Completed => {
                            eprintln!("  {} {}", "✓".style(self.green), step.step);
                        }
                        codex_app_server_protocol::TurnPlanStepStatus::InProgress => {
                            eprintln!("  {} {}", "→".style(self.cyan), step.step);
                        }
                        codex_app_server_protocol::TurnPlanStepStatus::Pending => {
                            eprintln!(
                                "  {} {}",
                                "•".style(self.dimmed),
                                step.step.style(self.dimmed)
                            );
                        }
                    }
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
        if self.emit_final_message_on_shutdown
            && let Some(path) = self.last_message_path.as_deref()
        {
            handle_last_message(self.final_message.as_deref(), path);
        }

        if let Some(usage) = &self.last_total_token_usage {
            eprintln!(
                "{}\n{}",
                "tokens used".style(self.dimmed),
                format_with_separators(blended_total(usage))
            );
        }

        #[allow(clippy::print_stdout)]
        if should_print_final_message_to_stdout(
            self.emit_final_message_on_shutdown
                .then_some(self.final_message.as_deref())
                .flatten(),
            std::io::stdout().is_terminal(),
            std::io::stderr().is_terminal(),
        ) && let Some(message) = self.final_message.as_deref()
        {
            println!("{message}");
        } else if should_print_final_message_to_tty(
            self.emit_final_message_on_shutdown
                .then_some(self.final_message.as_deref())
                .flatten(),
            self.final_message_rendered,
            std::io::stdout().is_terminal(),
            std::io::stderr().is_terminal(),
        ) && let Some(message) = self.final_message.as_deref()
        {
            eprintln!(
                "{}\n{}",
                "codex".style(self.italic).style(self.magenta),
                message
            );
        }
    }
}

fn config_summary_entries(
    config: &Config,
    session_configured_event: &SessionConfiguredEvent,
) -> Vec<(&'static str, String)> {
    let mut entries = vec![
        ("workdir", config.cwd.display().to_string()),
        ("model", session_configured_event.model.clone()),
        (
            "provider",
            session_configured_event.model_provider_id.clone(),
        ),
        (
            "approval",
            config.permissions.approval_policy.value().to_string(),
        ),
        (
            "sandbox",
            summarize_permission_profile(
                config.permissions.permission_profile.get(),
                config.cwd.as_path(),
            ),
        ),
    ];
    if config.model_provider.wire_api == WireApi::Responses {
        entries.push((
            "reasoning effort",
            config
                .model_reasoning_effort
                .map(|effort| effort.to_string())
                .unwrap_or_else(|| "none".to_string()),
        ));
        entries.push((
            "reasoning summaries",
            config
                .model_reasoning_summary
                .map(|summary| summary.to_string())
                .unwrap_or_else(|| "none".to_string()),
        ));
    }
    entries.push((
        "session id",
        session_configured_event.session_id.to_string(),
    ));
    entries
}

fn summarize_permission_profile(permission_profile: &PermissionProfile, cwd: &Path) -> String {
    match permission_profile {
        PermissionProfile::Disabled => "danger-full-access".to_string(),
        PermissionProfile::External { network } => {
            let mut summary = "external-sandbox".to_string();
            append_network_summary(&mut summary, *network);
            summary
        }
        PermissionProfile::Managed { .. } => {
            let file_system_policy = permission_profile.file_system_sandbox_policy();
            let network_policy = permission_profile.network_sandbox_policy();
            if file_system_policy.has_full_disk_write_access() {
                let mut summary = "workspace-write [/]".to_string();
                append_network_summary(&mut summary, network_policy);
                return summary;
            }

            let writable_roots = file_system_policy.get_writable_roots_with_cwd(cwd);
            if writable_roots.is_empty() {
                let mut summary = "read-only".to_string();
                append_network_summary(&mut summary, network_policy);
                return summary;
            }

            let mut summary = "workspace-write".to_string();
            let writable_entries = writable_roots
                .iter()
                .map(|root| writable_root_label(root.root.as_path(), cwd))
                .collect::<Vec<_>>();
            summary.push_str(&format!(" [{}]", writable_entries.join(", ")));
            append_network_summary(&mut summary, network_policy);
            summary
        }
    }
}

fn append_network_summary(summary: &mut String, network_policy: NetworkSandboxPolicy) {
    if network_policy.is_enabled() {
        summary.push_str(" (network access enabled)");
    }
}

fn writable_root_label(root: &Path, cwd: &Path) -> String {
    if paths_match_after_canonicalization(root, cwd) {
        return "workdir".to_string();
    }
    if paths_match_after_canonicalization(root, Path::new("/tmp")) {
        return "/tmp".to_string();
    }
    if std::env::var_os("TMPDIR")
        .filter(|tmpdir| !tmpdir.is_empty())
        .is_some_and(|tmpdir| paths_match_after_canonicalization(root, Path::new(&tmpdir)))
    {
        return "$TMPDIR".to_string();
    }
    display_path_label(root)
}

fn paths_match_after_canonicalization(left: &Path, right: &Path) -> bool {
    canonicalize_preserving_symlinks(left).ok() == canonicalize_preserving_symlinks(right).ok()
        || display_path_label(left) == display_path_label(right)
}

fn display_path_label(path: &Path) -> String {
    path.strip_prefix("/private/tmp")
        .ok()
        .map(|suffix| Path::new("/tmp").join(suffix))
        .unwrap_or_else(|| path.to_path_buf())
        .to_string_lossy()
        .to_string()
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

fn blended_total(usage: &ThreadTokenUsage) -> i64 {
    let cached_input = usage.total.cached_input_tokens.max(0);
    let non_cached_input = (usage.total.input_tokens - cached_input).max(0);
    (non_cached_input + usage.total.output_tokens.max(0)).max(0)
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
#[path = "event_processor_with_human_output_tests.rs"]
mod tests;
