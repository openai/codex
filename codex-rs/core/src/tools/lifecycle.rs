use codex_extension_api::ToolCallOutcome;
use codex_extension_api::ToolCallSource as ExtensionToolCallSource;
use codex_extension_api::ToolFinishInput;
use codex_extension_api::ToolStartInput;
use codex_tools::ToolName;

use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use crate::tools::context::ToolCallSource;
use crate::tools::context::ToolInvocation;

pub(crate) async fn notify_tool_start(invocation: &ToolInvocation) {
    for contributor in invocation
        .session
        .services
        .extensions
        .tool_lifecycle_contributors()
    {
        contributor
            .on_tool_start(ToolStartInput {
                session_store: &invocation.session.services.session_extension_data,
                thread_store: &invocation.session.services.thread_extension_data,
                turn_store: invocation.turn.extension_data.as_ref(),
                turn_id: invocation.turn.sub_id.as_str(),
                call_id: invocation.call_id.as_str(),
                tool_name: &invocation.tool_name,
                source: extension_tool_call_source(invocation.source.clone()),
            })
            .await;
    }
}

pub(crate) async fn notify_tool_finish(invocation: &ToolInvocation, outcome: ToolCallOutcome) {
    notify_tool_finish_parts(
        invocation.session.as_ref(),
        invocation.turn.as_ref(),
        invocation.call_id.as_str(),
        &invocation.tool_name,
        invocation.source.clone(),
        outcome,
    )
    .await;
}

pub(crate) async fn notify_tool_aborted(
    session: &Session,
    turn: &TurnContext,
    call_id: &str,
    tool_name: &ToolName,
    source: ToolCallSource,
) {
    notify_tool_finish_parts(
        session,
        turn,
        call_id,
        tool_name,
        source,
        ToolCallOutcome::Aborted,
    )
    .await;
}

async fn notify_tool_finish_parts(
    session: &Session,
    turn: &TurnContext,
    call_id: &str,
    tool_name: &ToolName,
    source: ToolCallSource,
    outcome: ToolCallOutcome,
) {
    for contributor in session.services.extensions.tool_lifecycle_contributors() {
        contributor
            .on_tool_finish(ToolFinishInput {
                session_store: &session.services.session_extension_data,
                thread_store: &session.services.thread_extension_data,
                turn_store: turn.extension_data.as_ref(),
                turn_id: turn.sub_id.as_str(),
                call_id,
                tool_name,
                source: extension_tool_call_source(source.clone()),
                outcome,
            })
            .await;
    }
}

fn extension_tool_call_source(source: ToolCallSource) -> ExtensionToolCallSource {
    match source {
        ToolCallSource::Direct => ExtensionToolCallSource::Direct,
        ToolCallSource::CodeMode {
            cell_id,
            runtime_tool_call_id,
        } => ExtensionToolCallSource::CodeMode {
            cell_id,
            runtime_tool_call_id,
        },
    }
}
