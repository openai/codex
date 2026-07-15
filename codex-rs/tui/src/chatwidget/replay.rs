//! Thread replay rendering for `ChatWidget`.
//!
//! This module rehydrates turns and items into transcript state while avoiding
//! live-only side effects.

use super::*;

impl ChatWidget {
    /// Replay a subset of initial events into the UI to seed the transcript when
    /// resuming an existing session. This approximates the live event flow and
    /// is intentionally conservative: only safe-to-replay items are rendered to
    /// avoid triggering side effects. Event ids are passed as `None` to
    /// distinguish replayed events from live ones.
    pub(crate) fn replay_thread_turns(&mut self, turns: Vec<Turn>, replay_kind: ReplayKind) {
        let hidden_nested_review_turns = std::iter::once(/*value*/ false)
            .chain(turns.windows(/*size*/ 2).map(|turns| {
                crate::app_backtrack::is_hidden_nested_review_turn(&turns[0], &turns[1])
            }))
            .collect::<Vec<_>>();
        for (turn, hidden_nested_review_turn) in turns.into_iter().zip(hidden_nested_review_turns) {
            let Turn {
                id: turn_id,
                items_view: _,
                items,
                status,
                error,
                started_at,
                completed_at,
                duration_ms,
            } = turn;
            if matches!(status, TurnStatus::InProgress) {
                self.turn_lifecycle.last_turn_id = Some(turn_id.clone());
                self.last_non_retry_error = None;
                self.on_task_started();
            }
            for item in items {
                if hidden_nested_review_turn && matches!(item, ThreadItem::UserMessage { .. }) {
                    continue;
                }
                self.replay_thread_item(item, turn_id.clone(), replay_kind);
            }
            let status = if hidden_nested_review_turn {
                TurnStatus::Completed
            } else {
                status
            };
            if matches!(
                status,
                TurnStatus::Completed | TurnStatus::Interrupted | TurnStatus::Failed
            ) {
                self.handle_turn_completed_notification(
                    TurnCompletedNotification {
                        thread_id: self.thread_id.map(|id| id.to_string()).unwrap_or_default(),
                        turn: Turn {
                            id: turn_id,
                            items_view: codex_app_server_protocol::TurnItemsView::NotLoaded,
                            items: Vec::new(),
                            status,
                            error,
                            started_at,
                            completed_at,
                            duration_ms,
                        },
                    },
                    Some(replay_kind),
                );
            }
        }
    }

    pub(crate) fn replay_thread_item(
        &mut self,
        item: ThreadItem,
        turn_id: String,
        replay_kind: ReplayKind,
    ) {
        self.handle_thread_item(item, turn_id, ThreadItemRenderSource::Replay(replay_kind));
    }

    pub(super) fn handle_thread_item(
        &mut self,
        item: ThreadItem,
        turn_id: String,
        render_source: ThreadItemRenderSource,
    ) {
        let from_replay = render_source.is_replay();
        let replay_kind = render_source.replay_kind();
        match item {
            ThreadItem::UserMessage { content, .. } => {
                self.on_committed_user_message(&content, from_replay);
            }
            ThreadItem::AgentMessage {
                id,
                text,
                phase,
                memory_citation,
            } => {
                self.on_agent_message_item_completed(
                    AgentMessageItem {
                        id,
                        content: vec![AgentMessageContent::Text { text }],
                        phase,
                        memory_citation: memory_citation.map(|citation| {
                            codex_protocol::memory_citation::MemoryCitation {
                                entries: citation
                                    .entries
                                    .into_iter()
                                    .map(|entry| {
                                        codex_protocol::memory_citation::MemoryCitationEntry {
                                            path: entry.path,
                                            line_start: entry.line_start,
                                            line_end: entry.line_end,
                                            note: entry.note,
                                        }
                                    })
                                    .collect(),
                                rollout_ids: citation.thread_ids,
                            }
                        }),
                    },
                    from_replay,
                );
            }
            ThreadItem::Plan { text, .. } => self.on_plan_item_completed(text),
            ThreadItem::Reasoning {
                summary, content, ..
            } => {
                if from_replay {
                    let reasoning_parts = summary.into_iter().chain(
                        self.config
                            .show_raw_agent_reasoning
                            .then_some(content)
                            .into_iter()
                            .flatten(),
                    );
                    for (index, delta) in reasoning_parts.enumerate() {
                        if index > 0 {
                            self.on_reasoning_section_break();
                        }
                        self.on_agent_reasoning_delta(delta);
                    }
                }
                self.on_agent_reasoning_final();
            }
            item @ ThreadItem::CommandExecution {
                status: codex_app_server_protocol::CommandExecutionStatus::InProgress,
                ..
            } => self.on_command_execution_started(item),
            item @ ThreadItem::CommandExecution { .. } => self.on_command_execution_completed(item),
            ThreadItem::FileChange {
                status: codex_app_server_protocol::PatchApplyStatus::InProgress,
                ..
            } => {}
            item @ ThreadItem::FileChange { .. } => self.on_file_change_completed(item),
            item @ ThreadItem::McpToolCall {
                status: codex_app_server_protocol::McpToolCallStatus::InProgress,
                ..
            } => self.on_mcp_tool_call_started(item),
            item @ ThreadItem::McpToolCall { .. } => self.on_mcp_tool_call_completed(item),
            ThreadItem::WebSearch(item) => {
                self.on_web_search_begin(item.id.clone());
                self.on_web_search_end(
                    item.id,
                    item.query,
                    item.action
                        .unwrap_or(codex_app_server_protocol::WebSearchAction::Other),
                );
            }
            ThreadItem::ImageView { id: _, path } => {
                self.on_view_image_tool_call(path);
            }
            ThreadItem::ImageGeneration(item) => {
                self.on_image_generation_end(
                    item.id,
                    item.status,
                    item.revised_prompt,
                    item.saved_path,
                );
            }
            ThreadItem::EnteredReviewMode { review, .. } => {
                if from_replay {
                    self.enter_review_mode_with_hint(review, /*from_replay*/ true);
                }
            }
            ThreadItem::ExitedReviewMode { .. } => {
                self.exit_review_mode_after_item();
            }
            ThreadItem::ContextCompaction { .. } => {
                self.add_info_message("Context compacted".to_string(), /*hint*/ None);
            }
            ThreadItem::HookPrompt { .. } => {}
            ThreadItem::CollabAgentToolCall {
                id,
                tool,
                status,
                sender_thread_id,
                receiver_thread_ids,
                prompt,
                model,
                reasoning_effort,
                agents_states,
            } => self.on_collab_agent_tool_call(ThreadItem::CollabAgentToolCall {
                id,
                tool,
                status,
                sender_thread_id,
                receiver_thread_ids,
                prompt,
                model,
                reasoning_effort,
                agents_states,
            }),
            item @ ThreadItem::SubAgentActivity { .. } => self.on_sub_agent_activity(item),
            ThreadItem::DynamicToolCall { .. } => {}
            ThreadItem::Sleep(_) => {}
        }

        if matches!(replay_kind, Some(ReplayKind::ThreadSnapshot)) && turn_id.is_empty() {
            self.request_redraw();
        }
    }
}
