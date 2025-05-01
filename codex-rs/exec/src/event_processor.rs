use chrono::Utc;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::FileChange;
use owo_colors::OwoColorize;
use owo_colors::Style;
use shlex::try_join;
use std::collections::HashMap;

/// This should be configurable. When used in CI, users may not want to impose
/// a limit so they can see the full transcript.
const MAX_OUTPUT_LINES_FOR_EXEC_TOOL_CALL: usize = 20;

pub(crate) struct EventProcessor {
    call_id_to_command: HashMap<String, ExecCommandBegin>,
    call_id_to_patch: HashMap<String, PatchApplyBegin>,

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

        if with_ansi {
            Self {
                call_id_to_command,
                call_id_to_patch,
                bold: Style::new().bold(),
                dimmed: Style::new().dimmed(),
                magenta: Style::new().magenta(),
                red: Style::new().red(),
                green: Style::new().green(),
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
            }
        }
    }
}

struct ExecCommandBegin {
    command: Vec<String>,
    start_time: chrono::DateTime<Utc>,
}

struct PatchApplyBegin {
    start_time: chrono::DateTime<Utc>,
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
            EventMsg::Error { message } => {
                let prefix = "ERROR:".style(self.red);
                ts_println!("{prefix} {message}");
            }
            EventMsg::BackgroundEvent { message } => {
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
            EventMsg::AgentMessage { message } => {
                let prefix = "Agent message:".style(self.bold);
                ts_println!("{prefix} {message}");
            }
            EventMsg::ExecCommandBegin {
                call_id,
                command,
                cwd,
            } => {
                self.call_id_to_command.insert(
                    call_id.clone(),
                    ExecCommandBegin {
                        command: command.clone(),
                        start_time: Utc::now(),
                    },
                );
                ts_println!(
                    "{} {} in {}",
                    "exec".style(self.magenta),
                    escape_command(&command).style(self.bold),
                    cwd,
                );
            }
            EventMsg::ExecCommandEnd {
                call_id,
                stdout,
                stderr,
                exit_code,
            } => {
                let exec_command = self.call_id_to_command.remove(&call_id);
                let (duration, call) = if let Some(ExecCommandBegin {
                    command,
                    start_time,
                }) = exec_command
                {
                    (
                        format_duration(start_time),
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
                        let title = format!("{call} succeded{duration}:");
                        ts_println!("{}", title.style(self.green));
                    }
                    _ => {
                        let title = format!("{call} exited {exit_code}{duration}:");
                        ts_println!("{}", title.style(self.red));
                    }
                }
                println!("{}", truncated_output.style(self.dimmed));
            }
            EventMsg::PatchApplyBegin {
                call_id,
                auto_approved,
                changes,
            } => {
                // Store metadata so we can calculate duration later when we
                // receive the corresponding PatchApplyEnd event.
                self.call_id_to_patch.insert(
                    call_id.clone(),
                    PatchApplyBegin {
                        start_time: Utc::now(),
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
            EventMsg::PatchApplyEnd {
                call_id,
                stdout,
                stderr,
                success,
            } => {
                let patch_begin = self.call_id_to_patch.remove(&call_id);

                // Compute duration and summary label similar to exec commands.
                let (duration, label) = if let Some(PatchApplyBegin {
                    start_time,
                    auto_approved,
                }) = patch_begin
                {
                    (
                        format_duration(start_time),
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
            EventMsg::ExecApprovalRequest { .. } => {
                // Should we exit?
            }
            EventMsg::ApplyPatchApprovalRequest { .. } => {
                // Should we exit?
            }
            _ => {
                // Ignore event.
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

fn format_duration(start_time: chrono::DateTime<Utc>) -> String {
    let elapsed = Utc::now().signed_duration_since(start_time);
    let millis = elapsed.num_milliseconds();
    if millis < 1000 {
        format!(" in {}ms", millis)
    } else {
        format!(" in {:.2}s", millis as f64 / 1000.0)
    }
}
