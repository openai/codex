use crate::codex::Session;
use crate::codex::TurnContext;
use crate::error::CodexErr;
use crate::error::SandboxErr;
use crate::exec::ExecToolCallOutput;
use crate::function_tool::FunctionCallError;
use crate::parse_command::parse_command;
use crate::protocol::EventMsg;
use crate::protocol::ExecCommandBeginEvent;
use crate::protocol::ExecCommandEndEvent;
use crate::protocol::ExecCommandSource;
use crate::protocol::FileChange;
use crate::protocol::PatchApplyBeginEvent;
use crate::protocol::PatchApplyEndEvent;
use crate::protocol::TurnDiffEvent;
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::sandboxing::ToolError;
use codex_protocol::parse_command::ParsedCommand;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use super::format_exec_output_str;

#[derive(Clone, Copy)]
pub(crate) struct ToolEventCtx<'a> {
    pub session: &'a Session,
    pub turn: &'a TurnContext,
    pub call_id: &'a str,
    pub turn_diff_tracker: Option<&'a SharedTurnDiffTracker>,
}

impl<'a> ToolEventCtx<'a> {
    pub fn new(
        session: &'a Session,
        turn: &'a TurnContext,
        call_id: &'a str,
        turn_diff_tracker: Option<&'a SharedTurnDiffTracker>,
    ) -> Self {
        Self {
            session,
            turn,
            call_id,
            turn_diff_tracker,
        }
    }
}

pub(crate) enum ToolEventStage {
    Begin,
    Success(ExecToolCallOutput),
    Failure(ToolEventFailure),
}

pub(crate) enum ToolEventFailure {
    Output(ExecToolCallOutput),
    Message(String),
}

pub(crate) async fn emit_exec_command_begin(
    ctx: ToolEventCtx<'_>,
    command: &[String],
    cwd: &Path,
    parsed_cmd: &[ParsedCommand],
    source: ExecCommandSource,
    interaction_input: Option<String>,
) {
    ctx.session
        .send_event(
            ctx.turn,
            EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
                call_id: ctx.call_id.to_string(),
                turn_id: ctx.turn.sub_id.clone(),
                command: command.to_vec(),
                cwd: cwd.to_path_buf(),
                parsed_cmd: parsed_cmd.to_vec(),
                source,
                interaction_input,
            }),
        )
        .await;
}
// Concrete, allocation-free emitter: avoid trait objects and boxed futures.
pub(crate) enum ToolEmitter {
    Shell {
        command: Vec<String>,
        cwd: PathBuf,
        source: ExecCommandSource,
        parsed_cmd: Vec<ParsedCommand>,
    },
    ApplyPatch {
        changes: HashMap<PathBuf, FileChange>,
        auto_approved: bool,
    },
    UnifiedExec {
        command: Vec<String>,
        cwd: PathBuf,
        source: ExecCommandSource,
        interaction_input: Option<String>,
        parsed_cmd: Vec<ParsedCommand>,
    },
}

impl ToolEmitter {
    pub fn shell(command: Vec<String>, cwd: PathBuf, source: ExecCommandSource) -> Self {
        let parsed_cmd = parse_command(&command);
        Self::Shell {
            command,
            cwd,
            source,
            parsed_cmd,
        }
    }

    pub fn apply_patch(changes: HashMap<PathBuf, FileChange>, auto_approved: bool) -> Self {
        Self::ApplyPatch {
            changes,
            auto_approved,
        }
    }

    pub fn unified_exec(
        command: &[String],
        cwd: PathBuf,
        source: ExecCommandSource,
        interaction_input: Option<String>,
    ) -> Self {
        let parsed_cmd = parse_command(command);
        Self::UnifiedExec {
            command: command.to_vec(),
            cwd,
            source,
            interaction_input,
            parsed_cmd,
        }
    }

    pub async fn emit(&self, ctx: ToolEventCtx<'_>, stage: ToolEventStage) {
        match (self, stage) {
            (
                Self::Shell {
                    command,
                    cwd,
                    source,
                    parsed_cmd,
                },
                ToolEventStage::Begin,
            ) => {
                emit_exec_command_begin(ctx, command, cwd.as_path(), parsed_cmd, *source, None)
                    .await;
            }
            (
                Self::Shell {
                    command,
                    cwd,
                    source,
                    parsed_cmd,
                },
                ToolEventStage::Success(output),
            ) => {
                let meta = ExecEventMetadata {
                    command,
                    cwd: cwd.as_path(),
                    parsed_cmd,
                    source: *source,
                    interaction_input: None,
                };
                emit_exec_end(ctx, meta, payload_from_output(&output)).await;
            }
            (
                Self::Shell {
                    command,
                    cwd,
                    source,
                    parsed_cmd,
                },
                ToolEventStage::Failure(ToolEventFailure::Output(output)),
            ) => {
                let meta = ExecEventMetadata {
                    command,
                    cwd: cwd.as_path(),
                    parsed_cmd,
                    source: *source,
                    interaction_input: None,
                };
                emit_exec_end(ctx, meta, payload_from_output(&output)).await;
            }
            (
                Self::Shell {
                    command,
                    cwd,
                    source,
                    parsed_cmd,
                },
                ToolEventStage::Failure(ToolEventFailure::Message(message)),
            ) => {
                let meta = ExecEventMetadata {
                    command,
                    cwd: cwd.as_path(),
                    parsed_cmd,
                    source: *source,
                    interaction_input: None,
                };
                let payload = ExecCommandResultPayload {
                    stdout: String::new(),
                    stderr: (*message).to_string(),
                    aggregated_output: (*message).to_string(),
                    exit_code: -1,
                    duration: Duration::ZERO,
                    formatted_output: message.clone(),
                };
                emit_exec_end(ctx, meta, payload).await;
            }

            (
                Self::ApplyPatch {
                    changes,
                    auto_approved,
                },
                ToolEventStage::Begin,
            ) => {
                if let Some(tracker) = ctx.turn_diff_tracker {
                    let mut guard = tracker.lock().await;
                    guard.on_patch_begin(changes);
                }
                ctx.session
                    .send_event(
                        ctx.turn,
                        EventMsg::PatchApplyBegin(PatchApplyBeginEvent {
                            call_id: ctx.call_id.to_string(),
                            auto_approved: *auto_approved,
                            changes: changes.clone(),
                        }),
                    )
                    .await;
            }
            (Self::ApplyPatch { .. }, ToolEventStage::Success(output)) => {
                emit_patch_end(
                    ctx,
                    output.stdout.text.clone(),
                    output.stderr.text.clone(),
                    output.exit_code == 0,
                )
                .await;
            }
            (
                Self::ApplyPatch { .. },
                ToolEventStage::Failure(ToolEventFailure::Output(output)),
            ) => {
                emit_patch_end(
                    ctx,
                    output.stdout.text.clone(),
                    output.stderr.text.clone(),
                    output.exit_code == 0,
                )
                .await;
            }
            (
                Self::ApplyPatch { .. },
                ToolEventStage::Failure(ToolEventFailure::Message(message)),
            ) => {
                emit_patch_end(ctx, String::new(), (*message).to_string(), false).await;
            }
            (
                Self::UnifiedExec {
                    command,
                    cwd,
                    source,
                    interaction_input,
                    parsed_cmd,
                },
                ToolEventStage::Begin,
            ) => {
                emit_exec_command_begin(
                    ctx,
                    command,
                    cwd.as_path(),
                    parsed_cmd,
                    *source,
                    interaction_input.clone(),
                )
                .await;
            }
            (
                Self::UnifiedExec {
                    command,
                    cwd,
                    source,
                    interaction_input,
                    parsed_cmd,
                },
                ToolEventStage::Success(output),
            ) => {
                let meta = ExecEventMetadata {
                    command,
                    cwd: cwd.as_path(),
                    parsed_cmd,
                    source: *source,
                    interaction_input: interaction_input.clone(),
                };
                emit_exec_end(ctx, meta, payload_from_output(&output)).await;
            }
            (
                Self::UnifiedExec {
                    command,
                    cwd,
                    source,
                    interaction_input,
                    parsed_cmd,
                },
                ToolEventStage::Failure(ToolEventFailure::Output(output)),
            ) => {
                let meta = ExecEventMetadata {
                    command,
                    cwd: cwd.as_path(),
                    parsed_cmd,
                    source: *source,
                    interaction_input: interaction_input.clone(),
                };
                emit_exec_end(ctx, meta, payload_from_output(&output)).await;
            }
            (
                Self::UnifiedExec {
                    command,
                    cwd,
                    source,
                    interaction_input,
                    parsed_cmd,
                },
                ToolEventStage::Failure(ToolEventFailure::Message(message)),
            ) => {
                let meta = ExecEventMetadata {
                    command,
                    cwd: cwd.as_path(),
                    parsed_cmd,
                    source: *source,
                    interaction_input: interaction_input.clone(),
                };
                let payload = ExecCommandResultPayload {
                    stdout: String::new(),
                    stderr: (*message).to_string(),
                    aggregated_output: (*message).to_string(),
                    exit_code: -1,
                    duration: Duration::ZERO,
                    formatted_output: message.clone(),
                };
                emit_exec_end(ctx, meta, payload).await;
            }
        }
    }

    pub async fn begin(&self, ctx: ToolEventCtx<'_>) {
        self.emit(ctx, ToolEventStage::Begin).await;
    }

    pub async fn finish(
        &self,
        ctx: ToolEventCtx<'_>,
        out: Result<ExecToolCallOutput, ToolError>,
    ) -> Result<String, FunctionCallError> {
        let (event, result) = match out {
            Ok(output) => {
                let content = super::format_exec_output_for_model(&output);
                let exit_code = output.exit_code;
                let event = ToolEventStage::Success(output);
                let result = if exit_code == 0 {
                    Ok(content)
                } else {
                    Err(FunctionCallError::RespondToModel(content))
                };
                (event, result)
            }
            Err(ToolError::Codex(CodexErr::Sandbox(SandboxErr::Timeout { output })))
            | Err(ToolError::Codex(CodexErr::Sandbox(SandboxErr::Denied { output }))) => {
                let response = super::format_exec_output_for_model(&output);
                let event = ToolEventStage::Failure(ToolEventFailure::Output(*output));
                let result = Err(FunctionCallError::RespondToModel(response));
                (event, result)
            }
            Err(ToolError::Codex(err)) => {
                let message = format!("execution error: {err:?}");
                let event = ToolEventStage::Failure(ToolEventFailure::Message(message.clone()));
                let result = Err(FunctionCallError::RespondToModel(message));
                (event, result)
            }
            Err(ToolError::Rejected(msg)) => {
                // Normalize common rejection messages for exec tools so tests and
                // users see a clear, consistent phrase.
                let normalized = if msg == "rejected by user" {
                    "exec command rejected by user".to_string()
                } else {
                    msg
                };
                let event = ToolEventStage::Failure(ToolEventFailure::Message(normalized.clone()));
                let result = Err(FunctionCallError::RespondToModel(normalized));
                (event, result)
            }
        };
        self.emit(ctx, event).await;
        result
    }
}

struct ExecEventMetadata<'a> {
    command: &'a [String],
    cwd: &'a Path,
    parsed_cmd: &'a [ParsedCommand],
    source: ExecCommandSource,
    interaction_input: Option<String>,
}

struct ExecCommandResultPayload {
    stdout: String,
    stderr: String,
    aggregated_output: String,
    exit_code: i32,
    duration: Duration,
    formatted_output: String,
}

fn payload_from_output(output: &ExecToolCallOutput) -> ExecCommandResultPayload {
    ExecCommandResultPayload {
        stdout: output.stdout.text.clone(),
        stderr: output.stderr.text.clone(),
        aggregated_output: output.aggregated_output.text.clone(),
        exit_code: output.exit_code,
        duration: output.duration,
        formatted_output: format_exec_output_str(output),
    }
}

async fn emit_exec_end(
    ctx: ToolEventCtx<'_>,
    meta: ExecEventMetadata<'_>,
    payload: ExecCommandResultPayload,
) {
    ctx.session
        .send_event(
            ctx.turn,
            EventMsg::ExecCommandEnd(ExecCommandEndEvent {
                call_id: ctx.call_id.to_string(),
                turn_id: ctx.turn.sub_id.clone(),
                command: meta.command.to_vec(),
                cwd: meta.cwd.to_path_buf(),
                parsed_cmd: meta.parsed_cmd.to_vec(),
                source: meta.source,
                interaction_input: meta.interaction_input,
                stdout: payload.stdout,
                stderr: payload.stderr,
                aggregated_output: payload.aggregated_output,
                exit_code: payload.exit_code,
                duration: payload.duration,
                formatted_output: payload.formatted_output,
            }),
        )
        .await;
}

async fn emit_patch_end(ctx: ToolEventCtx<'_>, stdout: String, stderr: String, success: bool) {
    ctx.session
        .send_event(
            ctx.turn,
            EventMsg::PatchApplyEnd(PatchApplyEndEvent {
                call_id: ctx.call_id.to_string(),
                stdout,
                stderr,
                success,
            }),
        )
        .await;

    if let Some(tracker) = ctx.turn_diff_tracker {
        let unified_diff = {
            let mut guard = tracker.lock().await;
            guard.get_unified_diff()
        };
        if let Ok(Some(unified_diff)) = unified_diff {
            ctx.session
                .send_event(ctx.turn, EventMsg::TurnDiff(TurnDiffEvent { unified_diff }))
                .await;
        }
    }
}
