//! Preserve voice answers and accepted transcripts across thread-widget replacement.
//!
//! A queued speech command can outlive its widget. Retain its bounded final
//! item by original thread until replay either renders that item or needs a
//! fallback, without adding history to whichever thread is currently visible.

use super::*;
use codex_protocol::models::MessagePhase;
use std::collections::HashMap;
use std::collections::HashSet;

type ReplayedVoiceTextCounts = HashMap<(String, String), usize>;
const MAX_INACTIVE_REALTIME_REPLAY_THREADS: usize = 16;

fn normalized_replay_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn record_voice_item(
    item: &ThreadItem,
    turn_id: &str,
    delegated: &mut HashSet<String>,
    items: &mut HashMap<(String, String, &'static str), String>,
) {
    match item {
        ThreadItem::UserMessage { id, content, .. } => {
            if let Some(input) = crate::chatwidget::realtime_delegation_input(content) {
                delegated.insert(turn_id.to_string());
                let display = crate::chatwidget::realtime_delegation_display_text(input);
                items.insert(
                    (turn_id.to_string(), id.clone(), "user"),
                    normalized_replay_text(&display),
                );
            }
        }
        ThreadItem::AgentMessage {
            id,
            text,
            phase: Some(MessagePhase::FinalAnswer) | None,
            ..
        } if delegated.contains(turn_id)
            && !crate::chatwidget::is_private_realtime_agent_item(item)
            && !text.trim().is_empty() =>
        {
            items.insert(
                (turn_id.to_string(), id.clone(), "assistant"),
                normalized_replay_text(text),
            );
        }
        _ => {}
    }
}

pub(super) fn replayed_voice_texts_from_turns(turns: &[Turn]) -> ReplayedVoiceTextCounts {
    let mut delegated = HashSet::new();
    let mut items = HashMap::new();
    for turn in turns {
        for item in &turn.items {
            record_voice_item(item, &turn.id, &mut delegated, &mut items);
        }
    }
    count_replayed_voice_texts(items)
}

fn count_replayed_voice_texts(
    items: HashMap<(String, String, &'static str), String>,
) -> ReplayedVoiceTextCounts {
    let mut counts = HashMap::new();
    for ((_, _, role), text) in items {
        if !text.is_empty() {
            *counts.entry((role.to_string(), text)).or_insert(0) += 1;
        }
    }
    counts
}

pub(super) fn replayed_voice_texts(snapshot: &ThreadEventSnapshot) -> ReplayedVoiceTextCounts {
    let mut delegated = snapshot
        .delegated_turns
        .iter()
        .cloned()
        .collect::<HashSet<_>>();
    let mut items = HashMap::new();
    for turn in &snapshot.turns {
        for item in &turn.items {
            record_voice_item(item, &turn.id, &mut delegated, &mut items);
        }
    }
    for event in &snapshot.events {
        let ThreadBufferedEvent::Notification(notification) = event else {
            continue;
        };
        match notification.as_ref() {
            ServerNotification::ItemStarted(n) => {
                if let ThreadItem::UserMessage { content, .. } = &n.item
                    && crate::chatwidget::realtime_delegation_input(content).is_some()
                {
                    delegated.insert(n.turn_id.clone());
                }
            }
            ServerNotification::ItemCompleted(n) => {
                record_voice_item(&n.item, &n.turn_id, &mut delegated, &mut items);
            }
            ServerNotification::TurnCompleted(n) if n.turn.status == TurnStatus::Completed => {
                if let Some(item) = n.turn.items.iter().rev().find(|item| {
                    matches!(
                        item,
                        ThreadItem::AgentMessage {
                            phase: Some(MessagePhase::FinalAnswer) | None,
                            ..
                        }
                    ) && !crate::chatwidget::is_private_realtime_agent_item(item)
                }) {
                    record_voice_item(item, &n.turn.id, &mut delegated, &mut items);
                }
            }
            _ => {}
        }
    }
    count_replayed_voice_texts(items)
}

impl App {
    pub(super) fn note_realtime_replay_thread(&mut self, thread_id: ThreadId) {
        if self.realtime_replay_order.contains(&thread_id) {
            return;
        }
        if self.realtime_replay_order.len() >= MAX_INACTIVE_REALTIME_REPLAY_THREADS
            && let Some(oldest) = self.realtime_replay_order.pop_front()
        {
            self.pending_realtime_transcript_replay.remove(&oldest);
            self.pending_realtime_speech_replay.remove(&oldest);
        }
        self.realtime_replay_order.push_back(thread_id);
    }

    pub(super) fn forget_realtime_replay_thread(&mut self, thread_id: ThreadId) {
        self.realtime_replay_order
            .retain(|saved| *saved != thread_id);
        self.pending_realtime_transcript_replay.remove(&thread_id);
        self.pending_realtime_speech_replay.remove(&thread_id);
    }

    pub(super) fn retain_inactive_realtime_transcript(
        &mut self,
        thread_id: ThreadId,
        notification: &ServerNotification,
    ) {
        if let ServerNotification::TurnStarted(started) = notification
            && let Some(records) = self.pending_realtime_transcript_replay.get_mut(&thread_id)
        {
            for record in records {
                if record.complete && record.before_turn_id.is_none() {
                    record.before_turn_id = Some(started.turn.id.clone());
                }
            }
        }
        let (role, text, complete) = match notification {
            ServerNotification::ThreadRealtimeTranscriptDelta(n) => {
                (n.role.as_str(), n.delta.as_str(), false)
            }
            ServerNotification::ThreadRealtimeTranscriptDone(n) => {
                (n.role.as_str(), n.text.as_str(), true)
            }
            _ => return,
        };
        if !matches!(role, "user" | "assistant") {
            return;
        }
        self.note_realtime_replay_thread(thread_id);
        let records = self
            .pending_realtime_transcript_replay
            .entry(thread_id)
            .or_default();
        if complete {
            if text.trim().is_empty() {
                if let Some(index) = records
                    .iter()
                    .rposition(|record| record.role == role && !record.complete)
                {
                    records.remove(index);
                }
                return;
            }
            let mut bounded = text.to_string();
            if bounded.len() > crate::chatwidget::MAX_TRANSCRIPT_BYTES {
                let mut end = crate::chatwidget::MAX_TRANSCRIPT_BYTES;
                while !bounded.is_char_boundary(end) {
                    end -= 1;
                }
                bounded.truncate(end);
            }
            if let Some(partial) = records
                .iter_mut()
                .rev()
                .find(|record| record.role == role && !record.complete)
            {
                partial.text = bounded;
                partial.complete = true;
                return;
            }
            if records.len() >= crate::chatwidget::MAX_REPLAY_TRANSCRIPT_CELLS {
                records.pop_front();
            }
            records.push_back(crate::chatwidget::RealtimeTranscriptRecord {
                role: role.to_string(),
                text: bounded,
                complete: true,
                before_turn_id: None,
            });
        } else {
            if text.is_empty() {
                return;
            }
            let partial = records
                .iter_mut()
                .rev()
                .find(|record| record.role == role && !record.complete);
            if let Some(partial) = partial {
                partial.text.push_str(text);
                if partial.text.len() > crate::chatwidget::MAX_TRANSCRIPT_BYTES {
                    let mut start = partial.text.len() - crate::chatwidget::MAX_TRANSCRIPT_BYTES;
                    while !partial.text.is_char_boundary(start) {
                        start += 1;
                    }
                    partial.text.drain(..start);
                }
            } else {
                let mut bounded = text.to_string();
                if bounded.len() > crate::chatwidget::MAX_TRANSCRIPT_BYTES {
                    let mut start = bounded.len() - crate::chatwidget::MAX_TRANSCRIPT_BYTES;
                    while !bounded.is_char_boundary(start) {
                        start += 1;
                    }
                    bounded.drain(..start);
                }
                if records.len() >= crate::chatwidget::MAX_REPLAY_TRANSCRIPT_CELLS {
                    records.pop_front();
                }
                records.push_back(crate::chatwidget::RealtimeTranscriptRecord {
                    role: role.to_string(),
                    text: bounded,
                    complete: false,
                    before_turn_id: None,
                });
            }
        }
    }

    pub(super) fn retain_realtime_replay_state_before_replace(&mut self) {
        if let Some(thread_id) = self.chat_widget.thread_id() {
            if self.chat_widget.may_receive_realtime_transcripts() {
                self.note_realtime_replay_thread(thread_id);
                // Even a session without captions can receive a final queued caption
                // after this widget has been replaced.
                self.pending_realtime_transcript_replay
                    .entry(thread_id)
                    .or_default();
            }
            let cells = self.chat_widget.take_realtime_transcript_cells_for_replay();
            if !cells.is_empty() {
                self.note_realtime_replay_thread(thread_id);
                let pending = self
                    .pending_realtime_transcript_replay
                    .entry(thread_id)
                    .or_default();
                // Only the active widget produces these cells; reattachment consumes them
                // before another widget for this thread can become active.
                // Up to 32 accepted cells plus one partial for each speaker.
                debug_assert!(pending.len() + cells.len() <= 34);
                pending.extend(cells);
            }
        }
        for (thread_id, turn_id, item) in self
            .chat_widget
            .take_undelivered_realtime_speech_for_replay()
        {
            self.note_realtime_replay_thread(thread_id);
            let pending = self
                .pending_realtime_speech_replay
                .entry(thread_id)
                .or_default();
            // The active widget is the only producer for a thread; selecting
            // that thread reconciles these records before it can produce more.
            debug_assert!(pending.len() < 16);
            pending.push((turn_id, item));
        }
    }

    pub(super) fn prepare_realtime_transcript_replay(
        &mut self,
        mut replayed_voice_texts: ReplayedVoiceTextCounts,
    ) -> HashMap<String, usize> {
        let Some(thread_id) = self.chat_widget.thread_id() else {
            return HashMap::new();
        };
        self.realtime_replay_order
            .retain(|saved| *saved != thread_id);
        let mut retained_assistant_captions = HashMap::<String, usize>::new();
        if let Some(mut cells) = self.pending_realtime_transcript_replay.remove(&thread_id) {
            cells.retain(|record| {
                if !record.complete {
                    return true;
                }
                let key = (record.role.clone(), normalized_replay_text(&record.text));
                if let Some(count) = replayed_voice_texts.get_mut(&key)
                    && *count > 0
                {
                    *count -= 1;
                    return false;
                }
                true
            });
            for record in &cells {
                if record.complete && record.role == "assistant" {
                    *retained_assistant_captions
                        .entry(normalized_replay_text(&record.text))
                        .or_default() += 1;
                }
            }
            self.chat_widget
                .queue_realtime_transcripts_for_replay(cells);
        }
        retained_assistant_captions
    }

    pub(super) fn restore_realtime_replay_state_after_replay(
        &mut self,
        replayed_final_items: &HashMap<(String, String), String>,
        mut retained_assistant_captions: HashMap<String, usize>,
    ) {
        let Some(thread_id) = self.chat_widget.thread_id() else {
            return;
        };
        self.chat_widget.finish_realtime_transcript_replay();
        let Some(pending) = self.pending_realtime_speech_replay.remove(&thread_id) else {
            return;
        };
        for (turn_id, item) in pending {
            let ThreadItem::AgentMessage { id, text, .. } = &item else {
                continue;
            };
            // A matching ID with empty text did not render the saved answer.
            // Any nonempty snapshot text is authoritative even if it changed.
            if replayed_final_items
                .get(&(turn_id.clone(), id.clone()))
                .is_none_or(|text| text.trim().is_empty())
            {
                // A completed caption restored above already displays this
                // answer. Consume only one exact match; partial or distinct
                // captions must not hide an undelivered fallback.
                let spoken_text = text.trim().strip_prefix("[FINAL]").unwrap_or(text);
                if let Some(count) =
                    retained_assistant_captions.get_mut(&normalized_replay_text(spoken_text))
                    && *count > 0
                {
                    *count -= 1;
                    continue;
                }
                self.chat_widget
                    .render_undelivered_realtime_speech(thread_id, turn_id, item);
            }
        }
    }
}

pub(super) fn completed_agent_items(
    snapshot: &ThreadEventSnapshot,
) -> HashMap<(String, String), String> {
    let mut completed = completed_agent_items_from_turns(&snapshot.turns);
    for event in &snapshot.events {
        let ThreadBufferedEvent::Notification(notification) = event else {
            continue;
        };
        match notification.as_ref() {
            ServerNotification::ItemCompleted(completed_item) => {
                if let ThreadItem::AgentMessage { id, text, .. } = &completed_item.item
                    && !text.trim().is_empty()
                {
                    completed.insert((completed_item.turn_id.clone(), id.clone()), text.clone());
                }
            }
            ServerNotification::TurnCompleted(completed_turn)
                if completed_turn.turn.status == TurnStatus::Completed =>
            {
                for item in &completed_turn.turn.items {
                    if let ThreadItem::AgentMessage { id, text, .. } = item
                        && !text.trim().is_empty()
                    {
                        completed
                            .insert((completed_turn.turn.id.clone(), id.clone()), text.clone());
                    }
                }
            }
            _ => {}
        }
    }
    completed
}

pub(super) fn completed_agent_items_from_turns(
    turns: &[Turn],
) -> HashMap<(String, String), String> {
    let mut completed = HashMap::new();
    for turn in turns {
        for item in &turn.items {
            if let ThreadItem::AgentMessage { id, text, .. } = item
                && !text.trim().is_empty()
            {
                completed.insert((turn.id.clone(), id.clone()), text.clone());
            }
        }
    }
    completed
}
