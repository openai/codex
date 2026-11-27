//! Reports terminal process-creation failures as command attempts without a process handle.
//! Approval rejections retain their own lifecycle, and sandbox denials remain retryable.
//! Session shutdown drains failure publications that outlive the caller.

use crate::session::command_approval::CommandApproval;
use crate::tools::events::ToolEmitter;
use crate::tools::events::ToolEventCtx;
use crate::tools::events::ToolEventFailure;
use crate::tools::events::ToolEventStage;
use crate::tools::handlers::unified_exec::preparation::PreparedExecCommand;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use crate::unified_exec::UnifiedExecProcess;
use codex_protocol::protocol::ExecCommandSource;
use std::sync::Arc;
use tracing::Instrument;

pub(super) async fn with_launch_failure_events(
    result: Result<UnifiedExecProcess, ToolError>,
    req: &PreparedExecCommand,
    ctx: &ToolCtx,
    command_approval: &mut CommandApproval,
) -> Result<UnifiedExecProcess, ToolError> {
    let Err(ToolError::Rejected(message)) = &result else {
        return result;
    };
    let plugin_attribution = if req.turn_environment.environment().is_remote() {
        let file_system = req
            .turn_environment
            .environment()
            .get_filesystem_without_reconnect();
        ctx.step_context
            .turn
            .plugin_attribution_for_executor_command(
                &req.resolved.command,
                &req.cwd,
                file_system.as_ref(),
            )
            .await
    } else {
        req.cwd.to_abs_path().ok().and_then(|cwd| {
            ctx.step_context
                .turn
                .plugin_attribution_for_command(&req.resolved.command, &cwd)
        })
    };
    // Register before checking closure so shutdown waits for every admitted write.
    let writes = &ctx
        .session
        .services
        .unified_exec_manager
        .launch_failure_writes;
    let write = writes.token();
    if writes.is_closed() {
        return result;
    }
    let emitter = ToolEmitter::unified_exec(
        &req.resolved.command,
        req.cwd.clone(),
        ExecCommandSource::UnifiedExecStartup,
        /*process_id*/ None,
        plugin_attribution,
    );
    let session = Arc::clone(&ctx.session);
    let step_context = Arc::clone(&ctx.step_context);
    let call_id = ctx.call_id.clone();
    let message = message.clone();
    // The failure write is registered, so the proposal can release its shutdown token.
    let (proposal_started_at_ms, _) = command_approval.handoff().unzip();
    // Dropping the join handle on cancellation leaves both lifecycle events running.
    let publish = async move {
        let model_context = step_context.model_context();
        let mut event_ctx = ToolEventCtx::new(
            session.as_ref(),
            step_context.turn.as_ref(),
            &step_context.settings.model_info,
            &call_id,
            /*turn_diff_tracker*/ None,
        );
        event_ctx.proposal_started_at_ms = proposal_started_at_ms;
        event_ctx.model_context = Some(&model_context);
        emitter.emit(event_ctx, ToolEventStage::Begin).await;
        emitter
            .emit(
                event_ctx,
                ToolEventStage::Failure(ToolEventFailure::Message(message)),
            )
            .await;
        drop(write);
    };
    if let Err(err) = tokio::spawn(publish.in_current_span()).await {
        tracing::warn!(%err, "failed to publish unified exec launch failure");
    }
    result
}
