use crate::function_tool::FunctionCallError;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::sandboxing::ToolError;
use crate::turn_timing::now_unix_timestamp_ms;
use codex_apply_patch::AppliedPatchDelta;
use codex_protocol::error::CodexErr;
use codex_protocol::error::SandboxErr;
use codex_protocol::exec_output::ExecToolCallOutput;
use codex_protocol::items::FileChangeItem;
use codex_protocol::items::TurnItem;
use codex_protocol::parse_command::ParsedCommand;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecCommandBeginEvent;
use codex_protocol::protocol::ExecCommandEndEvent;
use codex_protocol::protocol::ExecCommandSource;
use codex_protocol::protocol::ExecCommandStatus;
use codex_protocol::protocol::FileChange;
use codex_protocol::protocol::PatchApplyStatus;
use codex_protocol::protocol::TurnDiffEvent;
use codex_shell_command::parse_command::parse_command;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::HashMap;
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

pub(crate) enum ToolEventStage<'a> {
    Begin,
    Success {
        output: ExecToolCallOutput,
        applied_patch_delta: Option<&'a AppliedPatchDelta>,
    },
    Failure(ToolEventFailure<'a>),
}

pub(crate) enum ToolEventFailure<'a> {
    Output(ExecToolCallOutput),
    Message(String),
    Rejected {
        message: String,
        applied_patch_delta: Option<&'a AppliedPatchDelta>,
    },
}

enum TurnDiffTrackerUpdate<'a> {
    Track(&'a AppliedPatchDelta),
    Invalidate,
    None,
}

fn tracker_update_for_known_delta(delta: &AppliedPatchDelta) -> TurnDiffTrackerUpdate<'_> {
    if delta.is_exact() && delta.is_empty() {
        TurnDiffTrackerUpdate::None
    } else {
        TurnDiffTrackerUpdate::Track(delta)
    }
}

pub(crate) async fn emit_exec_command_begin(
    ctx: ToolEventCtx<'_>,
    command: &[String],
    cwd: &AbsolutePathBuf,
    parsed_cmd: &[ParsedCommand],
    source: ExecCommandSource,
    interaction_input: Option<String>,
    process_id: Option<&str>,
) {
    ctx.session
        .send_event(
            ctx.turn,
            EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
                call_id: ctx.call_id.to_string(),
                process_id: process_id.map(str::to_owned),
                turn_id: ctx.turn.sub_id.clone(),
                started_at_ms: now_unix_timestamp_ms(),
                command: command.to_vec(),
                cwd: cwd.clone(),
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
        cwd: AbsolutePathBuf,
        source: ExecCommandSource,
        parsed_cmd: Vec<ParsedCommand>,
        freeform: bool,
    },
    ApplyPatch {
        changes: HashMap<PathBuf, FileChange>,
        auto_approved: bool,
    },
    UnifiedExec {
        command: Vec<String>,
        cwd: AbsolutePathBuf,
        source: ExecCommandSource,
        parsed_cmd: Vec<ParsedCommand>,
        process_id: Option<String>,
    },
}

impl ToolEmitter {
    pub fn shell(
        command: Vec<String>,
        cwd: AbsolutePathBuf,
        source: ExecCommandSource,
        freeform: bool,
    ) -> Self {
        let parsed_cmd = parse_command(&command);
        Self::Shell {
            command,
            cwd,
            source,
            parsed_cmd,
            freeform,
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
        cwd: AbsolutePathBuf,
        source: ExecCommandSource,
        process_id: Option<String>,
    ) -> Self {
        let parsed_cmd = parse_command(command);
        Self::UnifiedExec {
            command: command.to_vec(),
            cwd,
            source,
            parsed_cmd,
            process_id,
        }
    }

    pub async fn emit(&self, ctx: ToolEventCtx<'_>, stage: ToolEventStage<'_>) {
        match (self, stage) {
            (
                Self::Shell {
                    command,
                    cwd,
                    source,
                    parsed_cmd,
                    ..
                },
                stage,
            ) => {
                emit_exec_stage(
                    ctx,
                    ExecCommandInput::new(
                        command, cwd, parsed_cmd, *source, /*interaction_input*/ None,
                        /*process_id*/ None,
                    ),
                    stage,
                )
                .await;
            }

            (
                Self::ApplyPatch {
                    changes,
                    auto_approved,
                    ..
                },
                ToolEventStage::Begin,
            ) => {
                ctx.session
                    .emit_turn_item_started(
                        ctx.turn,
                        &TurnItem::FileChange(FileChangeItem {
                            id: ctx.call_id.to_string(),
                            changes: changes.clone(),
                            status: None,
                            auto_approved: Some(*auto_approved),
                            stdout: None,
                            stderr: None,
                        }),
                    )
                    .await;
            }
            (
                Self::ApplyPatch { changes, .. },
                ToolEventStage::Success {
                    output,
                    applied_patch_delta,
                },
            ) => {
                let status = if output.exit_code == 0 {
                    PatchApplyStatus::Completed
                } else {
                    PatchApplyStatus::Failed
                };
                let tracker_update = applied_patch_delta
                    .map(tracker_update_for_known_delta)
                    .unwrap_or(TurnDiffTrackerUpdate::Invalidate);
                emit_patch_end(
                    ctx,
                    changes.clone(),
                    output.stdout.text.clone(),
                    output.stderr.text.clone(),
                    status,
                    tracker_update,
                )
                .await;
            }
            (
                Self::ApplyPatch { changes, .. },
                ToolEventStage::Failure(ToolEventFailure::Output(output)),
            ) => {
                emit_patch_end(
                    ctx,
                    changes.clone(),
                    output.stdout.text.clone(),
                    output.stderr.text.clone(),
                    if output.exit_code == 0 {
                        PatchApplyStatus::Completed
                    } else {
                        PatchApplyStatus::Failed
                    },
                    TurnDiffTrackerUpdate::Invalidate,
                )
                .await;
            }
            (
                Self::ApplyPatch { changes, .. },
                ToolEventStage::Failure(ToolEventFailure::Message(message)),
            ) => {
                emit_patch_end(
                    ctx,
                    changes.clone(),
                    String::new(),
                    (*message).to_string(),
                    PatchApplyStatus::Failed,
                    TurnDiffTrackerUpdate::None,
                )
                .await;
            }
            (
                Self::ApplyPatch { changes, .. },
                ToolEventStage::Failure(ToolEventFailure::Rejected {
                    message,
                    applied_patch_delta,
                }),
            ) => {
                emit_patch_end(
                    ctx,
                    changes.clone(),
                    String::new(),
                    (*message).to_string(),
                    PatchApplyStatus::Declined,
                    applied_patch_delta
                        .map(tracker_update_for_known_delta)
                        .unwrap_or(TurnDiffTrackerUpdate::None),
                )
                .await;
            }
            (
                Self::UnifiedExec {
                    command,
                    cwd,
                    source,
                    parsed_cmd,
                    process_id,
                },
                stage,
            ) => {
                emit_exec_stage(
                    ctx,
                    ExecCommandInput::new(
                        command,
                        cwd,
                        parsed_cmd,
                        *source,
                        /*interaction_input*/ None,
                        process_id.as_deref(),
                    ),
                    stage,
                )
                .await;
            }
        }
    }

    pub async fn begin(&self, ctx: ToolEventCtx<'_>) {
        self.emit(ctx, ToolEventStage::Begin).await;
    }

    fn format_exec_output_for_model(
        &self,
        output: &ExecToolCallOutput,
        ctx: ToolEventCtx<'_>,
    ) -> String {
        match self {
            Self::Shell { freeform: true, .. } => {
                super::format_exec_output_for_model_freeform(output, ctx.turn.truncation_policy)
            }
            _ => super::format_exec_output_for_model_structured(output, ctx.turn.truncation_policy),
        }
    }

    pub async fn finish(
        &self,
        ctx: ToolEventCtx<'_>,
        out: Result<ExecToolCallOutput, ToolError>,
        applied_patch_delta: Option<&AppliedPatchDelta>,
    ) -> Result<String, FunctionCallError> {
        let (event, result) = match out {
            Ok(output) => {
                let content = self.format_exec_output_for_model(&output, ctx);
                let exit_code = output.exit_code;
                let event = ToolEventStage::Success {
                    output,
                    applied_patch_delta,
                };
                let result = if exit_code == 0 {
                    Ok(content)
                } else {
                    Err(FunctionCallError::RespondToModel(content))
                };
                (event, result)
            }
            Err(ToolError::Codex(CodexErr::Sandbox(SandboxErr::Timeout { output }))) => {
                let response = self.format_exec_output_for_model(&output, ctx);
                let event = ToolEventStage::Failure(ToolEventFailure::Output(*output));
                let result = Err(FunctionCallError::RespondToModel(response));
                (event, result)
            }
            Err(ToolError::Codex(CodexErr::Sandbox(SandboxErr::Denied { output, .. }))) => {
                let response = self.format_exec_output_for_model(&output, ctx);
                // apply_patch can be denied after it has already committed a
                // known prefix. Reuse the output-bearing path so the visible
                // item still fails while the turn diff consumes that prefix.
                let event = match (self, applied_patch_delta) {
                    (Self::ApplyPatch { .. }, Some(delta)) => ToolEventStage::Success {
                        output: *output,
                        applied_patch_delta: Some(delta),
                    },
                    _ => ToolEventStage::Failure(ToolEventFailure::Output(*output)),
                };
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
                //
                // NOTE: ToolError::Rejected is currently used for both user-declined approvals
                // and some operational/runtime rejection paths (for example setup failures).
                // We intentionally map all of them through the "rejected" event path for now,
                // which means a subset of non-user failures may be reported as Declined.
                //
                // TODO: We should add a new ToolError variant for user-declined approvals.
                let normalized = if msg == "rejected by user" {
                    match self {
                        Self::Shell { .. } | Self::UnifiedExec { .. } => {
                            "exec command rejected by user".to_string()
                        }
                        Self::ApplyPatch { .. } => "patch rejected by user".to_string(),
                    }
                } else {
                    msg
                };
                let event = ToolEventStage::Failure(ToolEventFailure::Rejected {
                    message: normalized.clone(),
                    applied_patch_delta,
                });
                let result = Err(FunctionCallError::RespondToModel(normalized));
                (event, result)
            }
        };
        self.emit(ctx, event).await;
        result
    }
}

struct ExecCommandInput<'a> {
    command: &'a [String],
    cwd: &'a AbsolutePathBuf,
    parsed_cmd: &'a [ParsedCommand],
    source: ExecCommandSource,
    interaction_input: Option<&'a str>,
    process_id: Option<&'a str>,
}

impl<'a> ExecCommandInput<'a> {
    fn new(
        command: &'a [String],
        cwd: &'a AbsolutePathBuf,
        parsed_cmd: &'a [ParsedCommand],
        source: ExecCommandSource,
        interaction_input: Option<&'a str>,
        process_id: Option<&'a str>,
    ) -> Self {
        Self {
            command,
            cwd,
            parsed_cmd,
            source,
            interaction_input,
            process_id,
        }
    }
}

struct ExecCommandResult {
    stdout: String,
    stderr: String,
    aggregated_output: String,
    exit_code: i32,
    duration: Duration,
    formatted_output: String,
    status: ExecCommandStatus,
}

async fn emit_exec_stage(
    ctx: ToolEventCtx<'_>,
    exec_input: ExecCommandInput<'_>,
    stage: ToolEventStage<'_>,
) {
    match stage {
        ToolEventStage::Begin => {
            emit_exec_command_begin(
                ctx,
                exec_input.command,
                exec_input.cwd,
                exec_input.parsed_cmd,
                exec_input.source,
                exec_input.interaction_input.map(str::to_owned),
                exec_input.process_id,
            )
            .await;
        }
        ToolEventStage::Success { output, .. }
        | ToolEventStage::Failure(ToolEventFailure::Output(output)) => {
            let exec_result = ExecCommandResult {
                stdout: output.stdout.text.clone(),
                stderr: output.stderr.text.clone(),
                aggregated_output: output.aggregated_output.text.clone(),
                exit_code: output.exit_code,
                duration: output.duration,
                formatted_output: format_exec_output_str(&output, ctx.turn.truncation_policy),
                status: if output.exit_code == 0 {
                    ExecCommandStatus::Completed
                } else {
                    ExecCommandStatus::Failed
                },
            };
            emit_exec_end(ctx, exec_input, exec_result).await;
        }
        ToolEventStage::Failure(ToolEventFailure::Message(message)) => {
            let text = message.to_string();
            let exec_result = ExecCommandResult {
                stdout: String::new(),
                stderr: text.clone(),
                aggregated_output: text.clone(),
                exit_code: -1,
                duration: Duration::ZERO,
                formatted_output: text,
                status: ExecCommandStatus::Failed,
            };
            emit_exec_end(ctx, exec_input, exec_result).await;
        }
        ToolEventStage::Failure(ToolEventFailure::Rejected { message, .. }) => {
            let text = message.to_string();
            let exec_result = ExecCommandResult {
                stdout: String::new(),
                stderr: text.clone(),
                aggregated_output: text.clone(),
                exit_code: -1,
                duration: Duration::ZERO,
                formatted_output: text,
                status: ExecCommandStatus::Declined,
            };
            emit_exec_end(ctx, exec_input, exec_result).await;
        }
    }
}

async fn emit_exec_end(
    ctx: ToolEventCtx<'_>,
    exec_input: ExecCommandInput<'_>,
    exec_result: ExecCommandResult,
) {
    ctx.session
        .send_event(
            ctx.turn,
            EventMsg::ExecCommandEnd(ExecCommandEndEvent {
                call_id: ctx.call_id.to_string(),
                process_id: exec_input.process_id.map(str::to_owned),
                turn_id: ctx.turn.sub_id.clone(),
                completed_at_ms: now_unix_timestamp_ms(),
                command: exec_input.command.to_vec(),
                cwd: exec_input.cwd.clone(),
                parsed_cmd: exec_input.parsed_cmd.to_vec(),
                source: exec_input.source,
                interaction_input: exec_input.interaction_input.map(str::to_owned),
                stdout: exec_result.stdout,
                stderr: exec_result.stderr,
                aggregated_output: exec_result.aggregated_output,
                exit_code: exec_result.exit_code,
                duration: exec_result.duration,
                formatted_output: exec_result.formatted_output,
                status: exec_result.status,
            }),
        )
        .await;
}

async fn emit_patch_end(
    ctx: ToolEventCtx<'_>,
    changes: HashMap<PathBuf, FileChange>,
    stdout: String,
    stderr: String,
    status: PatchApplyStatus,
    tracker_update: TurnDiffTrackerUpdate<'_>,
) {
    ctx.session
        .emit_turn_item_completed(
            ctx.turn,
            TurnItem::FileChange(FileChangeItem {
                id: ctx.call_id.to_string(),
                changes,
                status: Some(status),
                auto_approved: None,
                stdout: Some(stdout),
                stderr: Some(stderr),
            }),
        )
        .await;

    if let Some(tracker) = ctx.turn_diff_tracker {
        let (should_emit_turn_diff, unified_diff) = {
            let mut guard = tracker.lock().await;
            let previous_diff = guard.get_unified_diff();
            let tracker_changed = match tracker_update {
                TurnDiffTrackerUpdate::Track(delta) => {
                    guard.track_delta(delta);
                    true
                }
                TurnDiffTrackerUpdate::Invalidate => {
                    guard.invalidate();
                    true
                }
                TurnDiffTrackerUpdate::None => false,
            };
            let unified_diff = guard.get_unified_diff();
            (
                tracker_changed && (previous_diff.is_some() || unified_diff.is_some()),
                unified_diff.unwrap_or_default(),
            )
        };
        if should_emit_turn_diff {
            ctx.session
                .send_event(ctx.turn, EventMsg::TurnDiff(TurnDiffEvent { unified_diff }))
                .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::tests::make_session_and_context_with_dynamic_tools_and_rx;
    use crate::turn_diff_tracker::TurnDiffTracker;
    use codex_exec_server::LOCAL_FS;
    use codex_protocol::error::CodexErr;
    use codex_protocol::error::SandboxErr;
    use codex_protocol::exec_output::ExecToolCallOutput;
    use codex_protocol::items::TurnItem;
    use codex_protocol::protocol::PatchApplyStatus;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use std::sync::Arc;
    use tempfile::tempdir;
    use tokio::sync::Mutex;

    async fn assert_failed_apply_patch_tracks_committed_delta(
        out: Result<ExecToolCallOutput, ToolError>,
        expected_status: PatchApplyStatus,
    ) {
        let (session, turn, rx_event) =
            make_session_and_context_with_dynamic_tools_and_rx(Vec::new()).await;
        let tracker = Arc::new(Mutex::new(TurnDiffTracker::new()));
        let dir = tempdir().expect("tempdir");
        let cwd = AbsolutePathBuf::from_absolute_path(dir.path()).expect("absolute cwd");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let delta = codex_apply_patch::apply_patch(
            "*** Begin Patch\n*** Add File: out/dest.txt\n+after\n*** End Patch",
            &cwd,
            &mut stdout,
            &mut stderr,
            LOCAL_FS.as_ref(),
            /*sandbox*/ None,
        )
        .await
        .expect("apply patch");

        ToolEmitter::apply_patch(HashMap::new(), /*auto_approved*/ false)
            .finish(
                ToolEventCtx::new(session.as_ref(), turn.as_ref(), "call-id", Some(&tracker)),
                out,
                Some(&delta),
            )
            .await
            .expect_err("failed patch");

        let completed = rx_event.recv().await.expect("item completed event");
        assert!(matches!(
            completed.msg,
            EventMsg::ItemCompleted(event)
                if matches!(
                    &event.item,
                    TurnItem::FileChange(FileChangeItem {
                        status: Some(status),
                        ..
                    }) if status == &expected_status
                )
        ));

        let unified_diff = loop {
            let event = tokio::time::timeout(Duration::from_secs(1), rx_event.recv())
                .await
                .expect("turn diff event")
                .expect("channel open");
            if let EventMsg::TurnDiff(TurnDiffEvent { unified_diff }) = event.msg {
                break unified_diff;
            }
        };
        assert!(unified_diff.contains("out/dest.txt"));
        assert!(unified_diff.contains("+after"));
    }

    #[tokio::test]
    async fn denied_apply_patch_tracks_committed_delta() {
        let output = ExecToolCallOutput {
            exit_code: 1,
            ..Default::default()
        };
        assert_failed_apply_patch_tracks_committed_delta(
            Err(ToolError::Codex(CodexErr::Sandbox(SandboxErr::Denied {
                output: Box::new(output),
                network_policy_decision: None,
            }))),
            PatchApplyStatus::Failed,
        )
        .await;
    }

    #[tokio::test]
    async fn rejected_apply_patch_tracks_committed_delta() {
        assert_failed_apply_patch_tracks_committed_delta(
            Err(ToolError::Rejected("rejected by user".to_string())),
            PatchApplyStatus::Declined,
        )
        .await;
    }
}
