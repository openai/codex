use std::path::Path;

use codex_analytics::CompactionTrigger;
use codex_protocol::dynamic_tools::DynamicToolCallOutputContentItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use serde::Serialize;
use serde_json::json;

pub(crate) struct TranscriptInput<'a> {
    pub(crate) events: &'a [EventMsg],
    pub(crate) trigger: CompactionTrigger,
    pub(crate) context_window_size: Option<i64>,
    pub(crate) rollout_path: &'a Path,
}

pub(crate) fn events_since_last_compaction(items: &[RolloutItem]) -> Vec<EventMsg> {
    let start_index = items
        .iter()
        .rposition(|item| matches!(item, RolloutItem::Compacted(_)))
        .map_or(0, |index| index + 1);

    items[start_index..]
        .iter()
        .filter_map(|item| match item {
            RolloutItem::EventMsg(event) => Some(event.clone()),
            RolloutItem::SessionMeta(_)
            | RolloutItem::ResponseItem(_)
            | RolloutItem::Compacted(_)
            | RolloutItem::TurnContext(_) => None,
        })
        .collect()
}

pub(crate) fn render(input: TranscriptInput<'_>) -> String {
    let mut out = String::new();
    out.push_str("# Reflections Log\n\n");
    out.push_str("- schema: reflections.transcript.v1\n");
    out.push_str(&format!("- trigger: {}\n", trigger_label(input.trigger)));
    out.push_str(&format!(
        "- context_window_size: {}\n",
        input
            .context_window_size
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unavailable".to_string())
    ));
    out.push_str(&format!(
        "- source_rollout: {}\n",
        input.rollout_path.display()
    ));
    out.push('\n');

    let mut index = 1usize;
    for event in input.events {
        if push_event(&mut out, index, event) {
            index += 1;
        }
    }

    out
}

fn push_event(out: &mut String, index: usize, event: &EventMsg) -> bool {
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
            push_message(out, index, "user", &text);
            true
        }
        EventMsg::AgentMessage(event) => {
            let heading = event.phase.as_ref().map_or_else(
                || "assistant".to_string(),
                |phase| format!("assistant {phase:?}").to_lowercase(),
            );
            push_message(out, index, &heading, &event.message);
            true
        }
        EventMsg::McpToolCallBegin(event) => {
            push_json(
                out,
                index,
                &format!(
                    "tool_call mcp.{}.{}",
                    event.invocation.server, event.invocation.tool
                ),
                &json!({
                    "call_id": event.call_id,
                    "arguments": event.invocation.arguments,
                }),
            );
            true
        }
        EventMsg::McpToolCallEnd(event) => {
            push_json(
                out,
                index,
                &format!(
                    "tool_result mcp.{}.{}",
                    event.invocation.server, event.invocation.tool
                ),
                &json!({
                    "call_id": event.call_id,
                    "arguments": event.invocation.arguments,
                    "success": event.is_success(),
                    "result": event.result,
                }),
            );
            true
        }
        EventMsg::WebSearchBegin(event) => {
            push_json(out, index, "tool_call web_search", event);
            true
        }
        EventMsg::WebSearchEnd(event) => {
            push_json(
                out,
                index,
                "tool_result web_search",
                &json!({
                    "call_id": event.call_id,
                    "query": event.query,
                    "action": event.action,
                }),
            );
            true
        }
        EventMsg::ImageGenerationBegin(event) => {
            push_json(out, index, "tool_call image_generation", event);
            true
        }
        EventMsg::ImageGenerationEnd(event) => {
            push_json(
                out,
                index,
                "tool_result image_generation",
                &json!({
                    "call_id": event.call_id,
                    "status": event.status,
                    "revised_prompt": event.revised_prompt,
                    "result": event.result,
                    "saved_path": event.saved_path.as_ref().map(|path| path.display().to_string()),
                }),
            );
            true
        }
        EventMsg::ExecCommandBegin(event) => {
            let mut text = String::new();
            text.push_str(&format!("call_id: {}\n", event.call_id));
            text.push_str(&format!("cwd: {}\n", event.cwd.display()));
            text.push_str(&format!(
                "command: {}\n",
                serde_json::to_string(&event.command).unwrap_or_else(|_| "[]".to_string())
            ));
            if let Some(input) = event.interaction_input.as_deref() {
                text.push_str(&format!("interaction_input: {input}\n"));
            }
            push_fenced(out, index, "tool_call exec_command", "text", &text);
            true
        }
        EventMsg::ExecCommandEnd(event) => {
            let mut text = String::new();
            text.push_str(&format!("call_id: {}\n", event.call_id));
            text.push_str(&format!("status: {:?}\n", event.status));
            text.push_str(&format!("exit_code: {}\n", event.exit_code));
            text.push_str(&format!("cwd: {}\n", event.cwd.display()));
            text.push_str(&format!(
                "command: {}\n",
                serde_json::to_string(&event.command).unwrap_or_else(|_| "[]".to_string())
            ));
            let output = exec_output(event);
            if !output.is_empty() {
                text.push_str("\noutput:\n");
                text.push_str(&output);
                if !output.ends_with('\n') {
                    text.push('\n');
                }
            }
            push_fenced(out, index, "tool_result exec_command", "text", &text);
            true
        }
        EventMsg::ViewImageToolCall(event) => {
            push_json(
                out,
                index,
                "tool_call view_image",
                &json!({
                    "call_id": event.call_id,
                    "path": event.path.display().to_string(),
                }),
            );
            true
        }
        EventMsg::DynamicToolCallRequest(event) => {
            push_json(
                out,
                index,
                &format!("tool_call {}", event.tool),
                &json!({
                    "call_id": event.call_id,
                    "arguments": event.arguments,
                }),
            );
            true
        }
        EventMsg::DynamicToolCallResponse(event) => {
            let text = format!(
                "call_id: {}\nsuccess: {}\nerror: {}\n\n{}",
                event.call_id,
                event.success,
                event.error.as_deref().unwrap_or(""),
                dynamic_tool_output_items_to_text(&event.content_items)
            );
            push_fenced(
                out,
                index,
                &format!("tool_result {}", event.tool),
                "text",
                &text,
            );
            true
        }
        EventMsg::PatchApplyBegin(event) => {
            push_json(
                out,
                index,
                "tool_call apply_patch",
                &json!({
                    "call_id": event.call_id,
                    "auto_approved": event.auto_approved,
                    "changes": event.changes,
                }),
            );
            true
        }
        EventMsg::PatchApplyEnd(event) => {
            push_json(
                out,
                index,
                "tool_result apply_patch",
                &json!({
                    "call_id": event.call_id,
                    "success": event.success,
                    "status": event.status,
                    "stdout": event.stdout,
                    "stderr": event.stderr,
                    "changes": event.changes,
                }),
            );
            true
        }
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
        | EventMsg::CollabResumeEnd(_) => false,
    }
}

fn push_message(out: &mut String, index: usize, heading: &str, text: &str) {
    push_fenced(out, index, heading, "text", text);
}

fn push_fenced(out: &mut String, index: usize, heading: &str, language: &str, text: &str) {
    out.push_str(&format!("## msg-{index:06} {heading}\n\n"));
    let fence = fence_for(text);
    out.push_str(&format!("{fence}{language}\n"));
    out.push_str(text);
    if !text.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&format!("{fence}\n\n"));
}

fn push_json<T: Serialize>(out: &mut String, index: usize, heading: &str, value: &T) {
    let text = serde_json::to_string_pretty(value)
        .unwrap_or_else(|err| format!("<failed to serialize visible event: {err}>"));
    push_fenced(out, index, heading, "json", &text);
}

fn push_blank_line_if_needed(text: &mut String) {
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    if !text.is_empty() {
        text.push('\n');
    }
}

fn exec_output(event: &codex_protocol::protocol::ExecCommandEndEvent) -> String {
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

fn dynamic_tool_output_items_to_text(items: &[DynamicToolCallOutputContentItem]) -> String {
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

fn fence_for(text: &str) -> String {
    let longest_run = longest_backtick_run(text);
    "`".repeat(longest_run.max(2) + 1)
}

fn longest_backtick_run(text: &str) -> usize {
    let mut longest = 0usize;
    let mut current = 0usize;
    for ch in text.chars() {
        if ch == '`' {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }
    longest
}

fn trigger_label(trigger: CompactionTrigger) -> &'static str {
    match trigger {
        CompactionTrigger::Manual => "manual_compact",
        CompactionTrigger::Auto => "auto_compact",
    }
}

#[cfg(test)]
mod tests {
    use super::TranscriptInput;
    use super::events_since_last_compaction;
    use super::render;
    use codex_analytics::CompactionTrigger;
    use codex_protocol::dynamic_tools::DynamicToolCallOutputContentItem;
    use codex_protocol::dynamic_tools::DynamicToolCallRequest;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ReasoningItemContent;
    use codex_protocol::models::ReasoningItemReasoningSummary;
    use codex_protocol::models::ResponseItem;
    use codex_protocol::protocol::AgentMessageEvent;
    use codex_protocol::protocol::AgentReasoningEvent;
    use codex_protocol::protocol::CompactedItem;
    use codex_protocol::protocol::DynamicToolCallResponseEvent;
    use codex_protocol::protocol::EventMsg;
    use codex_protocol::protocol::RolloutItem;
    use codex_protocol::protocol::UserMessageEvent;
    use std::path::Path;
    use std::time::Duration;

    #[test]
    fn transcript_renders_visible_events_only() {
        let items = vec![
            RolloutItem::ResponseItem(ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "AGENTS.md instructions should be omitted".to_string(),
                }],
                end_turn: None,
                phase: None,
            }),
            RolloutItem::ResponseItem(ResponseItem::Reasoning {
                id: "rs_1".to_string(),
                summary: vec![ReasoningItemReasoningSummary::SummaryText {
                    text: "checked tests".to_string(),
                }],
                content: Some(vec![ReasoningItemContent::ReasoningText {
                    text: "hidden chain of thought".to_string(),
                }]),
                encrypted_content: None,
            }),
            RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
                message: "please test".to_string(),
                images: None,
                local_images: Vec::new(),
                text_elements: Vec::new(),
            })),
            RolloutItem::EventMsg(EventMsg::DynamicToolCallRequest(DynamicToolCallRequest {
                call_id: "call-1".to_string(),
                turn_id: "turn-1".to_string(),
                tool: "lookup_ticket".to_string(),
                arguments: serde_json::json!({"id": "T-1"}),
            })),
            RolloutItem::EventMsg(EventMsg::DynamicToolCallResponse(
                DynamicToolCallResponseEvent {
                    call_id: "call-1".to_string(),
                    turn_id: "turn-1".to_string(),
                    tool: "lookup_ticket".to_string(),
                    arguments: serde_json::json!({"id": "T-1"}),
                    content_items: vec![DynamicToolCallOutputContentItem::InputText {
                        text: "ok".to_string(),
                    }],
                    success: true,
                    error: None,
                    duration: Duration::from_millis(10),
                },
            )),
            RolloutItem::EventMsg(EventMsg::AgentMessage(AgentMessageEvent {
                message: "done".to_string(),
                phase: None,
                memory_citation: None,
            })),
            RolloutItem::EventMsg(EventMsg::AgentReasoning(AgentReasoningEvent {
                text: "reasoning summary should be omitted".to_string(),
            })),
        ];

        let events = events_since_last_compaction(&items);
        let transcript = render(TranscriptInput {
            events: &events,
            trigger: CompactionTrigger::Manual,
            context_window_size: Some(98304),
            rollout_path: Path::new("/tmp/rollout.jsonl"),
        });

        assert!(transcript.contains("## msg-000001 user"));
        assert!(transcript.contains("## msg-000002 tool_call lookup_ticket"));
        assert!(transcript.contains("## msg-000003 tool_result lookup_ticket"));
        assert!(transcript.contains("## msg-000004 assistant"));
        assert!(transcript.contains("done"));
        assert!(!transcript.contains("AGENTS.md instructions should be omitted"));
        assert!(!transcript.contains("checked tests"));
        assert!(!transcript.contains("hidden chain of thought"));
        assert!(!transcript.contains("reasoning summary should be omitted"));
    }

    #[test]
    fn transcript_uses_events_after_last_compaction() {
        let items = vec![
            RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
                message: "old window".to_string(),
                images: None,
                local_images: Vec::new(),
                text_elements: Vec::new(),
            })),
            RolloutItem::Compacted(CompactedItem {
                message: "handoff".to_string(),
                replacement_history: None,
            }),
            RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
                message: "current window".to_string(),
                images: None,
                local_images: Vec::new(),
                text_elements: Vec::new(),
            })),
        ];

        let events = events_since_last_compaction(&items);
        let transcript = render(TranscriptInput {
            events: &events,
            trigger: CompactionTrigger::Auto,
            context_window_size: None,
            rollout_path: Path::new("/tmp/rollout.jsonl"),
        });

        assert!(transcript.contains("current window"));
        assert!(!transcript.contains("old window"));
    }
}
