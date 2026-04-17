use codex_protocol::dynamic_tools::DynamicToolCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_tools::REFLECTIONS_LIST_TOOL_NAME;
use codex_tools::REFLECTIONS_READ_TOOL_NAME;
use codex_tools::REFLECTIONS_SEARCH_TOOL_NAME;
use codex_tools::REFLECTIONS_WRITE_NOTE_TOOL_NAME;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct LogEntry {
    pub(crate) entry_id: String,
    pub(crate) kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) role: Option<String>,
    pub(crate) content: String,
    pub(crate) metadata: Value,
}

pub(crate) fn log_entries_from_items(window: &str, items: &[RolloutItem]) -> Vec<LogEntry> {
    let mut entries = Vec::new();
    let mut call_tools = std::collections::HashMap::new();
    for item in items {
        let entry = match item {
            RolloutItem::EventMsg(event) => log_entry_from_event(event),
            RolloutItem::ResponseItem(response_item) => {
                log_entry_from_response_item(response_item, &mut call_tools)
            }
            RolloutItem::SessionMeta(_)
            | RolloutItem::Compacted(_)
            | RolloutItem::TurnContext(_) => None,
        };
        if let Some(mut entry) = entry {
            let entry_index = entries.len() + 1;
            entry.entry_id = entry_id(window, entry_index);
            entries.push(entry);
        }
    }
    entries
}

fn log_entry_from_event(event: &EventMsg) -> Option<LogEntry> {
    match event {
        EventMsg::UserMessage(event) => {
            let mut text = event.message.clone();
            if let Some(images) = event.images.as_ref().filter(|images| !images.is_empty()) {
                push_blank_line_if_needed(&mut text);
                text.push_str("images:\n");
                for image in images {
                    text.push_str(&format!("- {image}\n"));
                }
            }
            if !event.local_images.is_empty() {
                push_blank_line_if_needed(&mut text);
                text.push_str("local_images:\n");
                for image in &event.local_images {
                    text.push_str(&format!("- {}\n", image.display()));
                }
            }
            Some(LogEntry {
                entry_id: String::new(),
                kind: "user_message".to_string(),
                role: Some("user".to_string()),
                content: text,
                metadata: json!({}),
            })
        }
        EventMsg::AgentMessage(event) => Some(LogEntry {
            entry_id: String::new(),
            kind: "assistant_message".to_string(),
            role: Some("assistant".to_string()),
            content: event.message.clone(),
            metadata: json!({
                "phase": event.phase.as_ref().map(|phase| format!("{phase:?}").to_lowercase()),
            }),
        }),
        EventMsg::McpToolCallBegin(event) => Some(LogEntry {
            entry_id: String::new(),
            kind: "tool_call".to_string(),
            role: None,
            content: serde_json::to_string_pretty(&json!({
                "call_id": event.call_id,
                "arguments": event.invocation.arguments,
            }))
            .unwrap_or_default(),
            metadata: json!({
                "tool_name": format!("mcp.{}.{}", event.invocation.server, event.invocation.tool),
            }),
        }),
        EventMsg::McpToolCallEnd(event) => Some(LogEntry {
            entry_id: String::new(),
            kind: "tool_result".to_string(),
            role: None,
            content: serde_json::to_string_pretty(&json!({
                "call_id": event.call_id,
                "arguments": event.invocation.arguments,
                "success": event.is_success(),
                "result": event.result,
            }))
            .unwrap_or_default(),
            metadata: json!({
                "tool_name": format!("mcp.{}.{}", event.invocation.server, event.invocation.tool),
            }),
        }),
        EventMsg::WebSearchBegin(event) => Some(LogEntry {
            entry_id: String::new(),
            kind: "tool_call".to_string(),
            role: None,
            content: serde_json::to_string_pretty(event).unwrap_or_default(),
            metadata: json!({ "tool_name": "web_search" }),
        }),
        EventMsg::WebSearchEnd(event) => Some(LogEntry {
            entry_id: String::new(),
            kind: "tool_result".to_string(),
            role: None,
            content: serde_json::to_string_pretty(&json!({
                "call_id": event.call_id,
                "query": event.query,
                "action": event.action,
            }))
            .unwrap_or_default(),
            metadata: json!({ "tool_name": "web_search" }),
        }),
        EventMsg::ImageGenerationBegin(event) => Some(LogEntry {
            entry_id: String::new(),
            kind: "tool_call".to_string(),
            role: None,
            content: serde_json::to_string_pretty(event).unwrap_or_default(),
            metadata: json!({ "tool_name": "image_generation" }),
        }),
        EventMsg::ImageGenerationEnd(event) => Some(LogEntry {
            entry_id: String::new(),
            kind: "tool_result".to_string(),
            role: None,
            content: serde_json::to_string_pretty(&json!({
                "call_id": event.call_id,
                "status": event.status,
                "revised_prompt": event.revised_prompt,
                "result": event.result,
                "saved_path": event.saved_path.as_ref().map(|path| path.display().to_string()),
            }))
            .unwrap_or_default(),
            metadata: json!({ "tool_name": "image_generation" }),
        }),
        EventMsg::ExecCommandBegin(event) => Some(LogEntry {
            entry_id: String::new(),
            kind: "tool_call".to_string(),
            role: None,
            content: format!(
                "call_id: {}\ncwd: {}\ncommand: {}\n",
                event.call_id,
                event.cwd.display(),
                serde_json::to_string(&event.command).unwrap_or_else(|_| "[]".to_string())
            ),
            metadata: json!({ "tool_name": "exec_command" }),
        }),
        EventMsg::ExecCommandEnd(event) => {
            let mut content = format!(
                "call_id: {}\nstatus: {:?}\nexit_code: {}\ncwd: {}\ncommand: {}\n",
                event.call_id,
                event.status,
                event.exit_code,
                event.cwd.display(),
                serde_json::to_string(&event.command).unwrap_or_else(|_| "[]".to_string())
            );
            let output = exec_output(event);
            if !output.is_empty() {
                content.push_str("\noutput:\n");
                content.push_str(&output);
            }
            Some(LogEntry {
                entry_id: String::new(),
                kind: "tool_result".to_string(),
                role: None,
                content,
                metadata: json!({ "tool_name": "exec_command" }),
            })
        }
        EventMsg::ViewImageToolCall(event) => Some(LogEntry {
            entry_id: String::new(),
            kind: "tool_call".to_string(),
            role: None,
            content: serde_json::to_string_pretty(&json!({
                "call_id": event.call_id,
                "path": event.path.display().to_string(),
            }))
            .unwrap_or_default(),
            metadata: json!({ "tool_name": "view_image" }),
        }),
        EventMsg::DynamicToolCallRequest(event) => Some(LogEntry {
            entry_id: String::new(),
            kind: "tool_call".to_string(),
            role: None,
            content: dynamic_tool_call_content(&event.tool, &event.call_id, &event.arguments),
            metadata: json!({ "tool_name": event.tool }),
        }),
        EventMsg::DynamicToolCallResponse(event) => Some(LogEntry {
            entry_id: String::new(),
            kind: "tool_result".to_string(),
            role: None,
            content: dynamic_tool_response_content(
                &event.tool,
                &event.call_id,
                event.success,
                event.error.as_deref(),
                &event.content_items,
                &event.arguments,
            ),
            metadata: json!({ "tool_name": event.tool }),
        }),
        EventMsg::PatchApplyBegin(event) => Some(LogEntry {
            entry_id: String::new(),
            kind: "tool_call".to_string(),
            role: None,
            content: serde_json::to_string_pretty(&json!({
                "call_id": event.call_id,
                "auto_approved": event.auto_approved,
                "changes": event.changes,
            }))
            .unwrap_or_default(),
            metadata: json!({ "tool_name": "apply_patch" }),
        }),
        EventMsg::PatchApplyEnd(event) => Some(LogEntry {
            entry_id: String::new(),
            kind: "tool_result".to_string(),
            role: None,
            content: serde_json::to_string_pretty(&json!({
                "call_id": event.call_id,
                "success": event.success,
                "status": event.status,
                "stdout": event.stdout,
                "stderr": event.stderr,
                "changes": event.changes,
            }))
            .unwrap_or_default(),
            metadata: json!({ "tool_name": "apply_patch" }),
        }),
        EventMsg::Error(_)
        | EventMsg::Warning(_)
        | EventMsg::RealtimeConversationStarted(_)
        | EventMsg::RealtimeConversationRealtime(_)
        | EventMsg::RealtimeConversationClosed(_)
        | EventMsg::RealtimeConversationSdp(_)
        | EventMsg::ModelReroute(_)
        | EventMsg::ContextCompacted(_)
        | EventMsg::ThreadRolledBack(_)
        | EventMsg::TurnStarted(_)
        | EventMsg::TurnComplete(_)
        | EventMsg::TokenCount(_)
        | EventMsg::AgentMessageDelta(_)
        | EventMsg::AgentReasoning(_)
        | EventMsg::AgentReasoningDelta(_)
        | EventMsg::AgentReasoningRawContent(_)
        | EventMsg::AgentReasoningRawContentDelta(_)
        | EventMsg::AgentReasoningSectionBreak(_)
        | EventMsg::SessionConfigured(_)
        | EventMsg::ThreadNameUpdated(_)
        | EventMsg::McpStartupUpdate(_)
        | EventMsg::McpStartupComplete(_)
        | EventMsg::ExecCommandOutputDelta(_)
        | EventMsg::TerminalInteraction(_)
        | EventMsg::ExecApprovalRequest(_)
        | EventMsg::RequestPermissions(_)
        | EventMsg::RequestUserInput(_)
        | EventMsg::ElicitationRequest(_)
        | EventMsg::ApplyPatchApprovalRequest(_)
        | EventMsg::GuardianAssessment(_)
        | EventMsg::DeprecationNotice(_)
        | EventMsg::BackgroundEvent(_)
        | EventMsg::UndoStarted(_)
        | EventMsg::UndoCompleted(_)
        | EventMsg::StreamError(_)
        | EventMsg::TurnDiff(_)
        | EventMsg::GetHistoryEntryResponse(_)
        | EventMsg::McpListToolsResponse(_)
        | EventMsg::ListSkillsResponse(_)
        | EventMsg::RealtimeConversationListVoicesResponse(_)
        | EventMsg::SkillsUpdateAvailable
        | EventMsg::PlanUpdate(_)
        | EventMsg::TurnAborted(_)
        | EventMsg::ShutdownComplete
        | EventMsg::EnteredReviewMode(_)
        | EventMsg::ExitedReviewMode(_)
        | EventMsg::RawResponseItem(_)
        | EventMsg::ItemStarted(_)
        | EventMsg::ItemCompleted(_)
        | EventMsg::HookStarted(_)
        | EventMsg::HookCompleted(_)
        | EventMsg::AgentMessageContentDelta(_)
        | EventMsg::PlanDelta(_)
        | EventMsg::ReasoningContentDelta(_)
        | EventMsg::ReasoningRawContentDelta(_)
        | EventMsg::CollabAgentSpawnBegin(_)
        | EventMsg::CollabAgentSpawnEnd(_)
        | EventMsg::CollabAgentInteractionBegin(_)
        | EventMsg::CollabAgentInteractionEnd(_)
        | EventMsg::CollabWaitingBegin(_)
        | EventMsg::CollabWaitingEnd(_)
        | EventMsg::CollabCloseBegin(_)
        | EventMsg::CollabCloseEnd(_)
        | EventMsg::CollabResumeBegin(_)
        | EventMsg::CollabResumeEnd(_) => None,
    }
}

fn log_entry_from_response_item(
    item: &ResponseItem,
    call_tools: &mut std::collections::HashMap<String, String>,
) -> Option<LogEntry> {
    match item {
        ResponseItem::FunctionCall {
            name,
            arguments,
            call_id,
            namespace,
            ..
        } if is_reflections_storage_tool(name) => {
            call_tools.insert(call_id.clone(), name.clone());
            let arguments_json = serde_json::from_str(arguments).unwrap_or(Value::Null);
            Some(LogEntry {
                entry_id: String::new(),
                kind: "tool_call".to_string(),
                role: None,
                content: reflections_storage_tool_call_summary(name, call_id, &arguments_json),
                metadata: json!({
                    "tool_name": name,
                    "namespace": namespace,
                }),
            })
        }
        ResponseItem::FunctionCallOutput { call_id, output }
            if call_tools
                .get(call_id)
                .is_some_and(|tool_name| is_reflections_storage_tool(tool_name)) =>
        {
            let tool_name = call_tools.get(call_id).cloned().unwrap_or_default();
            Some(LogEntry {
                entry_id: String::new(),
                kind: "tool_result".to_string(),
                role: None,
                content: reflections_storage_tool_output_summary(&tool_name, call_id, output),
                metadata: json!({
                    "tool_name": tool_name,
                    "success": output.success,
                }),
            })
        }
        ResponseItem::Message { .. }
        | ResponseItem::Reasoning { .. }
        | ResponseItem::LocalShellCall { .. }
        | ResponseItem::FunctionCall { .. }
        | ResponseItem::ToolSearchCall { .. }
        | ResponseItem::FunctionCallOutput { .. }
        | ResponseItem::CustomToolCall { .. }
        | ResponseItem::CustomToolCallOutput { .. }
        | ResponseItem::ToolSearchOutput { .. }
        | ResponseItem::WebSearchCall { .. }
        | ResponseItem::ImageGenerationCall { .. }
        | ResponseItem::GhostSnapshot { .. }
        | ResponseItem::Compaction { .. }
        | ResponseItem::Other => None,
    }
}

pub(super) fn push_blank_line_if_needed(text: &mut String) {
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    if !text.is_empty() {
        text.push('\n');
    }
}

pub(super) fn exec_output(event: &codex_protocol::protocol::ExecCommandEndEvent) -> String {
    if !event.aggregated_output.is_empty() {
        return event.aggregated_output.clone();
    }

    let mut output = String::new();
    if !event.stdout.is_empty() {
        output.push_str("stdout:\n");
        output.push_str(&event.stdout);
        if !event.stdout.ends_with('\n') {
            output.push('\n');
        }
    }
    if !event.stderr.is_empty() {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str("stderr:\n");
        output.push_str(&event.stderr);
        if !event.stderr.ends_with('\n') {
            output.push('\n');
        }
    }
    output
}

pub(super) fn dynamic_tool_output_items_to_text(
    items: &[DynamicToolCallOutputContentItem],
) -> String {
    let mut pieces = Vec::new();
    for item in items {
        match item {
            DynamicToolCallOutputContentItem::InputText { text } => pieces.push(text.clone()),
            DynamicToolCallOutputContentItem::InputImage { .. } => {
                pieces.push("[image omitted from Reflections transcript]".to_string());
            }
        }
    }
    pieces.join("\n")
}

fn dynamic_tool_call_content(tool: &str, call_id: &str, arguments: &Value) -> String {
    let value = if is_reflections_storage_tool(tool) {
        reflections_storage_tool_call_metadata(tool, call_id, arguments)
    } else {
        json!({
            "call_id": call_id,
            "arguments": arguments,
        })
    };
    serde_json::to_string_pretty(&value).unwrap_or_default()
}

fn dynamic_tool_response_content(
    tool: &str,
    call_id: &str,
    success: bool,
    error: Option<&str>,
    content_items: &[DynamicToolCallOutputContentItem],
    arguments: &Value,
) -> String {
    if is_reflections_storage_tool(tool) {
        return serde_json::to_string_pretty(&reflections_storage_tool_response_metadata(
            tool,
            call_id,
            success,
            error,
            content_items,
            arguments,
        ))
        .unwrap_or_default();
    }

    format!(
        "call_id: {call_id}\nsuccess: {success}\nerror: {}\n\n{}",
        error.unwrap_or(""),
        dynamic_tool_output_items_to_text(content_items)
    )
}

pub(crate) fn is_reflections_storage_tool(tool: &str) -> bool {
    matches!(
        tool,
        REFLECTIONS_LIST_TOOL_NAME
            | REFLECTIONS_READ_TOOL_NAME
            | REFLECTIONS_SEARCH_TOOL_NAME
            | REFLECTIONS_WRITE_NOTE_TOOL_NAME
    )
}

fn reflections_storage_tool_call_summary(tool: &str, call_id: &str, arguments: &Value) -> String {
    serde_json::to_string_pretty(&reflections_storage_tool_call_metadata(
        tool, call_id, arguments,
    ))
    .unwrap_or_default()
}

fn reflections_storage_tool_output_summary(
    tool: &str,
    call_id: &str,
    output: &FunctionCallOutputPayload,
) -> String {
    let text = output.body.to_text().unwrap_or_default();
    let parsed = serde_json::from_str::<Value>(&text).unwrap_or(Value::Null);
    serde_json::to_string_pretty(&reflections_storage_output_value_metadata(
        tool,
        call_id,
        output.success.unwrap_or(true),
        None,
        &parsed,
    ))
    .unwrap_or_default()
}

pub(super) fn reflections_storage_tool_call_metadata(
    tool: &str,
    call_id: &str,
    arguments: &Value,
) -> Value {
    match tool {
        REFLECTIONS_WRITE_NOTE_TOOL_NAME => json!({
            "tool_name": tool,
            "call_id": call_id,
            "note_id": arguments.get("note_id"),
            "operation": arguments.get("operation"),
            "content_chars": arguments
                .get("content")
                .and_then(Value::as_str)
                .map(str::chars)
                .map(Iterator::count)
                .unwrap_or(0),
        }),
        REFLECTIONS_READ_TOOL_NAME => json!({
            "tool_name": tool,
            "call_id": call_id,
            "kind": arguments.get("kind"),
            "id": arguments.get("id"),
            "start": arguments.get("start"),
            "stop": arguments.get("stop"),
        }),
        REFLECTIONS_LIST_TOOL_NAME => json!({
            "tool_name": tool,
            "call_id": call_id,
            "collection": arguments.get("collection"),
            "start": arguments.get("start"),
            "stop": arguments.get("stop"),
        }),
        REFLECTIONS_SEARCH_TOOL_NAME => json!({
            "tool_name": tool,
            "call_id": call_id,
            "scope": arguments.get("scope"),
            "query": arguments.get("query"),
            "log_id": arguments.get("log_id"),
            "start": arguments.get("start"),
            "stop": arguments.get("stop"),
        }),
        _ => json!({
            "tool_name": tool,
            "call_id": call_id,
            "arguments": arguments,
        }),
    }
}

pub(super) fn reflections_storage_tool_response_metadata(
    tool: &str,
    call_id: &str,
    success: bool,
    error: Option<&str>,
    content_items: &[DynamicToolCallOutputContentItem],
    arguments: &Value,
) -> Value {
    let text = dynamic_tool_output_items_to_text(content_items);
    let parsed = serde_json::from_str::<Value>(&text).unwrap_or(Value::Null);
    let mut metadata =
        reflections_storage_output_value_metadata(tool, call_id, success, error, &parsed);
    if metadata.get("kind").is_none()
        && let Some(kind) = arguments.get("kind")
    {
        metadata["kind"] = kind.clone();
    }
    if metadata.get("id").is_none()
        && let Some(id) = arguments.get("id")
    {
        metadata["id"] = id.clone();
    }
    metadata
}

fn reflections_storage_output_value_metadata(
    tool: &str,
    call_id: &str,
    success: bool,
    error: Option<&str>,
    value: &Value,
) -> Value {
    match tool {
        REFLECTIONS_WRITE_NOTE_TOOL_NAME => json!({
            "tool_name": tool,
            "call_id": call_id,
            "note_id": value.get("id"),
            "operation": value.get("operation"),
            "content_chars": value.get("content_chars"),
            "total_content_chars": value.get("total_content_chars"),
            "line_count": value.get("line_count"),
            "success": success,
            "error": error,
        }),
        REFLECTIONS_READ_TOOL_NAME => json!({
            "tool_name": tool,
            "call_id": call_id,
            "kind": value.get("kind"),
            "id": value.get("id"),
            "start": value.pointer("/range/start"),
            "stop": value.pointer("/range/stop"),
            "returned_entries": value
                .get("entries")
                .and_then(Value::as_array)
                .map(Vec::len),
            "content_chars": value.get("content_chars"),
            "success": success,
            "error": error,
        }),
        REFLECTIONS_LIST_TOOL_NAME => json!({
            "tool_name": tool,
            "call_id": call_id,
            "collection": value.get("collection"),
            "start": value.pointer("/range/start"),
            "stop": value.pointer("/range/stop"),
            "returned_items": value
                .get("items")
                .and_then(Value::as_array)
                .map(Vec::len),
            "success": success,
            "error": error,
        }),
        REFLECTIONS_SEARCH_TOOL_NAME => json!({
            "tool_name": tool,
            "call_id": call_id,
            "scope": value.get("scope"),
            "query": value.get("query"),
            "start": value.pointer("/range/start"),
            "stop": value.pointer("/range/stop"),
            "returned_results": value
                .get("results")
                .and_then(Value::as_array)
                .map(Vec::len),
            "success": success,
            "error": error,
        }),
        _ => json!({
            "tool_name": tool,
            "call_id": call_id,
            "success": success,
            "error": error,
        }),
    }
}

fn entry_id(window: &str, index: usize) -> String {
    format!("{window}:msg-{index:06}")
}
