use std::sync::Arc;

use codex_extension_api::CommandStartInput;
use codex_extension_api::McpToolContext;
use codex_extension_api::McpToolResultInput;
use codex_extension_api::ToolCallOutcome;
use codex_extension_api::ToolCallSource as ExtensionToolCallSource;
use codex_extension_api::ToolFinishInput;
use codex_extension_api::ToolStartInput;
use codex_file_system::ExecutorFileSystem;
use codex_protocol::mcp::CallToolResult;
use codex_tools::ToolName;
use codex_utils_path_uri::PathUri;

use crate::guardian::tool_permission_context;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use crate::tools::context::ToolCallSource;
use crate::tools::context::ToolInvocation;

pub(crate) async fn notify_tool_start(
    invocation: &ToolInvocation,
    mcp_tool: Option<&McpToolContext>,
) {
    let contributors = invocation
        .session
        .services
        .extensions
        .tool_lifecycle_contributors();
    if contributors.is_empty() {
        return;
    }
    let thread_store = &invocation.session.services.thread_extension_data;
    let conversation_history = invocation.session.conversation_history_snapshot().await;
    let root_turn_id = invocation.turn.turn_metadata_state.root_turn_id();
    let originating_item_id = invocation
        .originating_call()
        .await
        .and_then(|origin| origin.item_id);

    for contributor in contributors {
        contributor
            .on_tool_start(ToolStartInput {
                session_store: &invocation.session.services.session_extension_data,
                thread_store,
                turn_store: invocation.turn.extension_data.as_ref(),
                turn_id: invocation.turn.sub_id.as_str(),
                root_turn_id: root_turn_id.as_deref(),
                call_id: invocation.call_id.as_str(),
                originating_item_id: originating_item_id.as_ref(),
                tool_name: &invocation.tool_name,
                mcp_tool,
                permissions: Box::pin(async { tool_permission_context(invocation).await.ok() }),
                payload: &invocation.payload,
                conversation_history: Arc::clone(&conversation_history),
                source: extension_tool_call_source(invocation.source.clone()),
            })
            .await;
    }
}

pub(crate) async fn notify_command_start(
    session: &Session,
    turn: &TurnContext,
    call_id: &str,
    command: &[String],
    cwd: &PathUri,
    file_system: &dyn ExecutorFileSystem,
) {
    for contributor in session.services.extensions.tool_lifecycle_contributors() {
        contributor
            .on_command_start(CommandStartInput {
                session_store: &session.services.session_extension_data,
                thread_store: &session.services.thread_extension_data,
                turn_store: turn.extension_data.as_ref(),
                turn_id: &turn.sub_id,
                call_id,
                command,
                cwd,
                file_system,
            })
            .await;
    }
}

pub(crate) async fn process_mcp_tool_result(
    session: &Session,
    turn: &TurnContext,
    call_id: &str,
    mcp_tool: &McpToolContext,
    arguments: &serde_json::Value,
    result: &mut CallToolResult,
) {
    for contributor in session.services.extensions.tool_lifecycle_contributors() {
        contributor
            .on_mcp_tool_result(McpToolResultInput {
                session_store: &session.services.session_extension_data,
                thread_store: &session.services.thread_extension_data,
                turn_store: turn.extension_data.as_ref(),
                turn_id: &turn.sub_id,
                call_id,
                mcp_tool,
                arguments,
                result,
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

pub(crate) fn extension_tool_call_source(source: ToolCallSource) -> ExtensionToolCallSource {
    match source {
        ToolCallSource::Direct | ToolCallSource::DirectPlaintextMessage => {
            ExtensionToolCallSource::Direct
        }
        ToolCallSource::CodeMode {
            cell_id,
            runtime_tool_call_id,
        } => ExtensionToolCallSource::CodeMode {
            cell_id,
            runtime_tool_call_id,
        },
    }
}
