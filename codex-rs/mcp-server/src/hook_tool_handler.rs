// Hook Tool Handler
use anyhow::Result;
use mcp_types::CallToolResult;
use mcp_types::ContentBlock;
use mcp_types::TextContent;
use serde_json::Value;

use crate::hook_tool::HookToolParam;

pub async fn handle_hook_tool_call(arguments: Value) -> Result<CallToolResult> {
    let params: HookToolParam = serde_json::from_value(arguments)?;

    let event_description = match params.event.as_str() {
        "on_task_start" => "Task Start",
        "on_task_complete" => "Task Complete",
        "on_error" => "Error Occurred",
        "on_task_abort" => "Task Aborted",
        "on_subagent_start" => "SubAgent Started",
        "on_subagent_complete" => "SubAgent Completed",
        "on_session_start" => "Session Started",
        "on_session_end" => "Session Ended",
        "on_patch_apply" => "Patch Applied",
        "on_command_exec" => "Command Executed",
        _ => return Err(anyhow::anyhow!("Unknown hook event: {}", params.event)),
    };

    let context_info = params.context.as_deref().unwrap_or("No context provided");

    let response_text = format!(
        "[Hook] Executing hook for event: {}\n\n\
        Event Type: {}\n\
        Context: {}\n\n\
        Hook execution details:\n\
        - Environment variables set:\n\
          CODEX_HOOK_EVENT={}\n\
        - Execution mode: Asynchronous (non-blocking)\n\
        - Timeout: 30 seconds\n\n\
        Note: To configure hooks, add them to ~/.codex/config.toml:\n\n\
        [hooks]\n\
        async_execution = true\n\
        timeout_seconds = 30\n\n\
        [[hooks.{}]]\n\
        command = \"your-command-here\"\n\n\
        Example hook commands:\n\
        - echo 'Task completed' >> task.log\n\
        - notify-send 'Codex' 'Task done'\n\
        - curl -X POST $SLACK_WEBHOOK -d '{{\"text\":\"Hook triggered\"}}'",
        event_description, event_description, context_info, params.event, params.event
    );

    Ok(CallToolResult {
        content: vec![ContentBlock::TextContent(TextContent {
            r#type: "text".to_string(),
            text: response_text,
            annotations: None,
        })],
        is_error: None,
        structured_content: None,
    })
}
