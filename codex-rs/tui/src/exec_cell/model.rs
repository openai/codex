//! Data model for exec history cells shown in the transcript.
//!
//! An exec cell groups one or more command executions that should render together in the chat
//! transcript. Each execution is represented as an [`ExecCall`] with optional output once the
//! command completes. The cell also keeps track of whether animations are enabled so rendering can
//! opt into live updates without duplicating the state.
//!
//! "Exploring" calls are a subset of execs (read/list/search actions that are not user shell
//! commands). Consecutive exploring calls may be coalesced into the same cell so the transcript
//! presents a single exploratory block rather than a sequence of individual commands.
//!
//! The exec cell owns its calls and timestamps. Call completion clears the start time and fills in
//! the duration/output fields; callers should treat those fields as mutually exclusive with
//! "in-flight" state.

use std::time::Duration;
use std::time::Instant;

use codex_core::protocol::ExecCommandSource;
use codex_protocol::parse_command::ParsedCommand;

/// Captures a command's output once it finishes.
///
/// This struct stores both the raw interleaved output and the formatted
/// representation shown to the model, allowing rendering to choose the
/// appropriate view.
#[derive(Clone, Debug, Default)]
pub(crate) struct CommandOutput {
    /// Exit status reported by the command.
    pub(crate) exit_code: i32,

    /// The aggregated stderr + stdout interleaved.
    pub(crate) aggregated_output: String,

    /// The formatted output of the command, as seen by the model.
    pub(crate) formatted_output: String,
}

/// Records a single exec tool call and its eventual output.
///
/// The call starts with `output` unset and `start_time` populated. Completion
/// fills in `output` and `duration`, while clearing `start_time`.
#[derive(Debug, Clone)]
pub(crate) struct ExecCall {
    /// Unique identifier for the tool call, used to match begin/end events.
    pub(crate) call_id: String,

    /// The raw command tokens as invoked.
    pub(crate) command: Vec<String>,

    /// Parsed command classification used to detect "exploring" calls.
    pub(crate) parsed: Vec<ParsedCommand>,

    /// Output captured after completion; `None` while the call is running.
    pub(crate) output: Option<CommandOutput>,

    /// Source of the command (user shell, unified exec, etc.).
    pub(crate) source: ExecCommandSource,

    /// Start time for in-flight calls; cleared when the call completes.
    pub(crate) start_time: Option<Instant>,

    /// Duration of the call once complete.
    pub(crate) duration: Option<Duration>,

    /// User-provided input associated with a unified exec interaction.
    pub(crate) interaction_input: Option<String>,
}

/// Groups one or more exec calls into a single transcript cell.
///
/// Cells are appended to the transcript in order, but exploratory calls may be
/// merged to reduce clutter. The cell tracks whether the UI should animate
/// active commands during rendering.
#[derive(Debug)]
pub(crate) struct ExecCell {
    /// Ordered list of exec calls belonging to this cell.
    pub(crate) calls: Vec<ExecCall>,

    /// Whether the UI should render animated progress for in-flight calls.
    animations_enabled: bool,
}

impl ExecCell {
    /// Create a new exec cell with a single call.
    ///
    /// The initial call is treated as in-flight and will be completed via
    /// [`ExecCell::complete_call`] once output arrives.
    pub(crate) fn new(call: ExecCall, animations_enabled: bool) -> Self {
        Self {
            calls: vec![call],
            animations_enabled,
        }
    }

    /// Returns a new cell with `call` appended when both calls are "exploring".
    ///
    /// This allows consecutive exploratory commands to render as a single transcript block.
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

    /// Mark a call as complete with its final output and duration.
    ///
    /// The most recent matching call is updated to handle repeated call IDs in
    /// merged exploratory cells.
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

    /// Returns true once the cell is complete and safe to flush to history.
    ///
    /// Exploratory cells stay in the live view until they are merged into a
    /// non-exploring cell by the caller.
    pub(crate) fn should_flush(&self) -> bool {
        !self.is_exploring_cell() && self.calls.iter().all(|c| c.output.is_some())
    }

    /// Mark any in-flight calls as failed with empty output.
    ///
    /// This is used when the exec stream ends unexpectedly so the transcript
    /// does not leave hanging in-flight rows.
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

    /// Returns true if all calls in the cell are "exploring".
    pub(crate) fn is_exploring_cell(&self) -> bool {
        self.calls.iter().all(Self::is_exploring_call)
    }

    /// Returns true if any call in the cell is still running.
    pub(crate) fn is_active(&self) -> bool {
        self.calls.iter().any(|c| c.output.is_none())
    }

    /// Returns the start time of the first still-running call, if any.
    pub(crate) fn active_start_time(&self) -> Option<Instant> {
        self.calls
            .iter()
            .find(|c| c.output.is_none())
            .and_then(|c| c.start_time)
    }

    /// Returns whether the cell should animate while rendering.
    pub(crate) fn animations_enabled(&self) -> bool {
        self.animations_enabled
    }

    /// Iterate over calls in chronological order.
    pub(crate) fn iter_calls(&self) -> impl Iterator<Item = &ExecCall> {
        self.calls.iter()
    }

    /// Append a raw output chunk to the matching call's aggregated output.
    ///
    /// Returns true when the chunk was appended to an active call, or false if the call is not
    /// found or the chunk is empty.
    pub(crate) fn append_output(&mut self, call_id: &str, chunk: &str) -> bool {
        if chunk.is_empty() {
            return false;
        }
        let Some(call) = self.calls.iter_mut().rev().find(|c| c.call_id == call_id) else {
            return false;
        };
        let output = call.output.get_or_insert_with(CommandOutput::default);
        output.aggregated_output.push_str(chunk);
        true
    }

    /// Returns true if the call represents a non-user exploratory read/list/search.
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
    /// Returns true when the call originated from a direct user shell command.
    pub(crate) fn is_user_shell_command(&self) -> bool {
        matches!(self.source, ExecCommandSource::UserShell)
    }

    /// Returns true when the call is a unified exec interaction prompt.
    pub(crate) fn is_unified_exec_interaction(&self) -> bool {
        matches!(self.source, ExecCommandSource::UnifiedExecInteraction)
    }
}
