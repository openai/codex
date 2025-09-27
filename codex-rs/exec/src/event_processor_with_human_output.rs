use codex_common::elapsed::format_duration;
use codex_common::elapsed::format_elapsed;
use codex_core::config::Config;
use codex_core::plan_tool::UpdatePlanArgs;
use codex_core::protocol::AgentMessageDeltaEvent;
use codex_core::protocol::AgentMessageEvent;
use codex_core::protocol::AgentReasoningDeltaEvent;
use codex_core::protocol::AgentReasoningRawContentDeltaEvent;
use codex_core::protocol::AgentReasoningRawContentEvent;
use codex_core::protocol::BackgroundEventEvent;
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
use codex_core::protocol::WebSearchBeginEvent;
use codex_core::protocol::WebSearchEndEvent;
use codex_protocol::num_format::format_with_separators;
use owo_colors::OwoColorize;
use owo_colors::Style;
use shlex::try_join;
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use crate::event_processor::CodexStatus;
use crate::event_processor::EventProcessor;
use crate::event_processor::handle_last_message;
use codex_common::create_config_summary_entries;

/// This should be configurable. When used in CI, users may not want to impose
/// a limit so they can see the full transcript.
const MAX_OUTPUT_LINES_FOR_EXEC_TOOL_CALL: usize = 20;
pub(crate) struct EventProcessorWithHumanOutput {
    call_id_to_command: HashMap<String, ExecCommandBegin>,
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

    /// Whether to include `AgentReasoning` events in the output.
    show_agent_reasoning: bool,
    show_raw_agent_reasoning: bool,
    answer_started: bool,
    reasoning_started: bool,
    raw_reasoning_started: bool,
    last_message_path: Option<PathBuf>,
}

impl EventProcessorWithHumanOutput {
    pub(crate) fn create_with_ansi(
        with_ansi: bool,
        config: &Config,
        last_message_path: Option<PathBuf>,
    ) -> Self {
        let call_id_to_command = HashMap::new();
        let call_id_to_patch = HashMap::new();

        if with_ansi {
            Self {
                call_id_to_command,
                call_id_to_patch,
                bold: Style::new().bold(),
                italic: Style::new().italic(),
                dimmed: Style::new().dimmed(),
                magenta: Style::new().magenta(),
                red: Style::new().red(),
                green: Style::new().green(),
                cyan: Style::new().cyan(),
                show_agent_reasoning: !config.hide_agent_reasoning,
                show_raw_agent_reasoning: config.show_raw_agent_reasoning,
                answer_started: false,
                reasoning_started: false,
                raw_reasoning_started: false,
                last_message_path,
            }
        } else {
            Self {
                call_id_to_command,
                call_id_to_patch,
                bold: Style::new(),
                italic: Style::new(),
                dimmed: Style::new(),
                magenta: Style::new(),
                red: Style::new(),
                green: Style::new(),
                cyan: Style::new(),
                show_agent_reasoning: !config.hide_agent_reasoning,
                show_raw_agent_reasoning: config.show_raw_agent_reasoning,
                answer_started: false,
                reasoning_started: false,
                raw_reasoning_started: false,
                last_message_path,
            }
        }
    }
}

struct ExecCommandBegin {
    command: Vec<String>,
}

struct PatchApplyBegin {
    start_time: Instant,
    auto_approved: bool,
}

// Timestamped println helper. The timestamp is styled with self.dimmed.
#[macro_export]
macro_rules! ts_println {
    ($self:ident, $($arg:tt)*) => {{
        let now = chrono::Utc::now();
        let formatted = now.format("[%Y-%m-%dT%H:%M:%S]");
        print!("{} ", formatted.style($self.dimmed));
        println!($($arg)*);
    }};
}

impl EventProcessor for EventProcessorWithHumanOutput {
    /// Print a concise summary of the effective configuration that will be used
    /// for the session. This mirrors the information shown in the TUI welcome
    /// screen.
    fn print_config_summary(&mut self, config: &Config, prompt: &str, _: &SessionConfiguredEvent) {
        const VERSION: &str = env!("CARGO_PKG_VERSION");
        ts_println!(
            self,
            "OpenAI Codex v{} (research preview)\n--------",
            VERSION
        );

        let entries = create_config_summary_entries(config);

        for (key, value) in entries {
            println!("{} {}", format!("{key}:").style(self.bold), value);
        }

        println!("--------");

        // Echo the prompt that will be sent to the agent so it is visible in the
        // transcript/logs before any events come in. Note the prompt may have been
        // read from stdin, so it may not be visible in the terminal otherwise.
        ts_println!(
            self,
            "{}\n{}",
            "User instructions:".style(self.bold).style(self.cyan),
            prompt
        );
    }

    fn process_event(&mut self, event: Event) -> CodexStatus {
        let Event { id: _, msg } = event;
        match msg {
            EventMsg::Error(ErrorEvent { message }) => {
                let prefix = "ERROR:".style(self.red);
                ts_println!(self, "{prefix} {message}");
            }
            EventMsg::BackgroundEvent(BackgroundEventEvent { message }) => {
                ts_println!(self, "{}", message.style(self.dimmed));
            }
            EventMsg::StreamError(StreamErrorEvent { message }) => {
                ts_println!(self, "{}", message.style(self.dimmed));
            }
            EventMsg::TaskStarted(_) => {
                // Ignore.
            }
            EventMsg::TaskComplete(TaskCompleteEvent { last_agent_message }) => {
                if let Some(output_file) = self.last_message_path.as_deref() {
                    handle_last_message(last_agent_message.as_deref(), output_file);
                }
                return CodexStatus::InitiateShutdown;
            }
            EventMsg::TokenCount(ev) => {
                if let Some(usage_info) = ev.info {
                    ts_println!(
                        self,
                        "tokens used: {}",
                        format_with_separators(usage_info.total_token_usage.blended_total())
                    );
                }
            }
            EventMsg::AgentMessageDelta(AgentMessageDeltaEvent { delta }) => {
                if !self.answer_started {
                    ts_println!(self, "{}\n", "codex".style(self.italic).style(self.magenta));
                    self.answer_started = true;
                }
                print!("{delta}");
                #[expect(clippy::expect_used)]
                std::io::stdout().flush().expect("could not flush stdout");
            }
            EventMsg::AgentReasoningDelta(AgentReasoningDeltaEvent { delta }) => {
                if !self.show_agent_reasoning {
                    return CodexStatus::Running;
                }
                if !self.reasoning_started {
                    ts_println!(
                        self,
                        "{}\n",
                        "thinking".style(self.italic).style(self.magenta),
                    );
                    self.reasoning_started = true;
                }
                print!("{delta}");
                #[expect(clippy::expect_used)]
                std::io::stdout().flush().expect("could not flush stdout");
            }
            EventMsg::AgentReasoningSectionBreak(_) => {
                if !self.show_agent_reasoning {
                    return CodexStatus::Running;
                }
                println!();
                #[expect(clippy::expect_used)]
                std::io::stdout().flush().expect("could not flush stdout");
            }
            EventMsg::AgentReasoningRawContent(AgentReasoningRawContentEvent { text }) => {
                if !self.show_raw_agent_reasoning {
                    return CodexStatus::Running;
                }
                if !self.raw_reasoning_started {
                    print!("{text}");
                    #[expect(clippy::expect_used)]
                    std::io::stdout().flush().expect("could not flush stdout");
                } else {
                    println!();
                    self.raw_reasoning_started = false;
                }
            }
            EventMsg::AgentReasoningRawContentDelta(AgentReasoningRawContentDeltaEvent {
                delta,
            }) => {
                if !self.show_raw_agent_reasoning {
                    return CodexStatus::Running;
                }
                if !self.raw_reasoning_started {
                    self.raw_reasoning_started = true;
                }
                print!("{delta}");
                #[expect(clippy::expect_used)]
                std::io::stdout().flush().expect("could not flush stdout");
            }
            EventMsg::AgentMessage(AgentMessageEvent { message }) => {
                // if answer_started is false, this means we haven't received any
                // delta. Thus, we need to print the message as a new answer.
                if !self.answer_started {
                    ts_println!(
                        self,
                        "{}\n{}",
                        "codex".style(self.italic).style(self.magenta),
                        message,
                    );
                } else {
                    println!();
                    self.answer_started = false;
                }
            }
            EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
                call_id,
                command,
                cwd,
                parsed_cmd: _,
            }) => {
                self.call_id_to_command.insert(
                    call_id,
                    ExecCommandBegin {
                        command: command.clone(),
                    },
                );
                ts_println!(
                    self,
                    "{} {} in {}",
                    "exec".style(self.magenta),
                    escape_command(&command).style(self.bold),
                    cwd.to_string_lossy(),
                );
            }
            EventMsg::ExecCommandOutputDelta(_) => {}
            EventMsg::ExecCommandEnd(ExecCommandEndEvent {
                call_id,
                aggregated_output,
                duration,
                exit_code,
                ..
            }) => {
                let exec_command = self.call_id_to_command.remove(&call_id);
                let (duration, call) = if let Some(ExecCommandBegin { command, .. }) = exec_command
                {
                    (
                        format!(" in {}", format_duration(duration)),
                        format!("{}", escape_command(&command).style(self.bold)),
                    )
                } else {
                    ("".to_string(), format!("exec('{call_id}')"))
                };

                let truncated_output = aggregated_output
                    .lines()
                    .take(MAX_OUTPUT_LINES_FOR_EXEC_TOOL_CALL)
                    .collect::<Vec<_>>()
                    .join("\n");
                match exit_code {
                    0 => {
                        let title = format!("{call} succeeded{duration}:");
                        ts_println!(self, "{}", title.style(self.green));
                    }
                    _ => {
                        let title = format!("{call} exited {exit_code}{duration}:");
                        ts_println!(self, "{}", title.style(self.red));
                    }
                }
                println!("{}", truncated_output.style(self.dimmed));
            }
            EventMsg::McpToolCallBegin(McpToolCallBeginEvent {
                call_id: _,
                invocation,
            }) => {
                ts_println!(
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

                ts_println!(self, "{}", title.style(title_style));

                if let Ok(res) = result {
                    let val: serde_json::Value = res.into();
                    let pretty =
                        serde_json::to_string_pretty(&val).unwrap_or_else(|_| val.to_string());

                    for line in pretty.lines().take(MAX_OUTPUT_LINES_FOR_EXEC_TOOL_CALL) {
                        println!("{}", line.style(self.dimmed));
                    }
                }
            }
            EventMsg::WebSearchBegin(WebSearchBeginEvent { call_id: _ }) => {}
            EventMsg::WebSearchEnd(WebSearchEndEvent { call_id: _, query }) => {
                ts_println!(self, "🌐 Searched: {query}");
            }
            EventMsg::PatchApplyBegin(PatchApplyBeginEvent {
                call_id,
                auto_approved,
                changes,
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

                ts_println!(
                    self,
                    "{} auto_approved={}:",
                    "apply_patch".style(self.magenta),
                    auto_approved,
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
                            println!("{}", header.style(self.magenta));
                            for line in content.lines() {
                                println!("{}", line.style(self.green));
                            }
                        }
                        FileChange::Delete { content } => {
                            let header = format!(
                                "{} {}",
                                format_file_change(change),
                                path.to_string_lossy()
                            );
                            println!("{}", header.style(self.magenta));
                            for line in content.lines() {
                                println!("{}", line.style(self.red));
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
                            println!("{}", header.style(self.magenta));

                            // Colorize diff lines. We keep file header lines
                            // (--- / +++) without extra coloring so they are
                            // still readable.
                            for diff_line in unified_diff.lines() {
                                if diff_line.starts_with('+') && !diff_line.starts_with("+++") {
                                    println!("{}", diff_line.style(self.green));
                                } else if diff_line.starts_with('-')
                                    && !diff_line.starts_with("---")
                                {
                                    println!("{}", diff_line.style(self.red));
                                } else {
                                    println!("{diff_line}");
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
                ts_println!(self, "{}", title.style(title_style));
                for line in output.lines() {
                    println!("{}", line.style(self.dimmed));
                }
            }
            EventMsg::TurnDiff(TurnDiffEvent { unified_diff }) => {
                ts_println!(self, "{}", "turn diff:".style(self.magenta));
                println!("{unified_diff}");
            }
            EventMsg::ExecApprovalRequest(_) => {
                // Should we exit?
            }
            EventMsg::ApplyPatchApprovalRequest(_) => {
                // Should we exit?
            }
            EventMsg::AgentReasoning(agent_reasoning_event) => {
                if self.show_agent_reasoning {
                    if !self.reasoning_started {
                        ts_println!(
                            self,
                            "{}\n{}",
                            "codex".style(self.italic).style(self.magenta),
                            agent_reasoning_event.text,
                        );
                    } else {
                        println!();
                        self.reasoning_started = false;
                    }
                }
            }
            EventMsg::SessionConfigured(session_configured_event) => {
                let SessionConfiguredEvent {
                    session_id: conversation_id,
                    model,
                    reasoning_effort: _,
                    history_log_id: _,
                    history_entry_count: _,
                    initial_messages: _,
                    rollout_path: _,
                } = session_configured_event;

                ts_println!(
                    self,
                    "{} {}",
                    "codex session".style(self.magenta).style(self.bold),
                    conversation_id.to_string().style(self.dimmed)
                );

                ts_println!(self, "model: {}", model);
                println!();
            }
            EventMsg::PlanUpdate(plan_update_event) => {
                let UpdatePlanArgs { explanation, plan } = plan_update_event;

                // Header
                ts_println!(self, "{}", "Plan update".style(self.magenta));

                // Optional explanation
                if let Some(explanation) = explanation
                    && !explanation.trim().is_empty()
                {
                    ts_println!(self, "{}", explanation.style(self.italic));
                }

                // Pretty-print the plan items with simple status markers.
                for item in plan {
                    use codex_core::plan_tool::StepStatus;
                    match item.status {
                        StepStatus::Completed => {
                            ts_println!(self, "  {} {}", "✓".style(self.green), item.step);
                        }
                        StepStatus::InProgress => {
                            ts_println!(self, "  {} {}", "→".style(self.cyan), item.step);
                        }
                        StepStatus::Pending => {
                            ts_println!(
                                self,
                                "  {} {}",
                                "•".style(self.dimmed),
                                item.step.style(self.dimmed)
                            );
                        }
                    }
                }
            }
            EventMsg::GetHistoryEntryResponse(_) => {
                // Currently ignored in exec output.
            }
            EventMsg::McpListToolsResponse(_) => {
                // Currently ignored in exec output.
            }
            EventMsg::ListCustomPromptsResponse(_) => {
                // Currently ignored in exec output.
            }
            EventMsg::TurnAborted(abort_reason) => match abort_reason.reason {
                TurnAbortReason::Interrupted => {
                    ts_println!(self, "task interrupted");
                }
                TurnAbortReason::Replaced => {
                    ts_println!(self, "task aborted: replaced by a new task");
                }
                TurnAbortReason::ReviewEnded => {
                    ts_println!(self, "task aborted: review ended");
                }
            },
            EventMsg::ShutdownComplete => return CodexStatus::Shutdown,
            EventMsg::ConversationPath(_) => {}
            EventMsg::UserMessage(_) => {}
            EventMsg::EnteredReviewMode(_) => {}
            EventMsg::ExitedReviewMode(_) => {}
        }
        CodexStatus::Running
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
