use std::time::Duration;
use std::time::Instant;

use codex_core::protocol::ExecCommandSource;
use codex_protocol::parse_command::ParsedCommand;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(tag = "kind")]
pub(crate) enum SubagentCell {
    Spawn {
        label: String,
        #[serde(default)]
        model: Option<String>,
        #[serde(default)]
        summary: Option<String>,
    },
    Fork {
        label: String,
        #[serde(default)]
        model: Option<String>,
        #[serde(default)]
        summary: Option<String>,
    },
    SendMessage {
        label: String,
        #[serde(default)]
        summary: Option<String>,
    },
    List {
        #[serde(default)]
        count: Option<usize>,
    },
    Await {
        #[serde(default)]
        label: Option<String>,
        #[serde(default)]
        timed_out: Option<bool>,
        #[serde(default)]
        message: Option<String>,
        #[serde(default)]
        lifecycle_status: Option<String>,
    },
    Cancel {
        #[serde(default)]
        label: Option<String>,
    },
    Prune {
        #[serde(default)]
        counts: Option<serde_json::Value>,
    },
    Logs {
        #[serde(default)]
        label: Option<String>,
        #[serde(default)]
        rendered: Option<String>,
    },
    Watchdog {
        #[serde(default)]
        action: Option<String>,
        #[serde(default)]
        interval_s: Option<u64>,
        #[serde(default)]
        message: Option<String>,
    },
    Raw {
        text: String,
    },
}

pub(crate) fn parse_subagent_call(command: &[String]) -> Option<SubagentCell> {
    let first = command.first()?;
    let trimmed = first.trim();

    if let Some(rest) = trimmed.strip_prefix("Forked subagent") {
        return Some(SubagentCell::Fork {
            label: rest.trim().to_string(),
            model: None,
            summary: None,
        });
    }
    if let Some(rest) = trimmed.strip_prefix("Spawned subagent") {
        return Some(SubagentCell::Spawn {
            label: rest.trim().to_string(),
            model: None,
            summary: None,
        });
    }
    if let Some(rest) = trimmed.strip_prefix("Sent message to subagent") {
        return Some(SubagentCell::SendMessage {
            label: rest.trim().to_string(),
            summary: None,
        });
    }
    if let Some(rest) = trimmed.strip_prefix("Awaited subagent") {
        return Some(SubagentCell::Await {
            label: Some(rest.trim().to_string()),
            timed_out: None,
            message: None,
            lifecycle_status: None,
        });
    }
    if let Some(rest) = trimmed.strip_prefix("Canceled subagent") {
        return Some(SubagentCell::Cancel {
            label: Some(rest.trim().to_string()),
        });
    }
    if let Some(rest) = trimmed.strip_prefix("Fetched subagent logs") {
        return Some(SubagentCell::Logs {
            label: Some(rest.trim().to_string()),
            rendered: None,
        });
    }
    if trimmed.starts_with("Pruned subagents") {
        return Some(SubagentCell::Prune { counts: None });
    }
    if trimmed.starts_with("Listed subagents") {
        return Some(SubagentCell::List { count: None });
    }
    if trimmed.starts_with("Started watchdog") || trimmed.starts_with("Replaced watchdog") {
        return Some(SubagentCell::Watchdog {
            action: None,
            interval_s: None,
            message: None,
        });
    }

    None
}

pub(crate) fn subagent_from_formatted_output(formatted_output: &str) -> Option<SubagentCell> {
    serde_json::from_str::<serde_json::Value>(formatted_output)
        .ok()
        .and_then(|v| v.get("subagent_render").cloned())
        .and_then(|v| serde_json::from_value::<SubagentCell>(v).ok())
}

#[derive(Clone, Debug, Default)]
pub(crate) struct CommandOutput {
    pub(crate) exit_code: i32,
    /// The aggregated stderr + stdout interleaved.
    pub(crate) aggregated_output: String,
    /// The formatted output of the command, as seen by the model.
    pub(crate) formatted_output: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ExecCall {
    pub(crate) call_id: String,
    pub(crate) command: Vec<String>,
    pub(crate) parsed: Vec<ParsedCommand>,
    pub(crate) subagent: Option<SubagentCell>,
    pub(crate) output: Option<CommandOutput>,
    pub(crate) source: ExecCommandSource,
    pub(crate) is_user_shell_command: bool,
    pub(crate) start_time: Option<Instant>,
    pub(crate) duration: Option<Duration>,
    pub(crate) interaction_input: Option<String>,
}

#[derive(Debug)]
pub(crate) struct ExecCell {
    pub(crate) calls: Vec<ExecCall>,
    animations_enabled: bool,
}

impl ExecCell {
    pub(crate) fn new(call: ExecCall, animations_enabled: bool) -> Self {
        Self {
            calls: vec![call],
            animations_enabled,
        }
    }

    pub(crate) fn with_added_call(
        &self,
        call_id: String,
        command: Vec<String>,
        parsed: Vec<ParsedCommand>,
        source: ExecCommandSource,
        interaction_input: Option<String>,
        is_user_shell_command: bool,
    ) -> Option<Self> {
        let subagent = parse_subagent_call(&command);
        let call = ExecCall {
            call_id,
            command,
            parsed,
            subagent,
            output: None,
            source,
            is_user_shell_command,
            start_time: Some(Instant::now()),
            duration: None,
            interaction_input,
        };
        if self.is_exploring_cell() && Self::is_exploring_call(&call) {
            Some(Self {
                calls: [self.calls.clone(), vec![call]].concat(),
                animations_enabled: self.animations_enabled,
            })
        } else {
            None
        }
    }

    pub(crate) fn complete_call(
        &mut self,
        call_id: &str,
        output: CommandOutput,
        duration: Duration,
    ) {
        if let Some(call) = self.calls.iter_mut().rev().find(|c| c.call_id == call_id) {
            if let Some(subagent) = subagent_from_formatted_output(&output.formatted_output) {
                call.subagent = Some(subagent);
            }
            call.output = Some(output);
            call.duration = Some(duration);
            call.start_time = None;
        }
    }

    pub(crate) fn should_flush(&self) -> bool {
        !self.is_exploring_cell() && self.calls.iter().all(|c| c.output.is_some())
    }

    pub(crate) fn mark_failed(&mut self) {
        for call in self.calls.iter_mut() {
            if call.output.is_none() {
                let elapsed = call
                    .start_time
                    .map(|st| st.elapsed())
                    .unwrap_or_else(|| Duration::from_millis(0));
                call.start_time = None;
                call.duration = Some(elapsed);
                call.output = Some(CommandOutput {
                    exit_code: 1,
                    formatted_output: String::new(),
                    aggregated_output: String::new(),
                });
            }
        }
    }

    pub(crate) fn is_exploring_cell(&self) -> bool {
        self.calls.iter().all(Self::is_exploring_call)
    }

    pub(crate) fn is_subagent_cell(&self) -> bool {
        self.calls
            .iter()
            .all(|c| Self::is_subagent_call(c) && !c.is_user_shell_command)
    }

    pub(crate) fn is_active(&self) -> bool {
        self.calls.iter().any(|c| c.output.is_none())
    }

    pub(crate) fn active_start_time(&self) -> Option<Instant> {
        self.calls
            .iter()
            .find(|c| c.output.is_none())
            .and_then(|c| c.start_time)
    }

    pub(crate) fn animations_enabled(&self) -> bool {
        self.animations_enabled
    }

    pub(crate) fn iter_calls(&self) -> impl Iterator<Item = &ExecCall> {
        self.calls.iter()
    }

    pub(super) fn is_exploring_call(call: &ExecCall) -> bool {
        !matches!(call.source, ExecCommandSource::UserShell)
            && !call.parsed.is_empty()
            && call.parsed.iter().all(|p| {
                matches!(
                    p,
                    ParsedCommand::Read { .. }
                        | ParsedCommand::ListFiles { .. }
                        | ParsedCommand::Search { .. }
                )
            })
    }

    pub(super) fn is_subagent_call(call: &ExecCall) -> bool {
        call.subagent.is_some()
    }
}

impl ExecCall {
    pub(crate) fn is_user_shell_command(&self) -> bool {
        self.is_user_shell_command
    }

    pub(crate) fn is_unified_exec_interaction(&self) -> bool {
        matches!(self.source, ExecCommandSource::UnifiedExecInteraction)
    }
}
