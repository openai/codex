use chrono::Utc;
use codex_common::elapsed::format_elapsed;
use codex_core::protocol::AgentMessageEvent;
use codex_core::protocol::BackgroundEventEvent;
use codex_core::protocol::ErrorEvent;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ExecCommandBeginEvent;
use codex_core::protocol::ExecCommandEndEvent;
use codex_core::protocol::FileChange;
use codex_core::protocol::McpToolCallBeginEvent;
use codex_core::protocol::McpToolCallEndEvent;
use codex_core::protocol::PatchApplyBeginEvent;
use codex_core::protocol::PatchApplyEndEvent;
use codex_core::protocol::SessionConfiguredEvent;
use owo_colors::OwoColorize;
use owo_colors::Style;
use shlex::try_join;
use std::collections::HashMap;
use std::time::Instant;

/// This should be configurable. When used in CI, users may not want to impose
/// a limit so they can see the full transcript.
const MAX_OUTPUT_LINES_FOR_EXEC_TOOL_CALL: usize = 20;

pub(crate) struct EventProcessor {
    call_id_to_command: HashMap<String, ExecCommandBegin>,
    call_id_to_patch: HashMap<String, PatchApplyBegin>,

    /// Tracks in-flight MCP tool calls so we can calculate duration and print
    /// a concise summary when the corresponding `McpToolCallEnd` event is
    /// received.
    call_id_to_tool_call: HashMap<String, McpToolCallBegin>,

    // To ensure that --color=never is respected, ANSI escapes _must_ be added
    // using .style() with one of these fields. If you need a new style, add a
    // new field here.
    bold: Style,
    dimmed: Style,

    magenta: Style,
    red: Style,
    green: Style,
}

impl EventProcessor {
    pub(crate) fn create_with_ansi(with_ansi: bool) -> Self {
        let call_id_to_command = HashMap::new();
        let call_id_to_patch = HashMap::new();
        let call_id_to_tool_call = HashMap::new();

        if with_ansi {
            Self {
                call_id_to_command,
                call_id_to_patch,
                bold: Style::new().bold(),
                dimmed: Style::new().dimmed(),
                magenta: Style::new().magenta(),
                red: Style::new().red(),
                green: Style::new().green(),
                call_id_to_tool_call,
            }
        } else {
            Self {
                call_id_to_command,
                call_id_to_patch,
                bold: Style::new(),
                dimmed: Style::new(),
                magenta: Style::new(),
                red: Style::new(),
                green: Style::new(),
                call_id_to_tool_call,
            }
        }
    }
}

struct ExecCommandBegin {
    command: Vec<String>,
    start_time: Instant,
}

/// Metadata captured when an `McpToolCallBegin` event is received.
struct McpToolCallBegin {
    /// Formatted invocation string, e.g. `server.tool({"city":"sf"})`.
    invocation: String,
    /// Timestamp when the call started so we can compute duration later.
    start_time: Instant,
}

struct PatchApplyBegin {
    start_time: Instant,
    auto_approved: bool,
}

macro_rules! ts_println {
    ($($arg:tt)*) => {{
        let now = Utc::now();
        let formatted = now.format("%Y-%m-%dT%H:%M:%S").to_string();
        print!("[{}] ", formatted);
        println!($($arg)*);
    }};
}

impl EventProcessor {
    pub(crate) fn process_event(&mut self, event: Event) {
        let Event { id, msg } = event;
        match msg {
            EventMsg::Error(ErrorEvent { message }) => {
                let prefix = "ERROR:".style(self.red);
                ts_println!("{prefix} {message}");
            }
            EventMsg::BackgroundEvent(BackgroundEventEvent { message }) => {
                ts_println!("{}", message.style(self.dimmed));
            }
            EventMsg::TaskStarted => {
                let msg = format!("Task started: {id}");
                ts_println!("{}", msg.style(self.dimmed));
            }
            EventMsg::TaskComplete => {
                let msg = format!("Task complete: {id}");
                ts_println!("{}", msg.style(self.bold));
            }
            EventMsg::AgentMessage(AgentMessageEvent { message }) => {
                let prefix = "Agent message:".style(self.bold);
                ts_println!("{prefix} {message}");
            }
            EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
                call_id,
                command,
                cwd,
            }) => {
                self.call_id_to_command.insert(
                    call_id.clone(),
                    ExecCommandBegin {
                        command: command.clone(),
                        start_time: Instant::now(),
                    },
                );
                ts_println!(
                    "{} {} in {}",
                    "exec".style(self.magenta),
                    escape_command(&command).style(self.bold),
                    cwd.to_string_lossy(),
                );
            }
            EventMsg::ExecCommandEnd(ExecCommandEndEvent {
                call_id,
                stdout,
                stderr,
                exit_code,
            }) => {
                let exec_command = self.call_id_to_command.remove(&call_id);
                let (duration, call) = if let Some(ExecCommandBegin {
                    command,
                    start_time,
                }) = exec_command
                {
                    (
                        format!(" in {}", format_elapsed(start_time)),
                        format!("{}", escape_command(&command).style(self.bold)),
                    )
                } else {
                    ("".to_string(), format!("exec('{call_id}')"))
                };

                let output = if exit_code == 0 { stdout } else { stderr };
                let truncated_output = output
                    .lines()
                    .take(MAX_OUTPUT_LINES_FOR_EXEC_TOOL_CALL)
                    .collect::<Vec<_>>()
                    .join("\n");
                match exit_code {
                    0 => {
                        let title = format!("{call} succeeded{duration}:");
                        ts_println!("{}", title.style(self.green));
                    }
                    _ => {
                        let title = format!("{call} exited {exit_code}{duration}:");
                        ts_println!("{}", title.style(self.red));
                    }
                }
                println!("{}", truncated_output.style(self.dimmed));
            }
            EventMsg::McpToolCallBegin(McpToolCallBeginEvent {
                call_id,
                server,
                tool,
                arguments,
            }) => {
                // Build fully-qualified tool name: server.tool
                let fq_tool_name = format!("{server}.{tool}");

                // Format arguments as compact JSON so they fit on one line.
                let args_str = arguments
                    .as_ref()
                    .map(|v: &serde_json::Value| {
                        serde_json::to_string(v).unwrap_or_else(|_| v.to_string())
                    })
                    .unwrap_or_default();

                let invocation = if args_str.is_empty() {
                    format!("{fq_tool_name}()")
                } else {
                    format!("{fq_tool_name}({args_str})")
                };

                self.call_id_to_tool_call.insert(
                    call_id.clone(),
                    McpToolCallBegin {
                        invocation: invocation.clone(),
                        start_time: Instant::now(),
                    },
                );

                ts_println!(
                    "{} {}",
                    "tool".style(self.magenta),
                    invocation.style(self.bold),
                );
            }
            EventMsg::McpToolCallEnd(McpToolCallEndEvent {
                call_id,
                success,
                result,
            }) => {
                // Retrieve start time and invocation for duration calculation and labeling.
                let info = self.call_id_to_tool_call.remove(&call_id);

                let (duration, invocation) = if let Some(McpToolCallBegin {
                    invocation,
                    start_time,
                    ..
                }) = info
                {
                    (format!(" in {}", format_elapsed(start_time)), invocation)
                } else {
                    (String::new(), format!("tool('{call_id}')"))
                };

                let status_str = if success { "success" } else { "failed" };
                let title_style = if success { self.green } else { self.red };
                let title = format!("{invocation} {status_str}{duration}:");

                ts_println!("{}", title.style(title_style));

                if let Some(res) = result {
                    let val: serde_json::Value = res.into();
                    let pretty =
                        serde_json::to_string_pretty(&val).unwrap_or_else(|_| val.to_string());

                    for line in pretty.lines().take(MAX_OUTPUT_LINES_FOR_EXEC_TOOL_CALL) {
                        println!("{}", line.style(self.dimmed));
                    }
                }
            }
            EventMsg::PatchApplyBegin(PatchApplyBeginEvent {
                call_id,
                auto_approved,
                changes,
            }) => {
                // Store metadata so we can calculate duration later when we
                // receive the corresponding PatchApplyEnd event.
                self.call_id_to_patch.insert(
                    call_id.clone(),
                    PatchApplyBegin {
                        start_time: Instant::now(),
                        auto_approved,
                    },
                );

                ts_println!(
                    "{} auto_approved={}:",
                    "apply_patch".style(self.magenta),
                    auto_approved,
                );

                // Pretty-print the patch summary with colored diff markers so
                // it’s easy to scan in the terminal output.
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
                        FileChange::Delete => {
                            let header = format!(
                                "{} {}",
                                format_file_change(change),
                                path.to_string_lossy()
                            );
                            println!("{}", header.style(self.magenta));
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
                        format!("apply_patch(auto_approved={})", auto_approved),
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
                ts_println!("{}", title.style(title_style));
                for line in output.lines() {
                    println!("{}", line.style(self.dimmed));
                }
            }
            EventMsg::ExecApprovalRequest(_) => {
                // Should we exit?
            }
            EventMsg::ApplyPatchApprovalRequest(_) => {
                // Should we exit?
            }
            EventMsg::AgentReasoning(agent_reasoning_event) => {
                println!("thinking: {}", agent_reasoning_event.text);
            }
            EventMsg::SessionConfigured(session_configured_event) => {
                let SessionConfiguredEvent {
                    session_id,
                    model,
                    history_log_id: _,
                    history_entry_count: _,
                } = session_configured_event;
                println!("session {session_id} with model {model}");
            }
            EventMsg::GetHistoryEntryResponse(_) => {
                // Currently ignored in exec output.
            }
        }
    }
}

fn escape_command(command: &[String]) -> String {
    try_join(command.iter().map(|s| s.as_str())).unwrap_or_else(|_| command.join(" "))
}

fn format_file_change(change: &FileChange) -> &'static str {
    match change {
        FileChange::Add { .. } => "A",
        FileChange::Delete => "D",
        FileChange::Update {
            move_path: Some(_), ..
        } => "R",
        FileChange::Update {
            move_path: None, ..
        } => "M",
    }
}
