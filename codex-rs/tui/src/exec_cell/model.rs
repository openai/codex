use std::time::Duration;
use std::time::Instant;

use codex_core::protocol::ExecCommandSource;
use codex_protocol::parse_command::ParsedCommand;

/// Output captured from a completed exec call, including exit code and combined streams.
#[derive(Clone, Debug, Default)]
pub(crate) struct CommandOutput {
    /// The exit status returned by the command.
    pub(crate) exit_code: i32,
    /// The aggregated stderr + stdout interleaved.
    pub(crate) aggregated_output: String,
    /// The formatted output of the command, as seen by the model.
    pub(crate) formatted_output: String,
}

/// Single exec invocation (shell or tool) as it flows through the history cell.
#[derive(Debug, Clone)]
pub(crate) struct ExecCall {
    pub(crate) call_id: String,
    pub(crate) command: Vec<String>,
    pub(crate) parsed: Vec<ParsedCommand>,
    pub(crate) output: Option<CommandOutput>,
    pub(crate) source: ExecCommandSource,
    pub(crate) start_time: Option<Instant>,
    pub(crate) duration: Option<Duration>,
    pub(crate) interaction_input: Option<String>,
}

/// History cell that renders exec/search/read calls with status and wrapped output.
///
/// Exploring calls collapse search/read/list steps under an "Exploring"/"Explored" header with a
/// spinner or bullet. Non-exploration runs render a status bullet plus wrapped command, then a
/// tree-prefixed output block that truncates middle lines when necessary.
///
/// # Output
///
/// ```plain
/// • Ran bash -lc "rg term"
///   │ Search shimmer_spans in .
///   └ (no output)
/// ```
#[derive(Debug)]
pub(crate) struct ExecCell {
    pub(crate) calls: Vec<ExecCall>,
    animations_enabled: bool,
}

impl ExecCell {
    /// Create a new cell with a single active call and control over spinner animation.
    pub(crate) fn new(call: ExecCall, animations_enabled: bool) -> Self {
        Self {
            calls: vec![call],
            animations_enabled,
        }
    }

    /// Append an additional exploring call to the cell if it belongs to the same batch.
    ///
    /// Exploring calls render together (search/list/read), so when a new call is also exploring we
    /// coalesce it into the existing cell to avoid noisy standalone entries.
    pub(crate) fn with_added_call(
        &self,
        call_id: String,
        command: Vec<String>,
        parsed: Vec<ParsedCommand>,
        source: ExecCommandSource,
        interaction_input: Option<String>,
    ) -> Option<Self> {
        let call = ExecCall {
            call_id,
            command,
            parsed,
            output: None,
            source,
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

    /// Mark a call as completed with captured output and duration, replacing any spinner.
    pub(crate) fn complete_call(
        &mut self,
        call_id: &str,
        output: CommandOutput,
        duration: Duration,
    ) {
        if let Some(call) = self.calls.iter_mut().rev().find(|c| c.call_id == call_id) {
            call.output = Some(output);
            call.duration = Some(duration);
            call.start_time = None;
        }
    }

    /// Return true when the cell has only exploring calls and every call has finished.
    pub(crate) fn should_flush(&self) -> bool {
        !self.is_exploring_cell() && self.calls.iter().all(|c| c.output.is_some())
    }

    /// Mark in-flight calls as failed, preserving how long they were running.
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

    /// Whether all calls are exploratory (search/list/read) and should render together.
    pub(crate) fn is_exploring_cell(&self) -> bool {
        self.calls.iter().all(Self::is_exploring_call)
    }

    /// True if any call is still active.
    pub(crate) fn is_active(&self) -> bool {
        self.calls.iter().any(|c| c.output.is_none())
    }

    /// Start time of the first active call, used to drive spinners.
    pub(crate) fn active_start_time(&self) -> Option<Instant> {
        self.calls
            .iter()
            .find(|c| c.output.is_none())
            .and_then(|c| c.start_time)
    }

    /// Whether animated spinners are enabled for active calls.
    pub(crate) fn animations_enabled(&self) -> bool {
        self.animations_enabled
    }

    /// Iterate over contained calls in order for rendering.
    pub(crate) fn iter_calls(&self) -> impl Iterator<Item = &ExecCall> {
        self.calls.iter()
    }

    /// Detect whether a call is exploratory (read/list/search) for coalescing.
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
}

impl ExecCall {
    /// Whether the invocation originated from a user shell command.
    pub(crate) fn is_user_shell_command(&self) -> bool {
        matches!(self.source, ExecCommandSource::UserShell)
    }

    /// Whether the invocation expects user input back (unified exec interaction).
    pub(crate) fn is_unified_exec_interaction(&self) -> bool {
        matches!(self.source, ExecCommandSource::UnifiedExecInteraction)
    }
}
