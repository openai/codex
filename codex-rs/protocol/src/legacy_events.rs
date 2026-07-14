use crate::ThreadId;
use crate::dynamic_tools::DynamicToolCallRequest;
use crate::items::AgentMessageContent;
use crate::items::AgentMessageItem;
use crate::items::CollabAgentTool;
use crate::items::CollabAgentToolCallItem;
use crate::items::CollabAgentToolCallStatus;
use crate::items::CommandExecutionItem;
use crate::items::CommandExecutionStatus;
use crate::items::ContextCompactionItem;
use crate::items::DynamicToolCallItem;
use crate::items::DynamicToolCallStatus;
use crate::items::EnteredReviewModeItem;
use crate::items::ExitedReviewModeItem;
use crate::items::FileChangeItem;
use crate::items::ImageGenerationItem;
use crate::items::McpToolCallItem;
use crate::items::ReasoningItem;
use crate::items::SubAgentActivityItem;
use crate::items::TurnItem;
use crate::items::UserMessageItem;
use crate::items::WebSearchItem;
use crate::protocol::AgentMessageContentDeltaEvent;
use crate::protocol::AgentMessageEvent;
use crate::protocol::AgentReasoningEvent;
use crate::protocol::AgentReasoningRawContentEvent;
use crate::protocol::AgentStatus;
use crate::protocol::CollabAgentInteractionBeginEvent;
use crate::protocol::CollabAgentInteractionEndEvent;
use crate::protocol::CollabAgentSpawnBeginEvent;
use crate::protocol::CollabAgentSpawnEndEvent;
use crate::protocol::CollabAgentStatusEntry;
use crate::protocol::CollabCloseBeginEvent;
use crate::protocol::CollabCloseEndEvent;
use crate::protocol::CollabResumeBeginEvent;
use crate::protocol::CollabResumeEndEvent;
use crate::protocol::CollabWaitingBeginEvent;
use crate::protocol::CollabWaitingEndEvent;
use crate::protocol::ContextCompactedEvent;
use crate::protocol::DynamicToolCallResponseEvent;
use crate::protocol::EnteredReviewModeEvent;
use crate::protocol::EventMsg;
use crate::protocol::ExecCommandBeginEvent;
use crate::protocol::ExecCommandEndEvent;
use crate::protocol::ExecCommandStatus;
use crate::protocol::ExitedReviewModeEvent;
use crate::protocol::ImageGenerationBeginEvent;
use crate::protocol::ImageGenerationEndEvent;
use crate::protocol::ItemCompletedEvent;
use crate::protocol::ItemStartedEvent;
use crate::protocol::McpInvocation;
use crate::protocol::McpToolCallBeginEvent;
use crate::protocol::McpToolCallEndEvent;
use crate::protocol::PatchApplyBeginEvent;
use crate::protocol::PatchApplyEndEvent;
use crate::protocol::PatchApplyStatus;
use crate::protocol::ReasoningContentDeltaEvent;
use crate::protocol::ReasoningRawContentDeltaEvent;
use crate::protocol::SubAgentActivityEvent;
use crate::protocol::UserMessageEvent;
use crate::protocol::ViewImageToolCallEvent;
use crate::protocol::WebSearchBeginEvent;
use crate::protocol::WebSearchEndEvent;

/// Converts canonical item lifecycle events back into the legacy raw event stream used by
/// compatibility consumers that have not migrated to `TurnItem`.
pub trait HasLegacyEvent {
    fn as_legacy_events(&self, show_raw_agent_reasoning: bool) -> Vec<EventMsg>;
}

impl ContextCompactionItem {
    pub fn as_legacy_event(&self) -> EventMsg {
        EventMsg::ContextCompacted(ContextCompactedEvent {})
    }
}

impl UserMessageItem {
    pub fn as_legacy_user_message_event(&self) -> UserMessageEvent {
        // Legacy user-message events flatten only text inputs into `message` and
        // rebase text element ranges onto that concatenated text.
        UserMessageEvent {
            client_id: self.client_id.clone(),
            message: self.message(),
            images: Some(self.image_urls()),
            image_details: self.image_details(),
            local_images: self.local_image_paths(),
            local_image_details: self.local_image_details(),
            text_elements: self.text_elements(),
        }
    }

    pub fn as_legacy_event(&self) -> EventMsg {
        EventMsg::UserMessage(self.as_legacy_user_message_event())
    }
}

impl AgentMessageItem {
    pub fn as_legacy_events(&self) -> Vec<EventMsg> {
        self.content
            .iter()
            .map(|c| match c {
                AgentMessageContent::Text { text } => EventMsg::AgentMessage(AgentMessageEvent {
                    message: text.clone(),
                    phase: self.phase.clone(),
                    memory_citation: self.memory_citation.clone(),
                }),
            })
            .collect()
    }
}

impl EnteredReviewModeItem {
    pub fn as_legacy_event(&self, turn_id: String) -> EventMsg {
        EventMsg::EnteredReviewMode(EnteredReviewModeEvent {
            target: self.target.clone(),
            user_facing_hint: Some(self.user_facing_hint.clone()),
            turn_id: Some(turn_id),
            item_id: Some(self.id.clone()),
        })
    }
}

impl ExitedReviewModeItem {
    pub fn as_legacy_event(&self, turn_id: String) -> EventMsg {
        EventMsg::ExitedReviewMode(ExitedReviewModeEvent {
            turn_id: Some(turn_id),
            item_id: Some(self.id.clone()),
            review_output: self.review_output.clone(),
        })
    }
}

impl ReasoningItem {
    pub fn as_legacy_events(&self, show_raw_agent_reasoning: bool) -> Vec<EventMsg> {
        let mut events = Vec::new();
        for summary in &self.summary_text {
            events.push(EventMsg::AgentReasoning(AgentReasoningEvent {
                text: summary.clone(),
            }));
        }

        if show_raw_agent_reasoning {
            for entry in &self.raw_content {
                events.push(EventMsg::AgentReasoningRawContent(
                    AgentReasoningRawContentEvent {
                        text: entry.clone(),
                    },
                ));
            }
        }

        events
    }
}

impl CommandExecutionItem {
    pub(crate) fn as_legacy_begin_event(&self, turn_id: String, started_at_ms: i64) -> EventMsg {
        EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
            call_id: self.id.clone(),
            process_id: self.process_id.clone(),
            turn_id,
            started_at_ms,
            command: self.command.clone(),
            cwd: self.cwd.clone(),
            parsed_cmd: self.parsed_cmd.clone(),
            source: self.source,
            interaction_input: self.interaction_input.clone(),
        })
    }

    pub(crate) fn as_legacy_end_event(
        &self,
        turn_id: String,
        completed_at_ms: i64,
    ) -> Option<EventMsg> {
        let status = match self.status {
            CommandExecutionStatus::InProgress => return None,
            CommandExecutionStatus::Completed => ExecCommandStatus::Completed,
            CommandExecutionStatus::Failed => ExecCommandStatus::Failed,
            CommandExecutionStatus::Declined => ExecCommandStatus::Declined,
        };
        Some(EventMsg::ExecCommandEnd(ExecCommandEndEvent {
            call_id: self.id.clone(),
            process_id: self.process_id.clone(),
            turn_id,
            completed_at_ms,
            command: self.command.clone(),
            cwd: self.cwd.clone(),
            parsed_cmd: self.parsed_cmd.clone(),
            source: self.source,
            interaction_input: self.interaction_input.clone(),
            stdout: self.stdout.clone().unwrap_or_default(),
            stderr: self.stderr.clone().unwrap_or_default(),
            aggregated_output: self.aggregated_output.clone().unwrap_or_default(),
            exit_code: self.exit_code.unwrap_or_default(),
            duration: self.duration.unwrap_or_default(),
            formatted_output: self.formatted_output.clone().unwrap_or_default(),
            status,
        }))
    }
}

impl DynamicToolCallItem {
    pub(crate) fn as_legacy_request_event(&self, turn_id: String, started_at_ms: i64) -> EventMsg {
        EventMsg::DynamicToolCallRequest(DynamicToolCallRequest {
            call_id: self.id.clone(),
            turn_id,
            started_at_ms,
            namespace: self.namespace.clone(),
            tool: self.tool.clone(),
            arguments: self.arguments.clone(),
        })
    }

    pub(crate) fn as_legacy_response_event(
        &self,
        turn_id: String,
        completed_at_ms: i64,
    ) -> Option<EventMsg> {
        if matches!(self.status, DynamicToolCallStatus::InProgress) {
            return None;
        }
        Some(EventMsg::DynamicToolCallResponse(
            DynamicToolCallResponseEvent {
                call_id: self.id.clone(),
                turn_id,
                completed_at_ms,
                namespace: self.namespace.clone(),
                tool: self.tool.clone(),
                arguments: self.arguments.clone(),
                content_items: self.content_items.clone().unwrap_or_default(),
                success: self.success.unwrap_or(false),
                error: self.error.clone(),
                duration: self.duration.unwrap_or_default(),
            },
        ))
    }
}

impl CollabAgentToolCallItem {
    pub(crate) fn as_legacy_begin_event(&self, started_at_ms: i64) -> Option<EventMsg> {
        let receiver_thread_id = self.receiver_thread_ids.first().copied();
        match self.tool {
            CollabAgentTool::SpawnAgent => Some(EventMsg::CollabAgentSpawnBegin(
                CollabAgentSpawnBeginEvent {
                    call_id: self.id.clone(),
                    started_at_ms,
                    sender_thread_id: self.sender_thread_id,
                    prompt: self.prompt.clone().unwrap_or_default(),
                    model: self.model.clone().unwrap_or_default(),
                    reasoning_effort: self.reasoning_effort.clone().unwrap_or_default(),
                },
            )),
            CollabAgentTool::SendInput => receiver_thread_id.map(|receiver_thread_id| {
                EventMsg::CollabAgentInteractionBegin(CollabAgentInteractionBeginEvent {
                    call_id: self.id.clone(),
                    started_at_ms,
                    sender_thread_id: self.sender_thread_id,
                    receiver_thread_id,
                    prompt: self.prompt.clone().unwrap_or_default(),
                })
            }),
            CollabAgentTool::ResumeAgent => receiver_thread_id.map(|receiver_thread_id| {
                let (receiver_agent_nickname, receiver_agent_role) =
                    self.receiver_agent_identity(receiver_thread_id);
                EventMsg::CollabResumeBegin(CollabResumeBeginEvent {
                    call_id: self.id.clone(),
                    started_at_ms,
                    sender_thread_id: self.sender_thread_id,
                    receiver_thread_id,
                    receiver_agent_nickname,
                    receiver_agent_role,
                })
            }),
            CollabAgentTool::Wait => Some(EventMsg::CollabWaitingBegin(CollabWaitingBeginEvent {
                started_at_ms,
                sender_thread_id: self.sender_thread_id,
                receiver_thread_ids: self.receiver_thread_ids.clone(),
                receiver_agents: self.receiver_agents.clone(),
                call_id: self.id.clone(),
            })),
            CollabAgentTool::CloseAgent => receiver_thread_id.map(|receiver_thread_id| {
                EventMsg::CollabCloseBegin(CollabCloseBeginEvent {
                    call_id: self.id.clone(),
                    started_at_ms,
                    sender_thread_id: self.sender_thread_id,
                    receiver_thread_id,
                })
            }),
        }
    }

    pub(crate) fn as_legacy_end_event(&self, completed_at_ms: i64) -> Option<EventMsg> {
        if matches!(self.status, CollabAgentToolCallStatus::InProgress) {
            return None;
        }
        let receiver_thread_id = self.receiver_thread_ids.first().copied();
        match self.tool {
            CollabAgentTool::SpawnAgent => {
                let (new_agent_nickname, new_agent_role) = receiver_thread_id
                    .map(|thread_id| self.receiver_agent_identity(thread_id))
                    .unwrap_or_default();
                Some(EventMsg::CollabAgentSpawnEnd(CollabAgentSpawnEndEvent {
                    call_id: self.id.clone(),
                    completed_at_ms,
                    sender_thread_id: self.sender_thread_id,
                    new_thread_id: receiver_thread_id,
                    new_agent_nickname,
                    new_agent_role,
                    prompt: self.prompt.clone().unwrap_or_default(),
                    model: self.model.clone().unwrap_or_default(),
                    reasoning_effort: self.reasoning_effort.clone().unwrap_or_default(),
                    status: receiver_thread_id
                        .map(|thread_id| self.agent_status(thread_id))
                        .unwrap_or(AgentStatus::NotFound),
                }))
            }
            CollabAgentTool::SendInput => receiver_thread_id.map(|receiver_thread_id| {
                let (receiver_agent_nickname, receiver_agent_role) =
                    self.receiver_agent_identity(receiver_thread_id);
                EventMsg::CollabAgentInteractionEnd(CollabAgentInteractionEndEvent {
                    call_id: self.id.clone(),
                    completed_at_ms,
                    sender_thread_id: self.sender_thread_id,
                    receiver_thread_id,
                    receiver_agent_nickname,
                    receiver_agent_role,
                    prompt: self.prompt.clone().unwrap_or_default(),
                    status: self.agent_status(receiver_thread_id),
                })
            }),
            CollabAgentTool::ResumeAgent => receiver_thread_id.map(|receiver_thread_id| {
                let (receiver_agent_nickname, receiver_agent_role) =
                    self.receiver_agent_identity(receiver_thread_id);
                EventMsg::CollabResumeEnd(CollabResumeEndEvent {
                    call_id: self.id.clone(),
                    completed_at_ms,
                    sender_thread_id: self.sender_thread_id,
                    receiver_thread_id,
                    receiver_agent_nickname,
                    receiver_agent_role,
                    status: self.agent_status(receiver_thread_id),
                })
            }),
            CollabAgentTool::Wait => Some(EventMsg::CollabWaitingEnd(CollabWaitingEndEvent {
                sender_thread_id: self.sender_thread_id,
                call_id: self.id.clone(),
                completed_at_ms,
                agent_statuses: self
                    .receiver_agents
                    .iter()
                    .map(|agent| CollabAgentStatusEntry {
                        thread_id: agent.thread_id,
                        agent_nickname: agent.agent_nickname.clone(),
                        agent_role: agent.agent_role.clone(),
                        status: self.agent_status(agent.thread_id),
                    })
                    .collect(),
                statuses: self.agents_states.clone(),
            })),
            CollabAgentTool::CloseAgent => receiver_thread_id.map(|receiver_thread_id| {
                let (receiver_agent_nickname, receiver_agent_role) =
                    self.receiver_agent_identity(receiver_thread_id);
                EventMsg::CollabCloseEnd(CollabCloseEndEvent {
                    call_id: self.id.clone(),
                    completed_at_ms,
                    sender_thread_id: self.sender_thread_id,
                    receiver_thread_id,
                    receiver_agent_nickname,
                    receiver_agent_role,
                    status: self.agent_status(receiver_thread_id),
                })
            }),
        }
    }

    fn receiver_agent_identity(&self, thread_id: ThreadId) -> (Option<String>, Option<String>) {
        let receiver_agent = self
            .receiver_agents
            .iter()
            .find(|agent| agent.thread_id == thread_id);
        (
            receiver_agent.and_then(|agent| agent.agent_nickname.clone()),
            receiver_agent.and_then(|agent| agent.agent_role.clone()),
        )
    }

    fn agent_status(&self, thread_id: ThreadId) -> AgentStatus {
        self.agents_states
            .get(&thread_id)
            .cloned()
            .unwrap_or(AgentStatus::NotFound)
    }
}

impl SubAgentActivityItem {
    pub(crate) fn as_legacy_event(&self, occurred_at_ms: i64) -> EventMsg {
        EventMsg::SubAgentActivity(SubAgentActivityEvent {
            event_id: self.id.clone(),
            occurred_at_ms,
            agent_thread_id: self.agent_thread_id,
            agent_path: self.agent_path.clone(),
            kind: self.kind,
        })
    }
}

impl WebSearchItem {
    pub fn as_legacy_event(&self) -> EventMsg {
        EventMsg::WebSearchEnd(WebSearchEndEvent {
            call_id: self.id.clone(),
            query: self.query.clone(),
            action: self.action.clone(),
            results: self.results.clone(),
        })
    }
}

impl ImageGenerationItem {
    pub fn as_legacy_event(&self) -> EventMsg {
        EventMsg::ImageGenerationEnd(ImageGenerationEndEvent {
            call_id: self.id.clone(),
            status: self.status.clone(),
            revised_prompt: self.revised_prompt.clone(),
            result: self.result.clone(),
            saved_path: self.saved_path.clone(),
        })
    }
}

impl FileChangeItem {
    pub fn as_legacy_begin_event(&self, turn_id: String) -> EventMsg {
        EventMsg::PatchApplyBegin(PatchApplyBeginEvent {
            call_id: self.id.clone(),
            turn_id,
            auto_approved: self.auto_approved.unwrap_or(false),
            changes: self.changes.clone(),
        })
    }

    pub fn as_legacy_end_event(&self, turn_id: String) -> Option<EventMsg> {
        let status = self.status.clone()?;
        Some(EventMsg::PatchApplyEnd(PatchApplyEndEvent {
            call_id: self.id.clone(),
            turn_id,
            stdout: self.stdout.clone().unwrap_or_default(),
            stderr: self.stderr.clone().unwrap_or_default(),
            success: status == PatchApplyStatus::Completed,
            changes: self.changes.clone(),
            status,
        }))
    }
}

impl McpToolCallItem {
    pub fn as_legacy_begin_event(&self) -> EventMsg {
        EventMsg::McpToolCallBegin(McpToolCallBeginEvent {
            call_id: self.id.clone(),
            invocation: McpInvocation {
                server: self.server.clone(),
                tool: self.tool.clone(),
                arguments: (!self.arguments.is_null()).then(|| self.arguments.clone()),
            },
            connector_id: self.connector_id.clone(),
            mcp_app_resource_uri: self.mcp_app_resource_uri.clone(),
            link_id: self.link_id.clone(),
            app_name: self.app_name.clone(),
            template_id: self.template_id.clone(),
            action_name: self.action_name.clone(),
            plugin_id: self.plugin_id.clone(),
        })
    }

    pub fn as_legacy_end_event(&self) -> Option<EventMsg> {
        let result = match (&self.result, &self.error) {
            (Some(result), _) => Ok(result.clone()),
            (None, Some(error)) => Err(error.message.clone()),
            (None, None) => return None,
        };

        Some(EventMsg::McpToolCallEnd(McpToolCallEndEvent {
            call_id: self.id.clone(),
            invocation: McpInvocation {
                server: self.server.clone(),
                tool: self.tool.clone(),
                arguments: (!self.arguments.is_null()).then(|| self.arguments.clone()),
            },
            mcp_app_resource_uri: self.mcp_app_resource_uri.clone(),
            connector_id: self.connector_id.clone(),
            link_id: self.link_id.clone(),
            app_name: self.app_name.clone(),
            template_id: self.template_id.clone(),
            action_name: self.action_name.clone(),
            plugin_id: self.plugin_id.clone(),
            duration: self.duration?,
            result,
        }))
    }
}

impl TurnItem {
    pub fn as_legacy_events(&self, show_raw_agent_reasoning: bool) -> Vec<EventMsg> {
        match self {
            TurnItem::UserMessage(item) => vec![item.as_legacy_event()],
            TurnItem::HookPrompt(_) => Vec::new(),
            TurnItem::AgentMessage(item) => item.as_legacy_events(),
            TurnItem::Plan(_) => Vec::new(),
            TurnItem::CommandExecution(_)
            | TurnItem::DynamicToolCall(_)
            | TurnItem::CollabAgentToolCall(_) => Vec::new(),
            TurnItem::SubAgentActivity(_) => Vec::new(),
            TurnItem::WebSearch(item) => vec![item.as_legacy_event()],
            TurnItem::ImageView(item) => {
                vec![EventMsg::ViewImageToolCall(ViewImageToolCallEvent {
                    call_id: item.id.clone(),
                    path: item.path.clone(),
                })]
            }
            TurnItem::Extension(_) => Vec::new(),
            TurnItem::ImageGeneration(item) => vec![item.as_legacy_event()],
            TurnItem::EnteredReviewMode(_) | TurnItem::ExitedReviewMode(_) => Vec::new(),
            TurnItem::FileChange(item) => item
                .as_legacy_end_event(String::new())
                .into_iter()
                .collect(),
            TurnItem::McpToolCall(item) => item.as_legacy_end_event().into_iter().collect(),
            TurnItem::Reasoning(item) => item.as_legacy_events(show_raw_agent_reasoning),
            TurnItem::ContextCompaction(item) => vec![item.as_legacy_event()],
        }
    }
}

impl HasLegacyEvent for ItemStartedEvent {
    fn as_legacy_events(&self, _: bool) -> Vec<EventMsg> {
        match &self.item {
            TurnItem::WebSearch(item) => vec![EventMsg::WebSearchBegin(WebSearchBeginEvent {
                call_id: item.id.clone(),
            })],
            TurnItem::ImageView(_) => Vec::new(),
            TurnItem::ImageGeneration(item) => {
                vec![EventMsg::ImageGenerationBegin(ImageGenerationBeginEvent {
                    call_id: item.id.clone(),
                })]
            }
            TurnItem::FileChange(item) => vec![item.as_legacy_begin_event(self.turn_id.clone())],
            TurnItem::McpToolCall(item) => vec![item.as_legacy_begin_event()],
            TurnItem::CommandExecution(item) => {
                vec![item.as_legacy_begin_event(self.turn_id.clone(), self.started_at_ms)]
            }
            TurnItem::DynamicToolCall(item) => {
                vec![item.as_legacy_request_event(self.turn_id.clone(), self.started_at_ms)]
            }
            TurnItem::CollabAgentToolCall(item) => item
                .as_legacy_begin_event(self.started_at_ms)
                .into_iter()
                .collect(),
            _ => Vec::new(),
        }
    }
}

impl HasLegacyEvent for ItemCompletedEvent {
    fn as_legacy_events(&self, show_raw_agent_reasoning: bool) -> Vec<EventMsg> {
        match &self.item {
            TurnItem::FileChange(item) => item
                .as_legacy_end_event(self.turn_id.clone())
                .into_iter()
                .collect(),
            TurnItem::CommandExecution(item) => item
                .as_legacy_end_event(self.turn_id.clone(), self.completed_at_ms)
                .into_iter()
                .collect(),
            TurnItem::DynamicToolCall(item) => item
                .as_legacy_response_event(self.turn_id.clone(), self.completed_at_ms)
                .into_iter()
                .collect(),
            TurnItem::CollabAgentToolCall(item) => item
                .as_legacy_end_event(self.completed_at_ms)
                .into_iter()
                .collect(),
            TurnItem::SubAgentActivity(item) => {
                vec![item.as_legacy_event(self.completed_at_ms)]
            }
            TurnItem::EnteredReviewMode(item) => {
                vec![item.as_legacy_event(self.turn_id.clone())]
            }
            TurnItem::ExitedReviewMode(item) => {
                vec![item.as_legacy_event(self.turn_id.clone())]
            }
            _ => self.item.as_legacy_events(show_raw_agent_reasoning),
        }
    }
}

impl HasLegacyEvent for AgentMessageContentDeltaEvent {
    fn as_legacy_events(&self, _: bool) -> Vec<EventMsg> {
        Vec::new()
    }
}

impl HasLegacyEvent for ReasoningContentDeltaEvent {
    fn as_legacy_events(&self, _: bool) -> Vec<EventMsg> {
        Vec::new()
    }
}

impl HasLegacyEvent for ReasoningRawContentDeltaEvent {
    fn as_legacy_events(&self, _: bool) -> Vec<EventMsg> {
        Vec::new()
    }
}

impl HasLegacyEvent for EventMsg {
    fn as_legacy_events(&self, show_raw_agent_reasoning: bool) -> Vec<EventMsg> {
        match self {
            EventMsg::ItemStarted(event) => event.as_legacy_events(show_raw_agent_reasoning),
            EventMsg::ItemCompleted(event) => event.as_legacy_events(show_raw_agent_reasoning),
            EventMsg::AgentMessageContentDelta(event) => {
                event.as_legacy_events(show_raw_agent_reasoning)
            }
            EventMsg::ReasoningContentDelta(event) => {
                event.as_legacy_events(show_raw_agent_reasoning)
            }
            EventMsg::ReasoningRawContentDelta(event) => {
                event.as_legacy_events(show_raw_agent_reasoning)
            }
            _ => Vec::new(),
        }
    }
}
