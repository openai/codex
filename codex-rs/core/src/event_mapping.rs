use crate::protocol::AgentMessageEvent;
use crate::protocol::AgentReasoningEvent;
use crate::protocol::AgentReasoningRawContentEvent;
use crate::protocol::EventMsg;
use crate::protocol::WebSearchEndEvent;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ReasoningItemReasoningSummary;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::WebSearchAction;

/// Convert a `ResponseItem` into zero or more `EventMsg` values that the UI can render.
///
/// When `show_raw_agent_reasoning` is false, raw reasoning content events are omitted.
pub(crate) fn map_response_item_to_event_messages(
    item: &ResponseItem,
    show_raw_agent_reasoning: bool,
) -> Vec<EventMsg> {
    match item {
        ResponseItem::Message { content, .. } => {
            let mut events = Vec::new();
            for content_item in content {
                if let ContentItem::OutputText { text } = content_item {
                    events.push(EventMsg::AgentMessage(AgentMessageEvent {
                        message: text.clone(),
                    }));
                }
            }
            events
        }

        ResponseItem::Reasoning {
            summary, content, ..
        } => {
            let mut events = Vec::new();
            for ReasoningItemReasoningSummary::SummaryText { text } in summary {
                events.push(EventMsg::AgentReasoning(AgentReasoningEvent {
                    text: text.clone(),
                }));
            }
            if let Some(items) = content.as_ref().filter(|_| show_raw_agent_reasoning) {
                for c in items {
                    let text = match c {
                        ReasoningItemContent::ReasoningText { text }
                        | ReasoningItemContent::Text { text } => text,
                    };
                    events.push(EventMsg::AgentReasoningRawContent(
                        AgentReasoningRawContentEvent { text: text.clone() },
                    ));
                }
            }
            events
        }

        ResponseItem::WebSearchCall { id, action, .. } => match action {
            WebSearchAction::Search { query } => {
                let call_id = id.clone().unwrap_or_else(|| "".to_string());
                vec![EventMsg::WebSearchEnd(WebSearchEndEvent {
                    call_id,
                    query: query.clone(),
                })]
            }
            WebSearchAction::Other => Vec::new(),
        },

        // Variants that require side effects are handled by higher layers and do not emit events here.
        ResponseItem::FunctionCall { .. }
        | ResponseItem::FunctionCallOutput { .. }
        | ResponseItem::LocalShellCall { .. }
        | ResponseItem::CustomToolCall { .. }
        | ResponseItem::CustomToolCallOutput { .. }
        | ResponseItem::Other => Vec::new(),
    }
}
