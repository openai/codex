pub mod context;
pub(crate) mod handlers;
pub mod registry;
pub mod router;
pub mod spec;

use crate::apply_patch;
use crate::apply_patch::ApplyPatchExec;
use crate::apply_patch::InternalApplyPatchInvocation;
use crate::apply_patch::convert_apply_patch_to_protocol;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::error::CodexErr;
use crate::error::SandboxErr;
use crate::exec::ExecParams;
use crate::exec::ExecToolCallOutput;
use crate::exec::StdoutStream;
use crate::executor::ExecutionMode;
use crate::executor::errors::ExecError;
use crate::executor::linkers::PreparedExec;
use crate::function_tool::FunctionCallError;
use crate::tools::context::ApplyPatchCommandContext;
use crate::tools::context::ExecCommandContext;
use crate::turn_diff_tracker::TurnDiffTracker;
use codex_apply_patch::MaybeApplyPatchVerified;
use codex_apply_patch::maybe_parse_apply_patch_verified;
use codex_protocol::protocol::AskForApproval;
use codex_utils_string::take_bytes_at_char_boundary;
use codex_utils_string::take_last_bytes_at_char_boundary;
pub use router::ToolRouter;
use serde::Serialize;
use tracing::trace;

// Model-formatting limits: clients get full streams; only content sent to the model is truncated.
pub(crate) const MODEL_FORMAT_MAX_BYTES: usize = 10 * 1024; // 10 KiB
pub(crate) const MODEL_FORMAT_MAX_LINES: usize = 256; // lines
pub(crate) const MODEL_FORMAT_HEAD_LINES: usize = MODEL_FORMAT_MAX_LINES / 2;
pub(crate) const MODEL_FORMAT_TAIL_LINES: usize = MODEL_FORMAT_MAX_LINES - MODEL_FORMAT_HEAD_LINES; // 128
pub(crate) const MODEL_FORMAT_HEAD_BYTES: usize = MODEL_FORMAT_MAX_BYTES / 2;

// Telemetry preview limits: keep log events smaller than model budgets.
pub(crate) const TELEMETRY_PREVIEW_MAX_BYTES: usize = 2 * 1024; // 2 KiB
pub(crate) const TELEMETRY_PREVIEW_MAX_LINES: usize = 64; // lines
pub(crate) const TELEMETRY_PREVIEW_TRUNCATION_NOTICE: &str =
    "[... telemetry preview truncated ...]";

// TODO(jif) break this down
pub(crate) async fn handle_container_exec_with_params(
    tool_name: &str,
    params: ExecParams,
    sess: &Session,
    turn_context: &TurnContext,
    turn_diff_tracker: &mut TurnDiffTracker,
    sub_id: String,
    call_id: String,
) -> Result<String, FunctionCallError> {
    let otel_event_manager = turn_context.client.get_otel_event_manager();

    if params.with_escalated_permissions.unwrap_or(false)
        && !matches!(turn_context.approval_policy, AskForApproval::OnRequest)
    {
        return Err(FunctionCallError::RespondToModel(format!(
            "approval policy is {policy:?}; reject command — you should not ask for escalated permissions if the approval policy is {policy:?}",
            policy = turn_context.approval_policy
        )));
    }

    // check if this was a patch, and apply it if so
    let apply_patch_exec = match maybe_parse_apply_patch_verified(&params.command, &params.cwd) {
        MaybeApplyPatchVerified::Body(changes) => {
            match apply_patch::apply_patch(sess, turn_context, &sub_id, &call_id, changes).await {
                InternalApplyPatchInvocation::Output(item) => return item,
                InternalApplyPatchInvocation::DelegateToExec(apply_patch_exec) => {
                    Some(apply_patch_exec)
                }
            }
        }
        MaybeApplyPatchVerified::CorrectnessError(parse_error) => {
            // It looks like an invocation of `apply_patch`, but we
            // could not resolve it into a patch that would apply
            // cleanly. Return to model for resample.
            return Err(FunctionCallError::RespondToModel(format!(
                "apply_patch verification failed: {parse_error}"
            )));
        }
        MaybeApplyPatchVerified::ShellParseError(error) => {
            trace!("Failed to parse shell command, {error:?}");
            None
        }
        MaybeApplyPatchVerified::NotApplyPatch => None,
    };

    let command_for_display = if let Some(exec) = apply_patch_exec.as_ref() {
        vec!["apply_patch".to_string(), exec.action.patch.clone()]
    } else {
        params.command.clone()
    };

    let exec_command_context = ExecCommandContext {
        sub_id: sub_id.clone(),
        call_id: call_id.clone(),
        command_for_display: command_for_display.clone(),
        cwd: params.cwd.clone(),
        apply_patch: apply_patch_exec.as_ref().map(
            |ApplyPatchExec {
                 action,
                 user_explicitly_approved_this_action,
             }| ApplyPatchCommandContext {
                user_explicitly_approved_this_action: *user_explicitly_approved_this_action,
                changes: convert_apply_patch_to_protocol(action),
            },
        ),
        tool_name: tool_name.to_string(),
        otel_event_manager,
    };

    let mode = match apply_patch_exec {
        Some(exec) => ExecutionMode::ApplyPatch(exec),
        None => ExecutionMode::Shell,
    };

    sess.services.executor.update_environment(
        turn_context.sandbox_policy.clone(),
        turn_context.cwd.clone(),
    );

    let prepared_exec = PreparedExec::new(
        exec_command_context,
        params,
        command_for_display,
        mode,
        Some(StdoutStream {
            sub_id: sub_id.clone(),
            call_id: call_id.clone(),
            tx_event: sess.get_tx_event(),
        }),
        turn_context.shell_environment_policy.use_profile,
    );

    let output_result = sess
        .run_exec_with_events(
            turn_diff_tracker,
            prepared_exec,
            turn_context.approval_policy,
        )
        .await;

    match output_result {
        Ok(output) => {
            let ExecToolCallOutput { exit_code, .. } = &output;
            let content = format_exec_output_apply_patch(&output);
            if *exit_code == 0 {
                Ok(content)
            } else {
                Err(FunctionCallError::RespondToModel(content))
            }
        }
        Err(ExecError::Function(err)) => Err(err),
        Err(ExecError::Codex(CodexErr::Sandbox(SandboxErr::Timeout { output }))) => Err(
            FunctionCallError::RespondToModel(format_exec_output_apply_patch(&output)),
        ),
        Err(ExecError::Codex(err)) => Err(FunctionCallError::RespondToModel(format!(
            "execution error: {err:?}"
        ))),
    }
}

pub fn format_exec_output_apply_patch(exec_output: &ExecToolCallOutput) -> String {
    let ExecToolCallOutput {
        exit_code,
        duration,
        ..
    } = exec_output;

    #[derive(Serialize)]
    struct ExecMetadata {
        exit_code: i32,
        duration_seconds: f32,
    }

    #[derive(Serialize)]
    struct ExecOutput<'a> {
        output: &'a str,
        metadata: ExecMetadata,
    }

    // round to 1 decimal place
    let duration_seconds = ((duration.as_secs_f32()) * 10.0).round() / 10.0;

    let formatted_output = format_exec_output_str(exec_output);

    let payload = ExecOutput {
        output: &formatted_output,
        metadata: ExecMetadata {
            exit_code: *exit_code,
            duration_seconds,
        },
    };

    #[expect(clippy::expect_used)]
    serde_json::to_string(&payload).expect("serialize ExecOutput")
}

pub fn format_exec_output_str(exec_output: &ExecToolCallOutput) -> String {
    let ExecToolCallOutput {
        aggregated_output, ..
    } = exec_output;

    // Head+tail truncation for the model: show the beginning and end with an elision.
    // Clients still receive full streams; only this formatted summary is capped.

    let mut s = &aggregated_output.text;
    let prefixed_str: String;

    if exec_output.timed_out {
        prefixed_str = format!(
            "command timed out after {} milliseconds\n",
            exec_output.duration.as_millis()
        ) + s;
        s = &prefixed_str;
    }

    let total_lines = s.lines().count();
    if s.len() <= MODEL_FORMAT_MAX_BYTES && total_lines <= MODEL_FORMAT_MAX_LINES {
        return s.to_string();
    }

    let segments: Vec<&str> = s.split_inclusive('\n').collect();
    let head_take = MODEL_FORMAT_HEAD_LINES.min(segments.len());
    let tail_take = MODEL_FORMAT_TAIL_LINES.min(segments.len().saturating_sub(head_take));
    let omitted = segments.len().saturating_sub(head_take + tail_take);

    let head_slice_end: usize = segments
        .iter()
        .take(head_take)
        .map(|segment| segment.len())
        .sum();
    let tail_slice_start: usize = if tail_take == 0 {
        s.len()
    } else {
        s.len()
            - segments
                .iter()
                .rev()
                .take(tail_take)
                .map(|segment| segment.len())
                .sum::<usize>()
    };
    let marker = format!("\n[... omitted {omitted} of {total_lines} lines ...]\n\n");

    // Byte budgets for head/tail around the marker
    let mut head_budget = MODEL_FORMAT_HEAD_BYTES.min(MODEL_FORMAT_MAX_BYTES);
    let tail_budget = MODEL_FORMAT_MAX_BYTES.saturating_sub(head_budget + marker.len());
    if tail_budget == 0 && marker.len() >= MODEL_FORMAT_MAX_BYTES {
        // Degenerate case: marker alone exceeds budget; return a clipped marker
        return take_bytes_at_char_boundary(&marker, MODEL_FORMAT_MAX_BYTES).to_string();
    }
    if tail_budget == 0 {
        // Make room for the marker by shrinking head
        head_budget = MODEL_FORMAT_MAX_BYTES.saturating_sub(marker.len());
    }

    let head_slice = &s[..head_slice_end];
    let head_part = take_bytes_at_char_boundary(head_slice, head_budget);
    let mut result = String::with_capacity(MODEL_FORMAT_MAX_BYTES.min(s.len()));

    result.push_str(head_part);
    result.push_str(&marker);

    let remaining = MODEL_FORMAT_MAX_BYTES.saturating_sub(result.len());
    if remaining == 0 {
        return result;
    }

    let tail_slice = &s[tail_slice_start..];
    let tail_part = take_last_bytes_at_char_boundary(tail_slice, remaining);
    result.push_str(tail_part);

    result
}
