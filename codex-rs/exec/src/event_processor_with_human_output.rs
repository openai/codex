use codex_common::elapsed::format_duration;
use codex_common::elapsed::format_elapsed;
use codex_core::config::Config;
use codex_core::protocol::AgentMessageEvent;
use codex_core::protocol::AgentReasoningRawContentEvent;
use codex_core::protocol::BackgroundEventEvent;
use codex_core::protocol::DeprecationNoticeEvent;
use codex_core::protocol::ErrorEvent;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ExecCommandBeginEvent;
use codex_core::protocol::ExecCommandEndEvent;
use codex_core::protocol::FileChange;
use codex_core::protocol::McpInvocation;
use codex_core::protocol::McpToolCallBeginEvent;
use codex_core::protocol::McpToolCallEndEvent;
use codex_core::protocol::PatchApplyBeginEvent;
use codex_core::protocol::PatchApplyEndEvent;
use codex_core::protocol::SessionConfiguredEvent;
use codex_core::protocol::StreamErrorEvent;
use codex_core::protocol::TaskCompleteEvent;
use codex_core::protocol::TurnAbortReason;
use codex_core::protocol::TurnDiffEvent;
use codex_core::protocol::WarningEvent;
use codex_core::protocol::WebSearchEndEvent;
use codex_protocol::num_format::format_with_separators;
use owo_colors::OwoColorize;
use owo_colors::Style;
use shlex::try_join;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use crate::event_processor::CodexStatus;
use crate::event_processor::EventProcessor;
use crate::event_processor::handle_last_message;
use codex_common::create_config_summary_entries;
use codex_protocol::plan_tool::StepStatus;
use codex_protocol::plan_tool::UpdatePlanArgs;

/// This should be configurable. When used in CI, users may not want to impose
/// a limit so they can see the full transcript.
const MAX_OUTPUT_LINES_FOR_EXEC_TOOL_CALL: usize = 20;
pub(crate) struct EventProcessorWithHumanOutput {
    call_id_to_patch: HashMap<String, PatchApplyBegin>,

    // To ensure that --color=never is respected, ANSI escapes _must_ be added
    // using .style() with one of these fields. If you need a new style, add a
    // new field here.
    bold: Style,
    italic: Style,
    dimmed: Style,

    magenta: Style,
    red: Style,
    green: Style,
    cyan: Style,
    yellow: Style,

    /// Whether to include `AgentReasoning` events in the output.
    show_agent_reasoning: bool,
    show_raw_agent_reasoning: bool,
    last_message_path: Option<PathBuf>,
    last_total_token_usage: Option<codex_core::protocol::TokenUsageInfo>,
    final_message: Option<String>,
}

impl EventProcessorWithHumanOutput {
    pub(crate) fn create_with_ansi(
        with_ansi: bool,
        config: &Config,
        last_message_path: Option<PathBuf>,
    ) -> Self {
        let call_id_to_patch = HashMap::new();

        if with_ansi {
            Self {
                call_id_to_patch,
                bold: Style::new().bold(),
                italic: Style::new().italic(),
                dimmed: Style::new().dimmed(),
                magenta: Style::new().magenta(),
                red: Style::new().red(),
                green: Style::new().green(),
                cyan: Style::new().cyan(),
                yellow: Style::new().yellow(),
                show_agent_reasoning: !config.hide_agent_reasoning,
                show_raw_agent_reasoning: config.show_raw_agent_reasoning,
                last_message_path,
                last_total_token_usage: None,
                final_message: None,
            }
        } else {
            Self {
                call_id_to_patch,
                bold: Style::new(),
                italic: Style::new(),
                dimmed: Style::new(),
                magenta: Style::new(),
                red: Style::new(),
                green: Style::new(),
                cyan: Style::new(),
                yellow: Style::new(),
                show_agent_reasoning: !config.hide_agent_reasoning,
                show_raw_agent_reasoning: config.show_raw_agent_reasoning,
                last_message_path,
                last_total_token_usage: None,
                final_message: None,
            }
        }
    }
}

struct PatchApplyBegin {
    start_time: Instant,
    auto_approved: bool,
}

/// Timestamped helper. The timestamp is styled with self.dimmed.
macro_rules! ts_msg {
    ($self:ident, $($arg:tt)*) => {{
        eprintln!($($arg)*);
    }};
}

impl EventProcessor for EventProcessorWithHumanOutput {
    /// Print a concise summary of the effective configuration that will be used
    /// for the session. This mirrors the information shown in the TUI welcome
    /// screen.
    fn print_config_summary(
        &mut self,
        config: &Config,
        prompt: &str,
        session_configured_event: &SessionConfiguredEvent,
    ) {
        const VERSION: &str = env!("CARGO_PKG_VERSION");
        ts_msg!(
            self,
            "OpenAI Codex v{} (research preview)\n--------",
            VERSION
        );

        let mut entries =
            create_config_summary_entries(config, session_configured_event.model.as_str());
        entries.push((
            "session id",
            session_configured_event.session_id.to_string(),
        ));

        for (key, value) in entries {
            eprintln!("{} {}", format!("{key}:").style(self.bold), value);
        }

        eprintln!("--------");

        // Echo the prompt that will be sent to the agent so it is visible in the
        // transcript/logs before any events come in. Note the prompt may have been
        // read from stdin, so it may not be visible in the terminal otherwise.
        ts_msg!(self, "{}\n{}", "user".style(self.cyan), prompt);
    }

    fn process_event(&mut self, event: Event) -> CodexStatus {
        let Event { id: _, msg } = event;
        match msg {
            EventMsg::Error(ErrorEvent { message, .. }) => {
                let prefix = "ERROR:".style(self.red);
                ts_msg!(self, "{prefix} {message}");
            }
            EventMsg::Warning(WarningEvent { message }) => {
                ts_msg!(
                    self,
                    "{} {message}",
                    "warning:".style(self.yellow).style(self.bold)
                );
            }
            EventMsg::DeprecationNotice(DeprecationNoticeEvent { summary, details }) => {
                ts_msg!(
                    self,
                    "{} {summary}",
                    "deprecated:".style(self.magenta).style(self.bold)
                );
                if let Some(details) = details {
                    ts_msg!(self, "  {}", details.style(self.dimmed));
                }
            }
            EventMsg::McpStartupUpdate(update) => {
                let status_text = match update.status {
                    codex_core::protocol::McpStartupStatus::Starting => "starting".to_string(),
                    codex_core::protocol::McpStartupStatus::Ready => "ready".to_string(),
                    codex_core::protocol::McpStartupStatus::Cancelled => "cancelled".to_string(),
                    codex_core::protocol::McpStartupStatus::Failed { ref error } => {
                        format!("failed: {error}")
                    }
                };
                ts_msg!(
                    self,
                    "{} {} {}",
                    "mcp:".style(self.cyan),
                    update.server,
                    status_text
                );
            }
            EventMsg::McpStartupComplete(summary) => {
                let mut parts = Vec::new();
                if !summary.ready.is_empty() {
                    parts.push(format!("ready: {}", summary.ready.join(", ")));
                }
                if !summary.failed.is_empty() {
                    let servers: Vec<_> = summary.failed.iter().map(|f| f.server.clone()).collect();
                    parts.push(format!("failed: {}", servers.join(", ")));
                }
                if !summary.cancelled.is_empty() {
                    parts.push(format!("cancelled: {}", summary.cancelled.join(", ")));
                }
                let joined = if parts.is_empty() {
                    "no servers".to_string()
                } else {
                    parts.join("; ")
                };
                ts_msg!(self, "{} {}", "mcp startup:".style(self.cyan), joined);
            }
            EventMsg::BackgroundEvent(BackgroundEventEvent { message }) => {
                ts_msg!(self, "{}", message.style(self.dimmed));
            }
            EventMsg::StreamError(StreamErrorEvent {
                message,
                additional_details,
                ..
            }) => {
                let message = match additional_details {
                    Some(details) if !details.trim().is_empty() => format!("{message} ({details})"),
                    _ => message,
                };
                ts_msg!(self, "{}", message.style(self.dimmed));
            }
            EventMsg::TaskStarted(_) => {
                // Ignore.
            }
            EventMsg::ElicitationRequest(ev) => {
                ts_msg!(
                    self,
                    "{} {}",
                    "elicitation request".style(self.magenta),
                    ev.server_name.style(self.dimmed)
                );
                ts_msg!(
                    self,
                    "{}",
                    "auto-cancelling (not supported in exec mode)".style(self.dimmed)
                );
            }
            EventMsg::TaskComplete(TaskCompleteEvent { last_agent_message }) => {
                let last_message = last_agent_message.as_deref();
                if let Some(output_file) = self.last_message_path.as_deref() {
                    handle_last_message(last_message, output_file);
                }

                self.final_message = last_agent_message;

                return CodexStatus::InitiateShutdown;
            }
            EventMsg::TokenCount(ev) => {
                self.last_total_token_usage = ev.info;
            }

            EventMsg::AgentReasoningSectionBreak(_) => {
                if !self.show_agent_reasoning {
                    return CodexStatus::Running;
                }
                eprintln!();
            }
            EventMsg::AgentReasoningRawContent(AgentReasoningRawContentEvent { text }) => {
                if self.show_raw_agent_reasoning {
                    ts_msg!(
                        self,
                        "{}\n{}",
                        "thinking".style(self.italic).style(self.magenta),
                        text,
                    );
                }
            }
            EventMsg::AgentMessage(AgentMessageEvent { message }) => {
                ts_msg!(
                    self,
                    "{}\n{}",
                    "codex".style(self.italic).style(self.magenta),
                    message,
                );
            }
            EventMsg::ExecCommandBegin(ExecCommandBeginEvent { command, cwd, .. }) => {
                eprint!(
                    "{}\n{} in {}",
                    "exec".style(self.italic).style(self.magenta),
                    escape_command(&command).style(self.bold),
                    cwd.to_string_lossy(),
                );
            }
            EventMsg::ExecCommandEnd(ExecCommandEndEvent {
                aggregated_output,
                duration,
                exit_code,
                ..
            }) => {
                let duration = format!(" in {}", format_duration(duration));

                let truncated_output = aggregated_output
                    .lines()
                    .take(MAX_OUTPUT_LINES_FOR_EXEC_TOOL_CALL)
                    .collect::<Vec<_>>()
                    .join("\n");
                match exit_code {
                    0 => {
                        let title = format!(" succeeded{duration}:");
                        ts_msg!(self, "{}", title.style(self.green));
                    }
                    _ => {
                        let title = format!(" exited {exit_code}{duration}:");
                        ts_msg!(self, "{}", title.style(self.red));
                    }
                }
                eprintln!("{}", truncated_output.style(self.dimmed));
            }
            EventMsg::McpToolCallBegin(McpToolCallBeginEvent {
                call_id: _,
                invocation,
            }) => {
                ts_msg!(
                    self,
                    "{} {}",
                    "tool".style(self.magenta),
                    format_mcp_invocation(&invocation).style(self.bold),
                );
            }
            EventMsg::McpToolCallEnd(tool_call_end_event) => {
                let is_success = tool_call_end_event.is_success();
                let McpToolCallEndEvent {
                    call_id: _,
                    result,
                    invocation,
                    duration,
                } = tool_call_end_event;

                let duration = format!(" in {}", format_duration(duration));

                let status_str = if is_success { "success" } else { "failed" };
                let title_style = if is_success { self.green } else { self.red };
                let title = format!(
                    "{} {status_str}{duration}:",
                    format_mcp_invocation(&invocation)
                );

                ts_msg!(self, "{}", title.style(title_style));

                if let Ok(res) = result {
                    let val: serde_json::Value = res.into();
                    let pretty =
                        serde_json::to_string_pretty(&val).unwrap_or_else(|_| val.to_string());

                    for line in pretty.lines().take(MAX_OUTPUT_LINES_FOR_EXEC_TOOL_CALL) {
                        eprintln!("{}", line.style(self.dimmed));
                    }
                }
            }
            EventMsg::WebSearchEnd(WebSearchEndEvent { call_id: _, query }) => {
                ts_msg!(self, "🌐 Searched: {query}");
            }
            EventMsg::PatchApplyBegin(PatchApplyBeginEvent {
                call_id,
                auto_approved,
                changes,
                ..
            }) => {
                // Store metadata so we can calculate duration later when we
                // receive the corresponding PatchApplyEnd event.
                self.call_id_to_patch.insert(
                    call_id,
                    PatchApplyBegin {
                        start_time: Instant::now(),
                        auto_approved,
                    },
                );

                ts_msg!(
                    self,
                    "{}",
                    "file update".style(self.magenta).style(self.italic),
                );

                // Pretty-print the patch summary with colored diff markers so
                // it's easy to scan in the terminal output.
                for (path, change) in changes.iter() {
                    match change {
                        FileChange::Add { content } => {
                            let header = format!(
                                "{} {}",
                                format_file_change(change),
                                path.to_string_lossy()
                            );
                            eprintln!("{}", header.style(self.magenta));
                            for line in content.lines() {
                                eprintln!("{}", line.style(self.green));
                            }
                        }
                        FileChange::Delete { content } => {
                            let header = format!(
                                "{} {}",
                                format_file_change(change),
                                path.to_string_lossy()
                            );
                            eprintln!("{}", header.style(self.magenta));
                            for line in content.lines() {
                                eprintln!("{}", line.style(self.red));
                            }
                        }
                        FileChange::Update {
                            unified_diff,
                            move_path,
                        } => {
                            let header = if let Some(dest) = move_path {
                                format!(
                                    "{} {} -> {}",
                                    format_file_change(change),
                                    path.to_string_lossy(),
                                    dest.to_string_lossy()
                                )
                            } else {
                                format!("{} {}", format_file_change(change), path.to_string_lossy())
                            };
                            eprintln!("{}", header.style(self.magenta));

                            // Colorize diff lines. We keep file header lines
                            // (--- / +++) without extra coloring so they are
                            // still readable.
                            for diff_line in unified_diff.lines() {
                                if diff_line.starts_with('+') && !diff_line.starts_with("+++") {
                                    eprintln!("{}", diff_line.style(self.green));
                                } else if diff_line.starts_with('-')
                                    && !diff_line.starts_with("---")
                                {
                                    eprintln!("{}", diff_line.style(self.red));
                                } else {
                                    eprintln!("{diff_line}");
                                }
                            }
                        }
                    }
                }
            }
            EventMsg::PatchApplyEnd(PatchApplyEndEvent {
                call_id,
                stdout,
                stderr,
                success,
                ..
            }) => {
                let patch_begin = self.call_id_to_patch.remove(&call_id);

                // Compute duration and summary label similar to exec commands.
                let (duration, label) = if let Some(PatchApplyBegin {
                    start_time,
                    auto_approved,
                }) = patch_begin
                {
                    (
                        format!(" in {}", format_elapsed(start_time)),
                        format!("apply_patch(auto_approved={auto_approved})"),
                    )
                } else {
                    (String::new(), format!("apply_patch('{call_id}')"))
                };

                let (exit_code, output, title_style) = if success {
                    (0, stdout, self.green)
                } else {
                    (1, stderr, self.red)
                };

                let title = format!("{label} exited {exit_code}{duration}:");
                ts_msg!(self, "{}", title.style(title_style));
                for line in output.lines() {
                    eprintln!("{}", line.style(self.dimmed));
                }
            }
            EventMsg::TurnDiff(TurnDiffEvent { unified_diff }) => {
                ts_msg!(
                    self,
                    "{}",
                    "file update:".style(self.magenta).style(self.italic)
                );
                eprintln!("{unified_diff}");
            }
            EventMsg::AgentReasoning(agent_reasoning_event) => {
                if self.show_agent_reasoning {
                    ts_msg!(
                        self,
                        "{}\n{}",
                        "thinking".style(self.italic).style(self.magenta),
                        agent_reasoning_event.text,
                    );
                }
            }
            EventMsg::SessionConfigured(session_configured_event) => {
                let SessionConfiguredEvent {
                    session_id: conversation_id,
                    model,
                    ..
                } = session_configured_event;

                ts_msg!(
                    self,
                    "{} {}",
                    "codex session".style(self.magenta).style(self.bold),
                    conversation_id.to_string().style(self.dimmed)
                );

                ts_msg!(self, "model: {}", model);
                eprintln!();
            }
            EventMsg::PlanUpdate(plan_update_event) => {
                let UpdatePlanArgs { explanation, plan } = plan_update_event;

                // Header
                ts_msg!(self, "{}", "Plan update".style(self.magenta));

                // Optional explanation
                if let Some(explanation) = explanation
                    && !explanation.trim().is_empty()
                {
                    ts_msg!(self, "{}", explanation.style(self.italic));
                }

                // Pretty-print the plan items with simple status markers.
                for item in plan {
                    match item.status {
                        StepStatus::Completed => {
                            ts_msg!(self, "  {} {}", "✓".style(self.green), item.step);
                        }
                        StepStatus::InProgress => {
                            ts_msg!(self, "  {} {}", "→".style(self.cyan), item.step);
                        }
                        StepStatus::Pending => {
                            ts_msg!(
                                self,
                                "  {} {}",
                                "•".style(self.dimmed),
                                item.step.style(self.dimmed)
                            );
                        }
                    }
                }
            }
            EventMsg::ViewImageToolCall(view) => {
                ts_msg!(
                    self,
                    "{} {}",
                    "viewed image".style(self.magenta),
                    view.path.display()
                );
            }
            EventMsg::TurnAborted(abort_reason) => match abort_reason.reason {
                TurnAbortReason::Interrupted => {
                    ts_msg!(self, "task interrupted");
                }
                TurnAbortReason::Replaced => {
                    ts_msg!(self, "task aborted: replaced by a new task");
                }
                TurnAbortReason::ReviewEnded => {
                    ts_msg!(self, "task aborted: review ended");
                }
            },
            EventMsg::ContextCompacted(_) => {
                ts_msg!(self, "context compacted");
            }
            EventMsg::ShutdownComplete => return CodexStatus::Shutdown,
            EventMsg::WebSearchBegin(_)
            | EventMsg::ExecApprovalRequest(_)
            | EventMsg::ApplyPatchApprovalRequest(_)
            | EventMsg::TerminalInteraction(_)
            | EventMsg::ExecCommandOutputDelta(_)
            | EventMsg::GetHistoryEntryResponse(_)
            | EventMsg::McpListToolsResponse(_)
            | EventMsg::ListCustomPromptsResponse(_)
            | EventMsg::ListSkillsResponse(_)
            | EventMsg::RawResponseItem(_)
            | EventMsg::UserMessage(_)
            | EventMsg::EnteredReviewMode(_)
            | EventMsg::ExitedReviewMode(_)
            | EventMsg::AgentMessageDelta(_)
            | EventMsg::AgentReasoningDelta(_)
            | EventMsg::AgentReasoningRawContentDelta(_)
            | EventMsg::ItemStarted(_)
            | EventMsg::ItemCompleted(_)
            | EventMsg::AgentMessageContentDelta(_)
            | EventMsg::ReasoningContentDelta(_)
            | EventMsg::ReasoningRawContentDelta(_)
            | EventMsg::SkillsUpdateAvailable
            | EventMsg::UndoCompleted(_)
            | EventMsg::UndoStarted(_) => {}
        }
        CodexStatus::Running
    }

    fn print_final_output(&mut self) {
        if let Some(usage_info) = &self.last_total_token_usage {
            eprintln!(
                "{}\n{}",
                "tokens used".style(self.magenta).style(self.italic),
                format_with_separators(usage_info.total_token_usage.blended_total())
            );
        }

        // If the user has not piped the final message to a file, they will see
        // it twice: once written to stderr as part of the normal event
        // processing, and once here on stdout. We print the token summary above
        // to help break up the output visually in that case.
        #[allow(clippy::print_stdout)]
        if let Some(message) = &self.final_message {
            if message.ends_with('\n') {
                print!("{message}");
            } else {
                println!("{message}");
            }
        }
    }
}

fn escape_command(command: &[String]) -> String {
    try_join(command.iter().map(String::as_str)).unwrap_or_else(|_| command.join(" "))
}

fn format_file_change(change: &FileChange) -> &'static str {
    match change {
        FileChange::Add { .. } => "A",
        FileChange::Delete { .. } => "D",
        FileChange::Update {
            move_path: Some(_), ..
        } => "R",
        FileChange::Update {
            move_path: None, ..
        } => "M",
    }
}

fn format_mcp_invocation(invocation: &McpInvocation) -> String {
    // Build fully-qualified tool name: server.tool
    let fq_tool_name = format!("{}.{}", invocation.server, invocation.tool);

    // Format arguments as compact JSON so they fit on one line.
    let args_str = invocation
        .arguments
        .as_ref()
        .map(|v: &serde_json::Value| serde_json::to_string(v).unwrap_or_else(|_| v.to_string()))
        .unwrap_or_default();

    if args_str.is_empty() {
        format!("{fq_tool_name}()")
    } else {
        format!("{fq_tool_name}({args_str})")
    }
}
