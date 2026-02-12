use crate::protocol::v2::ThreadItem;
use crate::protocol::v2::Turn;
use crate::protocol::v2::TurnError;
use crate::protocol::v2::TurnStatus;
use crate::protocol::v2::UserInput;
use codex_protocol::protocol::AgentReasoningEvent;
use codex_protocol::protocol::AgentReasoningRawContentEvent;
use codex_protocol::protocol::CompactedItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ItemCompletedEvent;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::ThreadRolledBackEvent;
use codex_protocol::protocol::TurnAbortedEvent;
use codex_protocol::protocol::TurnCompleteEvent;
use codex_protocol::protocol::TurnStartedEvent;
use codex_protocol::protocol::UserMessageEvent;
use uuid::Uuid;

/// Convert persisted [`RolloutItem`] entries into a sequence of [`Turn`] values.
///
/// When available, this uses `TurnContext.turn_id` as the canonical turn id so
/// resumed/rebuilt thread history preserves the original turn identifiers.
pub fn build_turns_from_rollout_items(items: &[RolloutItem]) -> Vec<Turn> {
    let mut builder = ThreadHistoryBuilder::new();
    for item in items {
        builder.handle_rollout_item(item);
    }
    builder.finish()
}

struct ThreadHistoryBuilder {
    turns: Vec<Turn>,
    current_turn: Option<PendingTurn>,
    next_item_index: i64,
}

impl ThreadHistoryBuilder {
    fn new() -> Self {
        Self {
            turns: Vec::new(),
            current_turn: None,
            next_item_index: 1,
        }
    }

    fn finish(mut self) -> Vec<Turn> {
        self.finish_current_turn();
        self.turns
    }

    /// This function should handle all EventMsg variants that can be persisted in a rollout file.
    /// See `should_persist_event_msg` in `codex-rs/core/rollout/policy.rs`.
    fn handle_event(&mut self, event: &EventMsg) {
        match event {
            EventMsg::UserMessage(payload) => self.handle_user_message(payload),
            EventMsg::AgentMessage(payload) => self.handle_agent_message(payload.message.clone()),
            EventMsg::AgentReasoning(payload) => self.handle_agent_reasoning(payload),
            EventMsg::AgentReasoningRawContent(payload) => {
                self.handle_agent_reasoning_raw_content(payload)
            }
            EventMsg::ItemCompleted(payload) => self.handle_item_completed(payload),
            EventMsg::TokenCount(_) => {}
            EventMsg::EnteredReviewMode(_) => {}
            EventMsg::ExitedReviewMode(_) => {}
            EventMsg::ThreadRolledBack(payload) => self.handle_thread_rollback(payload),
            EventMsg::UndoCompleted(_) => {}
            EventMsg::TurnAborted(payload) => self.handle_turn_aborted(payload),
            EventMsg::TurnStarted(payload) => self.handle_turn_started(payload),
            EventMsg::TurnComplete(payload) => self.handle_turn_complete(payload),
            _ => {}
        }
    }

    fn handle_rollout_item(&mut self, item: &RolloutItem) {
        match item {
            RolloutItem::EventMsg(event) => self.handle_event(event),
            RolloutItem::Compacted(payload) => self.handle_compacted(payload),
            RolloutItem::TurnContext(_)
            | RolloutItem::SessionMeta(_)
            | RolloutItem::ResponseItem(_) => {}
        }
    }

    fn handle_user_message(&mut self, payload: &UserMessageEvent) {
        // User messages should stay in explicitly opened turns. For backward
        // compatibility with older streams that did not open turns explicitly,
        // close any implicit/inactive turn and start a fresh one for this input.
        if let Some(turn) = self.current_turn.as_ref()
            && !turn.opened_explicitly
            && !(turn.saw_compaction && turn.items.is_empty())
        {
            self.finish_current_turn();
        }
        let mut turn = self
            .current_turn
            .take()
            .unwrap_or_else(|| self.new_turn(None));
        let id = self.next_item_id();
        let content = self.build_user_inputs(payload);
        turn.items.push(ThreadItem::UserMessage { id, content });
        self.current_turn = Some(turn);
    }

    fn handle_agent_message(&mut self, text: String) {
        if text.is_empty() {
            return;
        }

        let id = self.next_item_id();
        self.ensure_turn()
            .items
            .push(ThreadItem::AgentMessage { id, text });
    }

    fn handle_agent_reasoning(&mut self, payload: &AgentReasoningEvent) {
        if payload.text.is_empty() {
            return;
        }

        // If the last item is a reasoning item, add the new text to the summary.
        if let Some(ThreadItem::Reasoning { summary, .. }) = self.ensure_turn().items.last_mut() {
            summary.push(payload.text.clone());
            return;
        }

        // Otherwise, create a new reasoning item.
        let id = self.next_item_id();
        self.ensure_turn().items.push(ThreadItem::Reasoning {
            id,
            summary: vec![payload.text.clone()],
            content: Vec::new(),
        });
    }

    fn handle_agent_reasoning_raw_content(&mut self, payload: &AgentReasoningRawContentEvent) {
        if payload.text.is_empty() {
            return;
        }

        // If the last item is a reasoning item, add the new text to the content.
        if let Some(ThreadItem::Reasoning { content, .. }) = self.ensure_turn().items.last_mut() {
            content.push(payload.text.clone());
            return;
        }

        // Otherwise, create a new reasoning item.
        let id = self.next_item_id();
        self.ensure_turn().items.push(ThreadItem::Reasoning {
            id,
            summary: Vec::new(),
            content: vec![payload.text.clone()],
        });
    }

    fn handle_item_completed(&mut self, payload: &ItemCompletedEvent) {
        if let codex_protocol::items::TurnItem::Plan(plan) = &payload.item {
            if plan.text.is_empty() {
                return;
            }
            let id = self.next_item_id();
            self.ensure_turn().items.push(ThreadItem::Plan {
                id,
                text: plan.text.clone(),
            });
        }
    }

    fn handle_turn_aborted(&mut self, _payload: &TurnAbortedEvent) {
        let Some(turn) = self.current_turn.as_mut() else {
            return;
        };
        turn.status = TurnStatus::Interrupted;
    }

    fn handle_turn_started(&mut self, payload: &TurnStartedEvent) {
        self.finish_current_turn();
        self.current_turn = Some(
            self.new_turn(Some(payload.turn_id.clone()))
                .opened_explicitly(),
        );
    }

    fn handle_turn_complete(&mut self, _payload: &TurnCompleteEvent) {
        if let Some(current_turn) = self.current_turn.as_mut() {
            current_turn.status = TurnStatus::Completed;
            self.finish_current_turn();
        }
    }

    /// Marks the current turn as containing a persisted compaction marker.
    ///
    /// This keeps compaction-only legacy turns from being dropped by
    /// `finish_current_turn` when they have no renderable items and were not
    /// explicitly opened.
    fn handle_compacted(&mut self, _payload: &CompactedItem) {
        self.ensure_turn().saw_compaction = true;
    }

    fn handle_thread_rollback(&mut self, payload: &ThreadRolledBackEvent) {
        self.finish_current_turn();

        let n = usize::try_from(payload.num_turns).unwrap_or(usize::MAX);
        if n >= self.turns.len() {
            self.turns.clear();
        } else {
            self.turns.truncate(self.turns.len().saturating_sub(n));
        }

        let item_count: usize = self.turns.iter().map(|t| t.items.len()).sum();
        self.next_item_index = i64::try_from(item_count.saturating_add(1)).unwrap_or(i64::MAX);
    }

    fn finish_current_turn(&mut self) {
        if let Some(turn) = self.current_turn.take() {
            if turn.items.is_empty() && !turn.opened_explicitly && !turn.saw_compaction {
                return;
            }
            self.turns.push(turn.into());
        }
    }

    fn new_turn(&mut self, id: Option<String>) -> PendingTurn {
        PendingTurn {
            id: id.unwrap_or_else(|| Uuid::now_v7().to_string()),
            items: Vec::new(),
            error: None,
            status: TurnStatus::Completed,
            opened_explicitly: false,
            saw_compaction: false,
        }
    }

    fn ensure_turn(&mut self) -> &mut PendingTurn {
        if self.current_turn.is_none() {
            let turn = self.new_turn(None);
            return self.current_turn.insert(turn);
        }

        if let Some(turn) = self.current_turn.as_mut() {
            return turn;
        }

        unreachable!("current turn must exist after initialization");
    }

    fn next_item_id(&mut self) -> String {
        let id = format!("item-{}", self.next_item_index);
        self.next_item_index += 1;
        id
    }

    fn build_user_inputs(&self, payload: &UserMessageEvent) -> Vec<UserInput> {
        let mut content = Vec::new();
        if !payload.message.trim().is_empty() {
            content.push(UserInput::Text {
                text: payload.message.clone(),
                text_elements: payload
                    .text_elements
                    .iter()
                    .cloned()
                    .map(Into::into)
                    .collect(),
            });
        }
        if let Some(images) = &payload.images {
            for image in images {
                content.push(UserInput::Image { url: image.clone() });
            }
        }
        for path in &payload.local_images {
            content.push(UserInput::LocalImage { path: path.clone() });
        }
        content
    }
}

struct PendingTurn {
    id: String,
    items: Vec<ThreadItem>,
    error: Option<TurnError>,
    status: TurnStatus,
    /// True when this turn originated from an explicit `turn_started`/`turn_complete`
    /// boundary, so we preserve it even if it has no renderable items.
    opened_explicitly: bool,
    /// True when this turn includes a persisted `RolloutItem::Compacted`, which
    /// should keep the turn from being dropped even without normal items.
    saw_compaction: bool,
}

impl PendingTurn {
    fn opened_explicitly(mut self) -> Self {
        self.opened_explicitly = true;
        self
    }
}

impl From<PendingTurn> for Turn {
    fn from(value: PendingTurn) -> Self {
        Self {
            id: value.id,
            items: value.items,
            error: value.error,
            status: value.status,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::protocol::AgentMessageEvent;
    use codex_protocol::protocol::AgentReasoningEvent;
    use codex_protocol::protocol::AgentReasoningRawContentEvent;
    use codex_protocol::protocol::CompactedItem;
    use codex_protocol::protocol::ThreadRolledBackEvent;
    use codex_protocol::protocol::TurnAbortReason;
    use codex_protocol::protocol::TurnAbortedEvent;
    use codex_protocol::protocol::TurnCompleteEvent;
    use codex_protocol::protocol::TurnStartedEvent;
    use codex_protocol::protocol::UserMessageEvent;
    use pretty_assertions::assert_eq;
    use uuid::Uuid;

    #[test]
    fn builds_multiple_turns_with_reasoning_items() {
        let events = vec![
            EventMsg::UserMessage(UserMessageEvent {
                message: "First turn".into(),
                images: Some(vec!["https://example.com/one.png".into()]),
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::AgentMessage(AgentMessageEvent {
                message: "Hi there".into(),
            }),
            EventMsg::AgentReasoning(AgentReasoningEvent {
                text: "thinking".into(),
            }),
            EventMsg::AgentReasoningRawContent(AgentReasoningRawContentEvent {
                text: "full reasoning".into(),
            }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "Second turn".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::AgentMessage(AgentMessageEvent {
                message: "Reply two".into(),
            }),
        ];

        let items = events
            .into_iter()
            .map(RolloutItem::EventMsg)
            .collect::<Vec<_>>();
        let turns = build_turns_from_rollout_items(&items);
        assert_eq!(turns.len(), 2);

        let first = &turns[0];
        assert!(Uuid::parse_str(&first.id).is_ok());
        assert_eq!(first.status, TurnStatus::Completed);
        assert_eq!(first.items.len(), 3);
        assert_eq!(
            first.items[0],
            ThreadItem::UserMessage {
                id: "item-1".into(),
                content: vec![
                    UserInput::Text {
                        text: "First turn".into(),
                        text_elements: Vec::new(),
                    },
                    UserInput::Image {
                        url: "https://example.com/one.png".into(),
                    }
                ],
            }
        );
        assert_eq!(
            first.items[1],
            ThreadItem::AgentMessage {
                id: "item-2".into(),
                text: "Hi there".into(),
            }
        );
        assert_eq!(
            first.items[2],
            ThreadItem::Reasoning {
                id: "item-3".into(),
                summary: vec!["thinking".into()],
                content: vec!["full reasoning".into()],
            }
        );

        let second = &turns[1];
        assert!(Uuid::parse_str(&second.id).is_ok());
        assert_ne!(first.id, second.id);
        assert_eq!(second.items.len(), 2);
        assert_eq!(
            second.items[0],
            ThreadItem::UserMessage {
                id: "item-4".into(),
                content: vec![UserInput::Text {
                    text: "Second turn".into(),
                    text_elements: Vec::new(),
                }],
            }
        );
        assert_eq!(
            second.items[1],
            ThreadItem::AgentMessage {
                id: "item-5".into(),
                text: "Reply two".into(),
            }
        );
    }

    #[test]
    fn splits_reasoning_when_interleaved() {
        let events = vec![
            EventMsg::UserMessage(UserMessageEvent {
                message: "Turn start".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::AgentReasoning(AgentReasoningEvent {
                text: "first summary".into(),
            }),
            EventMsg::AgentReasoningRawContent(AgentReasoningRawContentEvent {
                text: "first content".into(),
            }),
            EventMsg::AgentMessage(AgentMessageEvent {
                message: "interlude".into(),
            }),
            EventMsg::AgentReasoning(AgentReasoningEvent {
                text: "second summary".into(),
            }),
        ];

        let items = events
            .into_iter()
            .map(RolloutItem::EventMsg)
            .collect::<Vec<_>>();
        let turns = build_turns_from_rollout_items(&items);
        assert_eq!(turns.len(), 1);
        let turn = &turns[0];
        assert_eq!(turn.items.len(), 4);

        assert_eq!(
            turn.items[1],
            ThreadItem::Reasoning {
                id: "item-2".into(),
                summary: vec!["first summary".into()],
                content: vec!["first content".into()],
            }
        );
        assert_eq!(
            turn.items[3],
            ThreadItem::Reasoning {
                id: "item-4".into(),
                summary: vec!["second summary".into()],
                content: Vec::new(),
            }
        );
    }

    #[test]
    fn marks_turn_as_interrupted_when_aborted() {
        let events = vec![
            EventMsg::UserMessage(UserMessageEvent {
                message: "Please do the thing".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::AgentMessage(AgentMessageEvent {
                message: "Working...".into(),
            }),
            EventMsg::TurnAborted(TurnAbortedEvent {
                turn_id: Some("turn-1".into()),
                reason: TurnAbortReason::Replaced,
            }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "Let's try again".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::AgentMessage(AgentMessageEvent {
                message: "Second attempt complete.".into(),
            }),
        ];

        let items = events
            .into_iter()
            .map(RolloutItem::EventMsg)
            .collect::<Vec<_>>();
        let turns = build_turns_from_rollout_items(&items);
        assert_eq!(turns.len(), 2);

        let first_turn = &turns[0];
        assert_eq!(first_turn.status, TurnStatus::Interrupted);
        assert_eq!(first_turn.items.len(), 2);
        assert_eq!(
            first_turn.items[0],
            ThreadItem::UserMessage {
                id: "item-1".into(),
                content: vec![UserInput::Text {
                    text: "Please do the thing".into(),
                    text_elements: Vec::new(),
                }],
            }
        );
        assert_eq!(
            first_turn.items[1],
            ThreadItem::AgentMessage {
                id: "item-2".into(),
                text: "Working...".into(),
            }
        );

        let second_turn = &turns[1];
        assert_eq!(second_turn.status, TurnStatus::Completed);
        assert_eq!(second_turn.items.len(), 2);
        assert_eq!(
            second_turn.items[0],
            ThreadItem::UserMessage {
                id: "item-3".into(),
                content: vec![UserInput::Text {
                    text: "Let's try again".into(),
                    text_elements: Vec::new(),
                }],
            }
        );
        assert_eq!(
            second_turn.items[1],
            ThreadItem::AgentMessage {
                id: "item-4".into(),
                text: "Second attempt complete.".into(),
            }
        );
    }

    #[test]
    fn drops_last_turns_on_thread_rollback() {
        let events = vec![
            EventMsg::UserMessage(UserMessageEvent {
                message: "First".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::AgentMessage(AgentMessageEvent {
                message: "A1".into(),
            }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "Second".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::AgentMessage(AgentMessageEvent {
                message: "A2".into(),
            }),
            EventMsg::ThreadRolledBack(ThreadRolledBackEvent { num_turns: 1 }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "Third".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::AgentMessage(AgentMessageEvent {
                message: "A3".into(),
            }),
        ];

        let items = events
            .into_iter()
            .map(RolloutItem::EventMsg)
            .collect::<Vec<_>>();
        let turns = build_turns_from_rollout_items(&items);
        assert_eq!(turns.len(), 2);
        assert!(Uuid::parse_str(&turns[0].id).is_ok());
        assert!(Uuid::parse_str(&turns[1].id).is_ok());
        assert_ne!(turns[0].id, turns[1].id);
        assert_eq!(turns[0].status, TurnStatus::Completed);
        assert_eq!(turns[1].status, TurnStatus::Completed);
        assert_eq!(
            turns[0].items,
            vec![
                ThreadItem::UserMessage {
                    id: "item-1".into(),
                    content: vec![UserInput::Text {
                        text: "First".into(),
                        text_elements: Vec::new(),
                    }],
                },
                ThreadItem::AgentMessage {
                    id: "item-2".into(),
                    text: "A1".into(),
                },
            ]
        );
        assert_eq!(
            turns[1].items,
            vec![
                ThreadItem::UserMessage {
                    id: "item-3".into(),
                    content: vec![UserInput::Text {
                        text: "Third".into(),
                        text_elements: Vec::new(),
                    }],
                },
                ThreadItem::AgentMessage {
                    id: "item-4".into(),
                    text: "A3".into(),
                },
            ]
        );
    }

    #[test]
    fn thread_rollback_clears_all_turns_when_num_turns_exceeds_history() {
        let events = vec![
            EventMsg::UserMessage(UserMessageEvent {
                message: "One".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::AgentMessage(AgentMessageEvent {
                message: "A1".into(),
            }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "Two".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::AgentMessage(AgentMessageEvent {
                message: "A2".into(),
            }),
            EventMsg::ThreadRolledBack(ThreadRolledBackEvent { num_turns: 99 }),
        ];

        let items = events
            .into_iter()
            .map(RolloutItem::EventMsg)
            .collect::<Vec<_>>();
        let turns = build_turns_from_rollout_items(&items);
        assert_eq!(turns, Vec::<Turn>::new());
    }

    #[test]
    fn uses_explicit_turn_boundaries_for_mid_turn_steering() {
        let events = vec![
            EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: "turn-a".into(),
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "Start".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "Steer".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::TurnComplete(TurnCompleteEvent {
                turn_id: "turn-a".into(),
                last_agent_message: None,
            }),
        ];

        let items = events
            .into_iter()
            .map(RolloutItem::EventMsg)
            .collect::<Vec<_>>();
        let turns = build_turns_from_rollout_items(&items);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].id, "turn-a");
        assert_eq!(
            turns[0].items,
            vec![
                ThreadItem::UserMessage {
                    id: "item-1".into(),
                    content: vec![UserInput::Text {
                        text: "Start".into(),
                        text_elements: Vec::new(),
                    }],
                },
                ThreadItem::UserMessage {
                    id: "item-2".into(),
                    content: vec![UserInput::Text {
                        text: "Steer".into(),
                        text_elements: Vec::new(),
                    }],
                },
            ]
        );
    }

    #[test]
    fn preserves_compaction_only_turn() {
        let items = vec![
            RolloutItem::EventMsg(EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: "turn-compact".into(),
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            })),
            RolloutItem::Compacted(CompactedItem {
                message: String::new(),
                replacement_history: None,
            }),
            RolloutItem::EventMsg(EventMsg::TurnComplete(TurnCompleteEvent {
                turn_id: "turn-compact".into(),
                last_agent_message: None,
            })),
        ];

        let turns = build_turns_from_rollout_items(&items);
        assert_eq!(
            turns,
            vec![Turn {
                id: "turn-compact".into(),
                status: TurnStatus::Completed,
                error: None,
                items: Vec::new(),
            }]
        );
    }
}
