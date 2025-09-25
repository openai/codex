use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;

use crate::event_processor::CodexStatus;
use crate::event_processor::EventProcessor;
use crate::event_processor::handle_last_message;
use crate::exec_events::AssistantMessageItem;
use crate::exec_events::CommandExecutionItem;
use crate::exec_events::CommandExecutionStatus;
use crate::exec_events::ConversationErrorEvent;
use crate::exec_events::ConversationEvent;
use crate::exec_events::ConversationItem;
use crate::exec_events::ConversationItemDetails;
use crate::exec_events::FileChangeItem;
use crate::exec_events::FileUpdateChange;
use crate::exec_events::ItemCompletedEvent;
use crate::exec_events::PatchApplyStatus;
use crate::exec_events::PatchChangeKind;
use crate::exec_events::ReasoningItem;
use crate::exec_events::SessionCreatedEvent;
use codex_core::config::Config;
use codex_core::protocol::AgentMessageEvent;
use codex_core::protocol::AgentReasoningEvent;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ExecCommandBeginEvent;
use codex_core::protocol::ExecCommandEndEvent;
use codex_core::protocol::FileChange;
use codex_core::protocol::PatchApplyBeginEvent;
use codex_core::protocol::PatchApplyEndEvent;
use codex_core::protocol::SessionConfiguredEvent;
use codex_core::protocol::TaskCompleteEvent;
use tracing::error;

pub struct ExperimentalEventProcessorWithJsonOutput {
    last_message_path: Option<PathBuf>,
    next_event_id: AtomicU64,
    running_commands: HashMap<String, Vec<String>>,
    running_patch_applies: HashMap<String, PatchApplyBeginEvent>,
}

impl ExperimentalEventProcessorWithJsonOutput {
    pub fn new(last_message_path: Option<PathBuf>) -> Self {
        Self {
            last_message_path,
            next_event_id: AtomicU64::new(0),
            running_commands: HashMap::new(),
            running_patch_applies: HashMap::new(),
        }
    }

    pub fn collect_conversation_events(&mut self, event: &Event) -> Vec<ConversationEvent> {
        match &event.msg {
            EventMsg::SessionConfigured(ev) => self.handle_session_configured(ev),
            EventMsg::AgentMessage(ev) => self.handle_agent_message(ev),
            EventMsg::AgentReasoning(ev) => self.handle_reasoning_event(ev),
            EventMsg::ExecCommandBegin(ev) => self.handle_exec_command_begin(ev),
            EventMsg::ExecCommandEnd(ev) => self.handle_exec_command_end(ev),
            EventMsg::PatchApplyBegin(ev) => self.handle_patch_apply_begin(ev),
            EventMsg::PatchApplyEnd(ev) => self.handle_patch_apply_end(ev),
            EventMsg::Error(ev) => vec![ConversationEvent::Error(ConversationErrorEvent {
                message: ev.message.clone(),
            })],
            EventMsg::StreamError(ev) => vec![ConversationEvent::Error(ConversationErrorEvent {
                message: ev.message.clone(),
            })],
            _ => Vec::new(),
        }
    }

    fn get_next_item_id(&self) -> String {
        format!(
            "item_{}",
            self.next_event_id
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        )
    }

    fn handle_session_configured(
        &self,
        payload: &SessionConfiguredEvent,
    ) -> Vec<ConversationEvent> {
        vec![ConversationEvent::SessionCreated(SessionCreatedEvent {
            session_id: payload.session_id.to_string(),
        })]
    }

    fn handle_agent_message(&self, payload: &AgentMessageEvent) -> Vec<ConversationEvent> {
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

    fn handle_reasoning_event(&self, ev: &AgentReasoningEvent) -> Vec<ConversationEvent> {
        let item = ConversationItem {
            id: self.get_next_item_id(),

            details: ConversationItemDetails::Reasoning(ReasoningItem {
                text: ev.text.clone(),
            }),
        };

        vec![ConversationEvent::ItemCompleted(ItemCompletedEvent {
            item,
        })]
    }
    fn handle_exec_command_begin(&mut self, ev: &ExecCommandBeginEvent) -> Vec<ConversationEvent> {
        self.running_commands
            .insert(ev.call_id.clone(), ev.command.clone());

        Vec::new()
    }

    fn handle_patch_apply_begin(&mut self, ev: &PatchApplyBeginEvent) -> Vec<ConversationEvent> {
        self.running_patch_applies
            .insert(ev.call_id.clone(), ev.clone());

        Vec::new()
    }

    fn map_change_kind(&self, kind: &FileChange) -> PatchChangeKind {
        match kind {
            FileChange::Add { .. } => PatchChangeKind::Add,
            FileChange::Delete { .. } => PatchChangeKind::Delete,
            FileChange::Update { .. } => PatchChangeKind::Update,
        }
    }

    fn handle_patch_apply_end(&mut self, ev: &PatchApplyEndEvent) -> Vec<ConversationEvent> {
        if let Some(running_patch_apply) = self.running_patch_applies.remove(&ev.call_id) {
            let status = if ev.success {
                PatchApplyStatus::Completed
            } else {
                PatchApplyStatus::Failed
            };
            let item = ConversationItem {
                id: self.get_next_item_id(),

                details: ConversationItemDetails::FileChange(FileChangeItem {
                    changes: running_patch_apply
                        .changes
                        .iter()
                        .map(|(path, change)| FileUpdateChange {
                            path: path.to_str().unwrap_or("").to_string(),
                            kind: self.map_change_kind(change),
                        })
                        .collect(),
                    status,
                }),
            };

            return vec![ConversationEvent::ItemCompleted(ItemCompletedEvent {
                item,
            })];
        }

        Vec::new()
    }

    fn handle_exec_command_end(&mut self, ev: &ExecCommandEndEvent) -> Vec<ConversationEvent> {
        let command = self
            .running_commands
            .remove(&ev.call_id)
            .map(|command| command.join(" "))
            .unwrap_or_default();
        let status = if ev.exit_code == 0 {
            CommandExecutionStatus::Completed
        } else {
            CommandExecutionStatus::Failed
        };
        let item = ConversationItem {
            id: self.get_next_item_id(),

            details: ConversationItemDetails::CommandExecution(CommandExecutionItem {
                command,
                aggregated_output: ev.aggregated_output.clone(),
                exit_code: ev.exit_code,
                status,
            }),
        };

        vec![ConversationEvent::ItemCompleted(ItemCompletedEvent {
            item,
        })]
    }
}

impl EventProcessor for ExperimentalEventProcessorWithJsonOutput {
    fn print_config_summary(&mut self, _: &Config, _: &str, ev: &SessionConfiguredEvent) {
        self.process_event(Event {
            id: "".to_string(),
            msg: EventMsg::SessionConfigured(ev.clone()),
        });
    }

    fn process_event(&mut self, event: Event) -> CodexStatus {
        let aggregated = self.collect_conversation_events(&event);
        for conv_event in aggregated {
            match serde_json::to_string(&conv_event) {
                Ok(line) => {
                    println!("{line}");
                }
                Err(e) => {
                    error!("Failed to serialize event: {e:?}");
                }
            }
        }

        let Event { msg, .. } = event;

        if let EventMsg::TaskComplete(TaskCompleteEvent { last_agent_message }) = msg {
            if let Some(output_file) = self.last_message_path.as_deref() {
                handle_last_message(last_agent_message.as_deref(), output_file);
            }
            CodexStatus::InitiateShutdown
        } else {
            CodexStatus::Running
        }
    }
}
