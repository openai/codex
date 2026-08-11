//! Replays normalized legacy rollout records into canonical paginated JSONL.
//!
//! `line_parser` makes old JSON shapes parseable, `legacy_event` converts obsolete completion
//! events into modern turn items. Rollback planning happens before this writer sees a record, so
//! this module only assigns stable ordinals, keeps `SessionMeta` at ordinal zero, and emits the
//! already-selected surviving history.
//!
//! The goal is to preserve the model-visible conversation, not to preserve every legacy record
//! byte-for-byte. Filesystem publishing and SQLite projection intentionally live outside this
//! module.

use chrono::DateTime;
use codex_protocol::ThreadId;
use codex_protocol::items::ReasoningItem;
use codex_protocol::items::TurnItem;
use codex_protocol::items::parse_hook_prompt_message;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ItemCompletedEvent;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::protocol::TurnCompleteEvent;
use codex_protocol::protocol::TurnStartedEvent;
use codex_rollout::RolloutItem;
use codex_rollout::RolloutLine;
use std::collections::HashSet;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::io::BufWriter;

use super::legacy_event;
use super::migration_error;
use crate::ThreadStoreResult;

#[derive(Clone)]
struct ActiveTurn {
    id: String,
    explicit: bool,
    saw_user: bool,
}

enum ReasoningTextKind {
    Summary,
    Raw,
}

pub(super) struct LegacyRolloutCanonicalizer {
    thread_id: ThreadId,
    next_ordinal: u64,
    next_item_index: u64,
    output_byte_offset: u64,
    bytes_written: u64,
    source_line_index: u64,
    active_turn: Option<ActiveTurn>,
    known_turn_ids: HashSet<String>,
    reasoning: Option<ReasoningItem>,
}

impl LegacyRolloutCanonicalizer {
    pub(super) fn new(thread_id: ThreadId) -> Self {
        Self {
            thread_id,
            next_ordinal: 0,
            next_item_index: 1,
            output_byte_offset: 0,
            bytes_written: 0,
            source_line_index: 0,
            active_turn: None,
            known_turn_ids: HashSet::new(),
            reasoning: None,
        }
    }

    pub(super) fn next_ordinal(&self) -> u64 {
        self.next_ordinal
    }

    pub(super) fn output_byte_offset(&self) -> u64 {
        self.output_byte_offset
    }

    pub(super) async fn write_head_session_meta(
        &mut self,
        line: RolloutLine,
        writer: &mut BufWriter<File>,
    ) -> ThreadStoreResult<u64> {
        let timestamp = line.timestamp;
        let RolloutItem::SessionMeta(mut metadata) = line.item else {
            return Err(migration_error("canonical session metadata is missing"));
        };
        if metadata.meta.id != self.thread_id {
            return Err(migration_error("rollout metadata thread id changed"));
        }
        metadata.meta.history_mode = ThreadHistoryMode::Paginated;
        metadata.meta.history_base = None;
        metadata.meta.subagent_history_start_ordinal = None;

        let bytes_before = self.bytes_written;
        self.write_item(writer, &timestamp, RolloutItem::SessionMeta(metadata))
            .await?;
        Ok(self.bytes_written - bytes_before)
    }

    pub(super) async fn process_line(
        &mut self,
        line: RolloutLine,
        writer: &mut BufWriter<File>,
    ) -> ThreadStoreResult<u64> {
        let source_index = self.source_line_index;
        self.source_line_index = self
            .source_line_index
            .checked_add(1)
            .ok_or_else(|| migration_error("legacy rollout line index overflow"))?;
        let timestamp = line.timestamp;
        let bytes_before = self.bytes_written;
        match line.item {
            RolloutItem::SessionMeta(_) => return Ok(0),
            RolloutItem::ResponseItem(response) => {
                if matches!(&response.item, ResponseItem::Other) {
                    return Err(migration_error(
                        "legacy rollout contains an unsupported response item",
                    ));
                }
                let hook = match &response.item {
                    ResponseItem::Message {
                        role, content, id, ..
                    } if role == "user" => parse_hook_prompt_message(id.as_deref(), content),
                    _ => None,
                };
                self.write_item(writer, &timestamp, RolloutItem::ResponseItem(response))
                    .await?;
                if let Some(hook) = hook {
                    self.ensure_turn(writer, &timestamp, source_index).await?;
                    self.reasoning = None;
                    self.write_completed_item(writer, &timestamp, TurnItem::HookPrompt(hook))
                        .await?;
                }
            }
            RolloutItem::EventMsg(EventMsg::ThreadRolledBack(_)) => {
                return Err(migration_error(
                    "rollback marker reached canonical writer without a rollback plan",
                ));
            }
            RolloutItem::EventMsg(EventMsg::TurnStarted(event)) => {
                self.finish_implicit_turn(writer, &timestamp).await?;
                self.known_turn_ids.insert(event.turn_id.clone());
                self.active_turn = Some(ActiveTurn {
                    id: event.turn_id.clone(),
                    explicit: true,
                    saw_user: false,
                });
                self.reasoning = None;
                self.write_item(
                    writer,
                    &timestamp,
                    RolloutItem::EventMsg(EventMsg::TurnStarted(event)),
                )
                .await?;
            }
            RolloutItem::EventMsg(EventMsg::TurnComplete(event)) => {
                self.reasoning = None;
                if self
                    .active_turn
                    .as_ref()
                    .is_some_and(|turn| turn.id == event.turn_id)
                {
                    self.active_turn = None;
                }
                self.write_item(
                    writer,
                    &timestamp,
                    RolloutItem::EventMsg(EventMsg::TurnComplete(event)),
                )
                .await?;
            }
            RolloutItem::EventMsg(EventMsg::TurnAborted(mut event)) => {
                if event.turn_id.is_none() {
                    event.turn_id = self.active_turn.as_ref().map(|turn| turn.id.clone());
                }
                if self
                    .active_turn
                    .as_ref()
                    .is_some_and(|turn| event.turn_id.as_deref() == Some(turn.id.as_str()))
                {
                    self.active_turn = None;
                }
                self.reasoning = None;
                self.write_item(
                    writer,
                    &timestamp,
                    RolloutItem::EventMsg(EventMsg::TurnAborted(event)),
                )
                .await?;
            }
            RolloutItem::EventMsg(EventMsg::UserMessage(event)) => {
                if self
                    .active_turn
                    .as_ref()
                    .is_some_and(|turn| !turn.explicit && turn.saw_user)
                {
                    self.finish_implicit_turn(writer, &timestamp).await?;
                }
                self.ensure_turn(writer, &timestamp, source_index).await?;
                let item = legacy_event::user_message_item(event, &mut || self.next_item_id())?;
                if let Some(turn) = self.active_turn.as_mut() {
                    turn.saw_user = true;
                }
                self.reasoning = None;
                self.write_completed_item(writer, &timestamp, item).await?;
            }
            RolloutItem::EventMsg(EventMsg::AgentReasoning(event)) => {
                self.write_reasoning(
                    writer,
                    &timestamp,
                    source_index,
                    event.text,
                    ReasoningTextKind::Summary,
                )
                .await?;
            }
            RolloutItem::EventMsg(EventMsg::AgentReasoningRawContent(event)) => {
                self.write_reasoning(
                    writer,
                    &timestamp,
                    source_index,
                    event.text,
                    ReasoningTextKind::Raw,
                )
                .await?;
            }
            RolloutItem::EventMsg(EventMsg::ItemCompleted(mut event)) => {
                event.thread_id = self.thread_id;
                self.reasoning = None;
                self.write_item(
                    writer,
                    &timestamp,
                    RolloutItem::EventMsg(EventMsg::ItemCompleted(event)),
                )
                .await?;
            }
            RolloutItem::EventMsg(event) => {
                if let Some((item, turn_id)) =
                    legacy_event::completed_item(&event, &mut || self.next_item_id())?
                {
                    match turn_id {
                        Some(turn_id)
                            if self
                                .active_turn
                                .as_ref()
                                .is_some_and(|turn| turn.id.as_str() != turn_id.as_str()) =>
                        {
                            self.write_completed_item_to_turn(writer, &timestamp, turn_id, item)
                                .await?;
                        }
                        Some(turn_id) => {
                            if self.active_turn.is_none()
                                && self.known_turn_ids.contains(turn_id.as_str())
                            {
                                self.reasoning = None;
                                self.write_completed_item_to_turn(
                                    writer, &timestamp, turn_id, item,
                                )
                                .await?;
                            } else if self.active_turn.is_none() {
                                self.start_implicit_turn(writer, &timestamp, turn_id)
                                    .await?;
                                self.reasoning = None;
                                self.write_completed_item(writer, &timestamp, item).await?;
                            } else {
                                self.reasoning = None;
                                self.write_completed_item(writer, &timestamp, item).await?;
                            }
                        }
                        None => {
                            self.ensure_turn(writer, &timestamp, source_index).await?;
                            self.reasoning = None;
                            self.write_completed_item(writer, &timestamp, item).await?;
                        }
                    }
                } else {
                    let item = RolloutItem::EventMsg(event);
                    if codex_rollout::is_persisted_rollout_item(&item, ThreadHistoryMode::Paginated)
                    {
                        self.write_item(writer, &timestamp, item).await?;
                    }
                }
            }
            item @ RolloutItem::InterAgentCommunication(_) => {
                self.write_item(writer, &timestamp, item).await?;
            }
            item @ RolloutItem::Compacted(_) => {
                self.write_item(writer, &timestamp, item).await?;
            }
            item @ (RolloutItem::InterAgentCommunicationMetadata { .. }
            | RolloutItem::TurnContext(_)
            | RolloutItem::WorldState(_)) => {
                self.write_item(writer, &timestamp, item).await?;
            }
        }

        Ok(self.bytes_written - bytes_before)
    }

    pub(super) async fn finish(
        &mut self,
        writer: &mut BufWriter<File>,
        timestamp: &str,
    ) -> ThreadStoreResult<u64> {
        let bytes_before = self.bytes_written;
        self.finish_implicit_turn(writer, timestamp).await?;
        Ok(self.bytes_written - bytes_before)
    }

    async fn ensure_turn(
        &mut self,
        writer: &mut BufWriter<File>,
        timestamp: &str,
        source_index: u64,
    ) -> ThreadStoreResult<()> {
        if self.active_turn.is_some() {
            return Ok(());
        }
        let turn_id = format!("rollout-{source_index}");
        self.start_implicit_turn(writer, timestamp, turn_id).await
    }

    async fn start_implicit_turn(
        &mut self,
        writer: &mut BufWriter<File>,
        timestamp: &str,
        turn_id: String,
    ) -> ThreadStoreResult<()> {
        self.active_turn = Some(ActiveTurn {
            id: turn_id.clone(),
            explicit: false,
            saw_user: false,
        });
        self.known_turn_ids.insert(turn_id.clone());
        self.reasoning = None;
        let started_at = DateTime::parse_from_rfc3339(timestamp)
            .map_err(migration_error)?
            .timestamp();
        self.write_item(
            writer,
            timestamp,
            RolloutItem::EventMsg(EventMsg::TurnStarted(TurnStartedEvent {
                turn_id,
                trace_id: None,
                started_at: Some(started_at),
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            })),
        )
        .await
    }

    async fn finish_implicit_turn(
        &mut self,
        writer: &mut BufWriter<File>,
        timestamp: &str,
    ) -> ThreadStoreResult<()> {
        let Some(turn) = self.active_turn.as_ref() else {
            return Ok(());
        };
        if turn.explicit {
            return Ok(());
        }
        let turn_id = turn.id.clone();
        self.active_turn = None;
        self.reasoning = None;
        self.write_item(
            writer,
            timestamp,
            RolloutItem::EventMsg(EventMsg::TurnComplete(TurnCompleteEvent {
                turn_id,
                last_agent_message: None,
                error: None,
                started_at: None,
                completed_at: Some(
                    DateTime::parse_from_rfc3339(timestamp)
                        .map_err(migration_error)?
                        .timestamp(),
                ),
                duration_ms: None,
                time_to_first_token_ms: None,
            })),
        )
        .await
    }

    async fn write_completed_item(
        &mut self,
        writer: &mut BufWriter<File>,
        timestamp: &str,
        item: TurnItem,
    ) -> ThreadStoreResult<()> {
        let turn_id = self
            .active_turn
            .as_ref()
            .map(|turn| turn.id.clone())
            .ok_or_else(|| migration_error("completed rollout item has no active turn"))?;
        self.write_completed_item_to_turn(writer, timestamp, turn_id, item)
            .await
    }

    async fn write_completed_item_to_turn(
        &mut self,
        writer: &mut BufWriter<File>,
        timestamp: &str,
        turn_id: String,
        item: TurnItem,
    ) -> ThreadStoreResult<()> {
        let completed_at_ms = DateTime::parse_from_rfc3339(timestamp)
            .map_err(migration_error)?
            .timestamp_millis();
        self.write_item(
            writer,
            timestamp,
            RolloutItem::EventMsg(EventMsg::ItemCompleted(ItemCompletedEvent {
                thread_id: self.thread_id,
                turn_id,
                item,
                started_at_ms: None,
                completed_at_ms,
            })),
        )
        .await
    }

    async fn write_reasoning(
        &mut self,
        writer: &mut BufWriter<File>,
        timestamp: &str,
        source_index: u64,
        text: String,
        kind: ReasoningTextKind,
    ) -> ThreadStoreResult<()> {
        if text.is_empty() {
            return Ok(());
        }
        self.ensure_turn(writer, timestamp, source_index).await?;
        let mut item = match self.reasoning.take() {
            Some(item) => item,
            None => ReasoningItem {
                id: self.next_item_id()?,
                summary_text: Vec::new(),
                raw_content: Vec::new(),
            },
        };
        match kind {
            ReasoningTextKind::Summary => item.summary_text.push(text),
            ReasoningTextKind::Raw => item.raw_content.push(text),
        }
        self.reasoning = Some(item.clone());
        self.write_completed_item(writer, timestamp, TurnItem::Reasoning(item))
            .await
    }

    async fn write_item(
        &mut self,
        writer: &mut BufWriter<File>,
        timestamp: &str,
        item: RolloutItem,
    ) -> ThreadStoreResult<()> {
        let mut bytes = serde_json::to_vec(&RolloutLine {
            timestamp: timestamp.to_string(),
            ordinal: Some(self.next_ordinal),
            item,
        })
        .map_err(migration_error)?;
        bytes.push(b'\n');
        writer.write_all(&bytes).await.map_err(migration_error)?;
        let byte_count = u64::try_from(bytes.len())
            .map_err(|_| migration_error("rollout record exceeds addressable size"))?;
        self.output_byte_offset = self
            .output_byte_offset
            .checked_add(byte_count)
            .ok_or_else(|| migration_error("paginated rollout byte offset overflow"))?;
        self.bytes_written = self
            .bytes_written
            .checked_add(byte_count)
            .ok_or_else(|| migration_error("paginated rollout byte count overflow"))?;
        self.next_ordinal = self
            .next_ordinal
            .checked_add(1)
            .ok_or_else(|| migration_error("paginated rollout ordinal overflow"))?;
        Ok(())
    }

    fn next_item_id(&mut self) -> ThreadStoreResult<String> {
        let item_id = format!("item-{}", self.next_item_index);
        self.next_item_index = self
            .next_item_index
            .checked_add(1)
            .ok_or_else(|| migration_error("legacy rollout item id overflow"))?;
        Ok(item_id)
    }
}
