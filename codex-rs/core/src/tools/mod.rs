pub mod context;
pub mod events;
pub(crate) mod handlers;
pub mod orchestrator;
pub mod parallel;
pub mod registry;
pub mod router;
pub mod runtimes;
pub mod sandboxing;
pub mod spec;

use crate::apply_patch;
use crate::apply_patch::InternalApplyPatchInvocation;
use crate::apply_patch::convert_apply_patch_to_protocol;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::error::CodexErr;
use crate::error::SandboxErr;
use crate::exec::ExecParams;
use crate::exec::ExecToolCallOutput;
use crate::function_tool::FunctionCallError;
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::events::ToolEmitter;
use crate::tools::events::ToolEventCtx;
use crate::tools::events::ToolEventFailure;
use crate::tools::events::ToolEventStage;
use crate::tools::orchestrator::ToolOrchestrator;
use crate::tools::runtimes::apply_patch::ApplyPatchRequest;
use crate::tools::runtimes::apply_patch::ApplyPatchRuntime;
use crate::tools::runtimes::shell::ShellRequest;
use crate::tools::runtimes::shell::ShellRuntime;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use codex_apply_patch::MaybeApplyPatchVerified;
use codex_apply_patch::maybe_parse_apply_patch_verified;
use codex_protocol::protocol::AskForApproval;
use codex_utils_string::take_bytes_at_char_boundary;
use codex_utils_string::take_last_bytes_at_char_boundary;
pub use router::ToolRouter;
use serde::Serialize;
use std::sync::Arc;
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
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    turn_diff_tracker: SharedTurnDiffTracker,
    call_id: String,
) -> Result<String, FunctionCallError> {
    let _otel_event_manager = turn_context.client.get_otel_event_manager();

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
            match apply_patch::apply_patch(sess.as_ref(), turn_context.as_ref(), &call_id, changes)
                .await
            {
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

    let (event_emitter, diff_opt) = match apply_patch_exec.as_ref() {
        Some(exec) => (
            ToolEmitter::apply_patch(
                convert_apply_patch_to_protocol(&exec.action),
                !exec.user_explicitly_approved_this_action,
            ),
            Some(&turn_diff_tracker),
        ),
        None => (
            ToolEmitter::shell(params.command.clone(), params.cwd.clone()),
            None,
        ),
    };

    let event_ctx = ToolEventCtx::new(sess.as_ref(), turn_context.as_ref(), &call_id, diff_opt);
    event_emitter.emit(event_ctx, ToolEventStage::Begin).await;

    // Build runtime contexts only when needed (shell/apply_patch below).

    if let Some(exec) = apply_patch_exec {
        // Route apply_patch execution through the new orchestrator/runtime.
        let req = ApplyPatchRequest {
            patch: exec.action.patch.clone(),
            cwd: params.cwd.clone(),
            timeout_ms: params.timeout_ms,
            user_explicitly_approved: exec.user_explicitly_approved_this_action,
            codex_exe: turn_context.codex_linux_sandbox_exe.clone(),
        };

        let mut orchestrator = ToolOrchestrator::new();
        let mut runtime = ApplyPatchRuntime::new();
        let tool_ctx = ToolCtx {
            session: sess.as_ref(),
            turn: turn_context.as_ref(),
            call_id: call_id.clone(),
            tool_name: tool_name.to_string(),
        };

        let out = orchestrator
            .run(
                &mut runtime,
                &req,
                &tool_ctx,
                &turn_context,
                turn_context.approval_policy,
            )
            .await;

        handle_exec_outcome(&event_emitter, event_ctx, out).await
    } else {
        // Route shell execution through the new orchestrator/runtime.
        let req = ShellRequest {
            command: params.command.clone(),
            cwd: params.cwd.clone(),
            timeout_ms: params.timeout_ms,
            env: params.env.clone(),
            with_escalated_permissions: params.with_escalated_permissions,
            justification: params.justification.clone(),
        };

        let mut orchestrator = ToolOrchestrator::new();
        let mut runtime = ShellRuntime::new();
        let tool_ctx = ToolCtx {
            session: sess.as_ref(),
            turn: turn_context.as_ref(),
            call_id: call_id.clone(),
            tool_name: tool_name.to_string(),
        };

        let out = orchestrator
            .run(
                &mut runtime,
                &req,
                &tool_ctx,
                &turn_context,
                turn_context.approval_policy,
            )
            .await;

        handle_exec_outcome(&event_emitter, event_ctx, out).await
    }
}

async fn handle_exec_outcome(
    event_emitter: &ToolEmitter,
    event_ctx: ToolEventCtx<'_>,
    out: Result<ExecToolCallOutput, ToolError>,
) -> Result<String, FunctionCallError> {
    let event;
    let result = match out {
        Ok(output) => {
            let content = format_exec_output_for_model(&output);
            let exit_code = output.exit_code;
            event = ToolEventStage::Success(output);
            if exit_code == 0 {
                Ok(content)
            } else {
                Err(FunctionCallError::RespondToModel(content))
            }
        }
        Err(ToolError::Codex(CodexErr::Sandbox(SandboxErr::Timeout { output })))
        | Err(ToolError::Codex(CodexErr::Sandbox(SandboxErr::Denied { output }))) => {
            let response = format_exec_output_for_model(&output);
            event = ToolEventStage::Failure(ToolEventFailure::Output(*output));
            Err(FunctionCallError::RespondToModel(response))
        }
        Err(ToolError::Codex(err)) => {
            let message = format!("execution error: {err:?}");
            let response = format_exec_output(&message);
            event = ToolEventStage::Failure(ToolEventFailure::Message(message));
            Err(FunctionCallError::RespondToModel(format_exec_output(
                &response,
            )))
        }
        Err(ToolError::Rejected(msg)) | Err(ToolError::SandboxDenied(msg)) => {
            // Normalize common rejection messages for exec tools so tests and
            // users see a clear, consistent phrase.
            let normalized = if msg == "rejected by user" {
                "exec command rejected by user".to_string()
            } else {
                msg
            };
            let response = format_exec_output(&normalized);
            event = ToolEventStage::Failure(ToolEventFailure::Message(normalized));
            Err(FunctionCallError::RespondToModel(format_exec_output(
                &response,
            )))
        }
    };
    event_emitter.emit(event_ctx, event).await;
    result
}

/// Format the combined exec output for sending back to the model.
/// Includes exit code and duration metadata; truncates large bodies safely.
pub fn format_exec_output_for_model(exec_output: &ExecToolCallOutput) -> String {
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

    let content = aggregated_output.text.as_str();

    if exec_output.timed_out {
        let prefixed = format!(
            "command timed out after {} milliseconds\n{content}",
            exec_output.duration.as_millis()
        );
        return format_exec_output(&prefixed);
    }

    format_exec_output(content)
}

pub(super) fn format_exec_output(content: &str) -> String {
    // Head+tail truncation for the model: show the beginning and end with an elision.
    // Clients still receive full streams; only this formatted summary is capped.
    let total_lines = content.lines().count();
    if content.len() <= MODEL_FORMAT_MAX_BYTES && total_lines <= MODEL_FORMAT_MAX_LINES {
        return content.to_string();
    }
    let output = truncate_formatted_exec_output(content, total_lines);
    format!("Total output lines: {total_lines}\n\n{output}")
}

fn truncate_formatted_exec_output(content: &str, total_lines: usize) -> String {
    let segments: Vec<&str> = content.split_inclusive('\n').collect();
    let head_take = MODEL_FORMAT_HEAD_LINES.min(segments.len());
    let tail_take = MODEL_FORMAT_TAIL_LINES.min(segments.len().saturating_sub(head_take));
    let omitted = segments.len().saturating_sub(head_take + tail_take);

    let head_slice_end: usize = segments
        .iter()
        .take(head_take)
        .map(|segment| segment.len())
        .sum();
    let tail_slice_start: usize = if tail_take == 0 {
        content.len()
    } else {
        content.len()
            - segments
                .iter()
                .rev()
                .take(tail_take)
                .map(|segment| segment.len())
                .sum::<usize>()
    };
    let head_slice = &content[..head_slice_end];
    let tail_slice = &content[tail_slice_start..];
    let truncated_by_bytes = content.len() > MODEL_FORMAT_MAX_BYTES;
    let marker = if omitted > 0 {
        Some(format!(
            "\n[... omitted {omitted} of {total_lines} lines ...]\n\n"
        ))
    } else if truncated_by_bytes {
        Some(format!(
            "\n[... output truncated to fit {MODEL_FORMAT_MAX_BYTES} bytes ...]\n\n"
        ))
    } else {
        None
    };

    let marker_len = marker.as_ref().map_or(0, String::len);
    let base_head_budget = MODEL_FORMAT_HEAD_BYTES.min(MODEL_FORMAT_MAX_BYTES);
    let head_budget = base_head_budget.min(MODEL_FORMAT_MAX_BYTES.saturating_sub(marker_len));
    let head_part = take_bytes_at_char_boundary(head_slice, head_budget);
    let mut result = String::with_capacity(MODEL_FORMAT_MAX_BYTES.min(content.len()));

    result.push_str(head_part);
    if let Some(marker_text) = marker.as_ref() {
        result.push_str(marker_text);
    }

    let remaining = MODEL_FORMAT_MAX_BYTES.saturating_sub(result.len());
    if remaining == 0 {
        return result;
    }

    let tail_part = take_last_bytes_at_char_boundary(tail_slice, remaining);
    result.push_str(tail_part);

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex_lite::Regex;

    fn truncate_function_error(err: FunctionCallError) -> FunctionCallError {
        match err {
            FunctionCallError::RespondToModel(msg) => {
                FunctionCallError::RespondToModel(format_exec_output(&msg))
            }
            FunctionCallError::Denied(msg) => FunctionCallError::Denied(format_exec_output(&msg)),
            FunctionCallError::Fatal(msg) => FunctionCallError::Fatal(format_exec_output(&msg)),
            other => other,
        }
    }

    fn assert_truncated_message_matches(message: &str, line: &str, total_lines: usize) {
        let pattern = truncated_message_pattern(line, total_lines);
        let regex = Regex::new(&pattern).unwrap_or_else(|err| {
            panic!("failed to compile regex {pattern}: {err}");
        });
        let captures = regex
            .captures(message)
            .unwrap_or_else(|| panic!("message failed to match pattern {pattern}: {message}"));
        let body = captures
            .name("body")
            .expect("missing body capture")
            .as_str();
        assert!(
            body.len() <= MODEL_FORMAT_MAX_BYTES,
            "body exceeds byte limit: {} bytes",
            body.len()
        );
    }

    fn truncated_message_pattern(line: &str, total_lines: usize) -> String {
        let head_take = MODEL_FORMAT_HEAD_LINES.min(total_lines);
        let tail_take = MODEL_FORMAT_TAIL_LINES.min(total_lines.saturating_sub(head_take));
        let omitted = total_lines.saturating_sub(head_take + tail_take);
        let escaped_line = regex_lite::escape(line);
        if omitted == 0 {
            return format!(
                r"(?s)^Total output lines: {total_lines}\n\n(?P<body>{escaped_line}.*\n\[\.{{3}} output truncated to fit {MODEL_FORMAT_MAX_BYTES} bytes \.{{3}}]\n\n.*)$",
            );
        }
        format!(
            r"(?s)^Total output lines: {total_lines}\n\n(?P<body>{escaped_line}.*\n\[\.{{3}} omitted {omitted} of {total_lines} lines \.{{3}}]\n\n.*)$",
        )
    }

    #[test]
    fn truncate_formatted_exec_output_truncates_large_error() {
        let line = "very long execution error line that should trigger truncation\n";
        let large_error = line.repeat(2_500); // way beyond both byte and line limits

        let truncated = format_exec_output(&large_error);

        let total_lines = large_error.lines().count();
        assert_truncated_message_matches(&truncated, line, total_lines);
        assert_ne!(truncated, large_error);
    }

    #[test]
    fn truncate_function_error_trims_respond_to_model() {
        let line = "respond-to-model error that should be truncated\n";
        let huge = line.repeat(3_000);
        let total_lines = huge.lines().count();

        let err = truncate_function_error(FunctionCallError::RespondToModel(huge));
        match err {
            FunctionCallError::RespondToModel(message) => {
                assert_truncated_message_matches(&message, line, total_lines);
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn truncate_function_error_trims_fatal() {
        let line = "fatal error output that should be truncated\n";
        let huge = line.repeat(3_000);
        let total_lines = huge.lines().count();

        let err = truncate_function_error(FunctionCallError::Fatal(huge));
        match err {
            FunctionCallError::Fatal(message) => {
                assert_truncated_message_matches(&message, line, total_lines);
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn truncate_formatted_exec_output_marks_byte_truncation_without_omitted_lines() {
        let long_line = "a".repeat(MODEL_FORMAT_MAX_BYTES + 50);
        let truncated = format_exec_output(&long_line);

        assert_ne!(truncated, long_line);
        let marker_line =
            format!("[... output truncated to fit {MODEL_FORMAT_MAX_BYTES} bytes ...]");
        assert!(
            truncated.contains(&marker_line),
            "missing byte truncation marker: {truncated}"
        );
        assert!(
            !truncated.contains("omitted"),
            "line omission marker should not appear when no lines were dropped: {truncated}"
        );
    }

    #[test]
    fn truncate_formatted_exec_output_returns_original_when_within_limits() {
        let content = "example output\n".repeat(10);

        assert_eq!(format_exec_output(&content), content);
    }

    #[test]
    fn truncate_formatted_exec_output_reports_omitted_lines_and_keeps_head_and_tail() {
        let total_lines = MODEL_FORMAT_MAX_LINES + 100;
        let content: String = (0..total_lines)
            .map(|idx| format!("line-{idx}\n"))
            .collect();

        let truncated = format_exec_output(&content);
        let omitted = total_lines - MODEL_FORMAT_MAX_LINES;
        let expected_marker = format!("[... omitted {omitted} of {total_lines} lines ...]");

        assert!(
            truncated.contains(&expected_marker),
            "missing omitted marker: {truncated}"
        );
        assert!(
            truncated.contains("line-0\n"),
            "expected head line to remain: {truncated}"
        );

        let last_line = format!("line-{}\n", total_lines - 1);
        assert!(
            truncated.contains(&last_line),
            "expected tail line to remain: {truncated}"
        );
    }

    #[test]
    fn truncate_formatted_exec_output_prefers_line_marker_when_both_limits_exceeded() {
        let total_lines = MODEL_FORMAT_MAX_LINES + 42;
        let long_line = "x".repeat(256);
        let content: String = (0..total_lines)
            .map(|idx| format!("line-{idx}-{long_line}\n"))
            .collect();

        let truncated = format_exec_output(&content);

        assert!(
            truncated.contains("[... omitted 42 of 298 lines ...]"),
            "expected omitted marker when line count exceeds limit: {truncated}"
        );
        assert!(
            !truncated.contains("output truncated to fit"),
            "line omission marker should take precedence over byte marker: {truncated}"
        );
    }
}
