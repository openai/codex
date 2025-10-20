use crate::codex::Session;
use crate::exec::ExecToolCallOutput;
use crate::parse_command::parse_command;
use crate::protocol::Event;
use crate::protocol::EventMsg;
use crate::protocol::ExecCommandBeginEvent;
use crate::protocol::ExecCommandEndEvent;
use crate::protocol::FileChange;
use crate::protocol::PatchApplyBeginEvent;
use crate::protocol::PatchApplyEndEvent;
use crate::protocol::TurnDiffEvent;
use crate::tools::context::SharedTurnDiffTracker;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use super::format_exec_output;
use super::format_exec_output_str;

#[derive(Clone, Copy)]
pub(crate) struct ToolEventCtx<'a> {
    pub session: &'a Session,
    pub sub_id: &'a str,
    pub call_id: &'a str,
    pub turn_diff_tracker: Option<&'a SharedTurnDiffTracker>,
}

impl<'a> ToolEventCtx<'a> {
    pub fn new(
        session: &'a Session,
        sub_id: &'a str,
        call_id: &'a str,
        turn_diff_tracker: Option<&'a SharedTurnDiffTracker>,
    ) -> Self {
        Self {
            session,
            sub_id,
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
// Concrete, allocation-free emitter: avoid trait objects and boxed futures.
pub(crate) enum ToolEmitter {
    Shell {
        command: Vec<String>,
        cwd: PathBuf,
    },
    ApplyPatch {
        changes: HashMap<PathBuf, FileChange>,
        auto_approved: bool,
    },
}

impl ToolEmitter {
    pub fn shell(command: Vec<String>, cwd: PathBuf) -> Self {
        Self::Shell { command, cwd }
    }

    pub fn apply_patch(changes: HashMap<PathBuf, FileChange>, auto_approved: bool) -> Self {
        Self::ApplyPatch {
            changes,
            auto_approved,
        }
    }

    pub async fn emit(&self, ctx: ToolEventCtx<'_>, stage: ToolEventStage) {
        match (self, stage) {
            (Self::Shell { command, cwd }, ToolEventStage::Begin) => {
                ctx.session
                    .send_event(Event {
                        id: ctx.sub_id.to_string(),
                        msg: EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
                            call_id: ctx.call_id.to_string(),
                            command: command.clone(),
                            cwd: cwd.clone(),
                            parsed_cmd: parse_command(command),
                        }),
                    })
                    .await;
            }
            (Self::Shell { .. }, ToolEventStage::Success(output)) => {
                emit_exec_end(
                    ctx,
                    output.stdout.text.clone(),
                    output.stderr.text.clone(),
                    output.aggregated_output.text.clone(),
                    output.exit_code,
                    output.duration,
                    format_exec_output_str(&output),
                )
                .await;
            }
            (Self::Shell { .. }, ToolEventStage::Failure(ToolEventFailure::Output(output))) => {
                emit_exec_end(
                    ctx,
                    output.stdout.text.clone(),
                    output.stderr.text.clone(),
                    output.aggregated_output.text.clone(),
                    output.exit_code,
                    output.duration,
                    format_exec_output_str(&output),
                )
                .await;
            }
            (Self::Shell { .. }, ToolEventStage::Failure(ToolEventFailure::Message(message))) => {
                emit_exec_end(
                    ctx,
                    String::new(),
                    (*message).to_string(),
                    (*message).to_string(),
                    -1,
                    Duration::ZERO,
                    format_exec_output(&message),
                )
                .await;
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
                    .send_event(Event {
                        id: ctx.sub_id.to_string(),
                        msg: EventMsg::PatchApplyBegin(PatchApplyBeginEvent {
                            call_id: ctx.call_id.to_string(),
                            auto_approved: *auto_approved,
                            changes: changes.clone(),
                        }),
                    })
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
        }
    }
}

async fn emit_exec_end(
    ctx: ToolEventCtx<'_>,
    stdout: String,
    stderr: String,
    aggregated_output: String,
    exit_code: i32,
    duration: Duration,
    formatted_output: String,
) {
    ctx.session
        .send_event(Event {
            id: ctx.sub_id.to_string(),
            msg: EventMsg::ExecCommandEnd(ExecCommandEndEvent {
                call_id: ctx.call_id.to_string(),
                stdout,
                stderr,
                aggregated_output,
                exit_code,
                duration,
                formatted_output,
            }),
        })
        .await;
}

async fn emit_patch_end(ctx: ToolEventCtx<'_>, stdout: String, stderr: String, success: bool) {
    ctx.session
        .send_event(Event {
            id: ctx.sub_id.to_string(),
            msg: EventMsg::PatchApplyEnd(PatchApplyEndEvent {
                call_id: ctx.call_id.to_string(),
                stdout,
                stderr,
                success,
            }),
        })
        .await;

    if let Some(tracker) = ctx.turn_diff_tracker {
        let unified_diff = {
            let mut guard = tracker.lock().await;
            guard.get_unified_diff()
        };
        if let Ok(Some(unified_diff)) = unified_diff {
            ctx.session
                .send_event(Event {
                    id: ctx.sub_id.to_string(),
                    msg: EventMsg::TurnDiff(TurnDiffEvent { unified_diff }),
                })
                .await;
        }
    }
}
