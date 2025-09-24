use std::path::PathBuf;

use crate::event_processor::CodexStatus;
use crate::event_processor::EventProcessor;
use crate::event_processor::handle_last_message;
use crate::exec_events::AssistantMessageItem;
use crate::exec_events::ConversationErrorEvent;
use crate::exec_events::ConversationEvent;
use crate::exec_events::ConversationItem;
use crate::exec_events::ConversationItemDetails;
use crate::exec_events::ItemCompletedEvent;
use crate::exec_events::ReasoningItem;
use crate::exec_events::SessionCreatedEvent;
use codex_core::config::Config;
use codex_core::protocol::AgentMessageEvent;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::SessionConfiguredEvent;
use codex_core::protocol::TaskCompleteEvent;

pub(crate) struct EventProcessorWithJsonOutput {
    last_message_path: Option<PathBuf>,
    next_event_id: u64,
}

impl EventProcessorWithJsonOutput {
    pub fn new(last_message_path: Option<PathBuf>) -> Self {
        Self {
            last_message_path,
            next_event_id: 0,
        }
    }

    fn collect_conversation_events(&mut self, event: &Event) -> Vec<ConversationEvent> {
        match &event.msg {
            EventMsg::SessionConfigured(ev) => self.handle_session_configured(ev),
            EventMsg::AgentMessage(ev) => self.handle_agent_message(ev),
            EventMsg::AgentReasoning(ev) => self.handle_reasoning_event(ev.text.clone()),
            EventMsg::Error(ev) => vec![ConversationEvent::Error(ConversationErrorEvent {
                message: ev.message.clone(),
            })],
            EventMsg::StreamError(ev) => vec![ConversationEvent::Error(ConversationErrorEvent {
                message: ev.message.clone(),
            })],
            _ => Vec::new(),
        }
    }

    fn get_next_item_id(&mut self) -> String {
        let id = format!("itm_{}", self.next_event_id);
        self.next_event_id += 1;
        id
    }

    fn handle_session_configured(
        &mut self,
        payload: &SessionConfiguredEvent,
    ) -> Vec<ConversationEvent> {
        vec![ConversationEvent::SessionCreated(SessionCreatedEvent {
            session_id: payload.session_id.to_string(),
        })]
    }

    fn handle_agent_message(&mut self, payload: &AgentMessageEvent) -> Vec<ConversationEvent> {
        let item = ConversationItem {
            id: self.get_next_item_id(),

            details: ConversationItemDetails::AssistantMessage(AssistantMessageItem {
                text: payload.message.clone(),
            }),
        };

        vec![ConversationEvent::ItemCompleted(ItemCompletedEvent {
            item,
        })]
    }

    fn handle_reasoning_event(&mut self, summary_text: String) -> Vec<ConversationEvent> {
        let item = ConversationItem {
            id: self.get_next_item_id(),

            details: ConversationItemDetails::Reasoning(ReasoningItem { summary_text }),
        };

        vec![ConversationEvent::ItemCompleted(ItemCompletedEvent {
            item,
        })]
    }
}

impl EventProcessor for EventProcessorWithJsonOutput {
    fn print_config_summary(&mut self, _: &Config, _: &str) {}

    fn process_event(&mut self, event: Event) -> CodexStatus {
        let aggregated = self.collect_conversation_events(&event);
        for conv_event in aggregated {
            if let Ok(line) = serde_json::to_string(&conv_event) {
                println!("{line}");
            }
        }

        let Event { msg, .. } = event;

        if let EventMsg::TaskComplete(TaskCompleteEvent { last_agent_message }) = msg {
            if let Some(output_file) = self.last_message_path.as_deref() {
                handle_last_message(last_agent_message.as_deref(), output_file);
            }
            return CodexStatus::InitiateShutdown;
        }
        CodexStatus::Running
    }
}
