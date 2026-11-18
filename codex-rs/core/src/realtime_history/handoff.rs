//! Builds handoff context from the same immutable transcript items served in history.
//! Pending text is only consumed after turn admission and the acceptance barrier succeed.

use super::Continuation;
use super::RealtimeHistoryState;
use crate::context::RealtimeDelegation;
use crate::context::RealtimeDelegationSource;
use codex_history::ResponseItemEnvelope;
use codex_protocol::protocol::RealtimeHandoffRequested;
use codex_protocol::protocol::RealtimeTranscriptEntry;
use codex_protocol::realtime::RealtimeItem;
use codex_protocol::realtime::RealtimeItemContent;
use codex_protocol::realtime::RealtimeTranscriptRole;
use codex_protocol::realtime::SessionInputReason;
use uuid::Uuid;

// Older protocols retain a bounded transcript tail; V3 drains whole completed utterances.
const PENDING_TRANSCRIPT_BYTES: usize = 64 * 1024;

/// Persisted lifecycle of a logical call, independent of its current transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecoveredSessionStatus {
    Open,
    Ending,
    Closed,
}

pub(crate) enum HandoffSettlement {
    Settled,
    Wait(std::sync::Arc<tokio::sync::Notify>),
}

// One transport utterance can span several frames when a promotion or typed input
// seals speech midway through it. V3 drains complete utterances in capture order.
pub(super) struct V3Utterance {
    role: RealtimeTranscriptRole,
    pub(super) frames: Vec<String>,
    pub(super) complete: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct PreparedRealtimeInput {
    pub(crate) id: Uuid,
    pub(crate) session_id: String,
    pub(crate) handoff_id: String,
    pub(crate) sources: Vec<String>,
    consumed_event_ids: Vec<String>,
    pub(crate) input: String,
    pub(crate) new_items: Vec<RealtimeItem>,
    pub(crate) reserved: bool,
    reserved_turn_id: Option<String>,
    pub(crate) cached_envelope: Option<ResponseItemEnvelope>,
}

impl RealtimeHistoryState {
    /// Recover only durable V3 speech. A new transport starts fresh utterances; frames from
    /// before reconnect remain frozen in persisted order until the next input consumes them.
    pub(crate) fn recover_session(
        &mut self,
        session_id: &str,
        items: &[RealtimeItem],
    ) -> RecoveredSessionStatus {
        let mut consumed = std::collections::HashSet::new();
        let mut status = RecoveredSessionStatus::Open;
        for item in items {
            if let Some(time) = item.observed_at_ms {
                self.observation_clock.observe(time);
            }
            match &item.content {
                RealtimeItemContent::TurnInputAccepted {
                    handoff_id,
                    source_event_ids,
                    ..
                } => {
                    self.committed_handoff_ids.insert(handoff_id.clone());
                    consumed.extend(source_event_ids.iter().cloned());
                }
                RealtimeItemContent::SessionInputSaved {
                    reason,
                    source_event_ids,
                    ..
                } => {
                    consumed.extend(source_event_ids.iter().cloned());
                    if *reason == SessionInputReason::SessionEnd {
                        status = RecoveredSessionStatus::Ending;
                    }
                }
                RealtimeItemContent::RealtimeSessionClosed { .. } => {
                    status = RecoveredSessionStatus::Closed
                }
                _ => {}
            }
        }
        self.active_session_id =
            (status != RecoveredSessionStatus::Closed).then(|| session_id.to_owned());
        self.preserve_transcript = true;
        self.active_segments = Default::default();
        self.v3_utterances.clear();
        self.pending_handoff_seals.clear();
        self.pending_transcript = items
            .iter()
            .filter(|item| {
                matches!(item.content, RealtimeItemContent::Frame { .. })
                    && !consumed.contains(&item.id)
            })
            .cloned()
            .collect();
        self.recovered_frame_ids = self
            .pending_transcript
            .iter()
            .map(|item| item.id.clone())
            .collect();
        status
    }

    /// Build one chunk's exact frame provenance before recording its canonical model input.
    /// Frozen speech must precede new speech so reconnect cannot reverse model input order.
    pub(crate) fn prepare_recorded_chunk(
        &self,
        entries: &[RealtimeTranscriptEntry],
    ) -> (Vec<RealtimeTranscriptEntry>, Vec<String>) {
        let mut combined = Vec::new();
        let mut sources = Vec::new();
        for item in &self.pending_transcript {
            if self.recovered_frame_ids.contains(&item.id)
                && let RealtimeItemContent::Frame { channels } = &item.content
            {
                let transcript = &channels.voice_transcript;
                combined.push(RealtimeTranscriptEntry {
                    role: match transcript.role {
                        RealtimeTranscriptRole::User => "user",
                        RealtimeTranscriptRole::Assistant => "assistant",
                    }
                    .to_owned(),
                    text: transcript.text.clone(),
                });
                sources.push(item.id.clone());
            }
        }
        sources.extend(
            self.v3_utterances
                .iter()
                .take(entries.len())
                .take_while(|u| u.complete)
                .flat_map(|u| u.frames.iter().cloned()),
        );
        combined.extend_from_slice(entries);
        (combined, sources)
    }

    pub(crate) fn active_session_id(&self) -> Option<&str> {
        self.active_session_id.as_deref()
    }

    pub(crate) fn reserved_turn_ids(&self) -> Vec<String> {
        self.handoff_inputs
            .values()
            .filter(|input| input.reserved)
            .filter_map(|input| input.reserved_turn_id.clone())
            .collect()
    }

    pub(super) fn remember_transcript(&mut self, item: RealtimeItem) {
        self.pending_transcript.push_back(item);
        if self.preserve_transcript {
            return;
        }
        let mut bytes: usize = self.pending_transcript.iter().map(transcript_bytes).sum();
        while bytes > PENDING_TRANSCRIPT_BYTES && self.pending_transcript.len() > 1 {
            if let Some(item) = self.pending_transcript.pop_front() {
                bytes -= transcript_bytes(&item);
            }
        }
    }

    pub(super) fn v3_utterance(
        &mut self,
        role: RealtimeTranscriptRole,
        start: bool,
    ) -> Option<&mut V3Utterance> {
        if !self.frames_enabled() || !self.preserve_transcript {
            return None;
        }
        let utterances = &mut self.v3_utterances;
        let index = match utterances
            .iter()
            .rposition(|utterance| !utterance.complete && utterance.role == role)
        {
            Some(index) => index,
            None if start => {
                utterances.push_back(V3Utterance {
                    role,
                    frames: Vec::new(),
                    complete: false,
                });
                utterances.len() - 1
            }
            None => return None,
        };
        utterances.get_mut(index)
    }

    pub(crate) fn consume_recorded_chunk(&mut self, entries: &[RealtimeTranscriptEntry]) {
        // The socket drains only the complete prefix. Match by utterance position,
        // not text: a late final can repeat speech already sealed at a handoff.
        let count = self
            .v3_utterances
            .iter()
            .take(entries.len())
            .take_while(|u| u.complete)
            .count();
        let mut ids: std::collections::HashSet<_> = self
            .v3_utterances
            .drain(..count)
            .flat_map(|u| u.frames)
            .collect();
        ids.extend(self.recovered_frame_ids.drain());
        self.pending_transcript
            .retain(|item| !ids.contains(&item.id));
    }

    /// Seal active speech and leave frames reserved by an in-flight input out of this one.
    fn seal_available_transcripts(&mut self) -> Vec<(Option<&str>, RealtimeTranscriptRole, &str)> {
        let mut new_items = Vec::new();
        self.seal_segments(&mut new_items, Continuation::Continue);
        self.pending_handoff_seals.extend(new_items);
        if !self.v3_utterances.is_empty() {
            // Speakers can finish out of order, and a promotion can split an utterance.
            // Keep each utterance's frames together in original speech order.
            let rank: std::collections::HashMap<_, _> = self
                .v3_utterances
                .iter()
                .flat_map(|utterance| &utterance.frames)
                .enumerate()
                .map(|(index, id)| (id, index + 1))
                .collect();
            self.pending_transcript
                .make_contiguous()
                .sort_by_key(|item| rank.get(&item.id).copied().unwrap_or(0));
        }
        let reserved = self
            .handoff_inputs
            .values()
            .filter(|input| input.reserved || input.cached_envelope.is_some())
            .flat_map(|input| input.consumed_event_ids.iter())
            .collect::<std::collections::HashSet<_>>();
        self.pending_transcript
            .iter()
            .filter(|item| !reserved.contains(&item.id))
            .filter_map(|item| {
                if let RealtimeItemContent::Frame { channels } = &item.content {
                    let transcript = &channels.voice_transcript;
                    Some((
                        Some(item.id.as_str()),
                        transcript.role,
                        transcript.text.as_str(),
                    ))
                } else {
                    None
                }
            })
            .collect()
    }

    /// A pending frame belongs to only one immutable seal candidate. Rejection leaves it available.
    fn unassigned_handoff_seals(&self) -> Vec<RealtimeItem> {
        let assigned = self
            .handoff_inputs
            .values()
            .flat_map(|input| input.new_items.iter().map(|item| &item.id))
            .collect::<std::collections::HashSet<_>>();
        self.pending_handoff_seals
            .iter()
            .filter(|item| !assigned.contains(&item.id))
            .cloned()
            .collect()
    }

    pub(crate) fn prepare_handoff(
        &mut self,
        handoff: &RealtimeHandoffRequested,
    ) -> Option<PreparedRealtimeInput> {
        if self.committed_handoff_ids.contains(&handoff.handoff_id) {
            return None;
        }
        if let Some(prepared) = self.handoff_inputs.get(&handoff.handoff_id) {
            return Some(prepared.clone());
        }
        let session_id = self.active_session_id.clone()?;
        let preserve_transcript = self.preserve_transcript;
        // The V3 transport appends a tool input row only if it did not already
        // hear that utterance, including in a chunk it previously recorded.
        let has_tool_input_row = handoff.active_transcript.len() > self.v3_utterances.len();
        let mut entries = self.seal_available_transcripts();
        let consumed_event_ids = entries
            .iter()
            .filter_map(|(id, _, _)| id.map(str::to_string))
            .collect();
        let fallback = handoff.input_transcript.trim();
        let add_fallback = if preserve_transcript {
            has_tool_input_row
        } else {
            !entries.iter().any(|(_, role, text)| {
                *role == RealtimeTranscriptRole::User && text.trim() == fallback
            })
        };
        if !fallback.is_empty() && add_fallback {
            // Tool-supplied task text is context, not evidence that the user spoke it.
            // Preserve it in the accepted input without inventing a transcript event.
            entries.push((None, RealtimeTranscriptRole::User, fallback));
        }
        let (input, sources) = RealtimeDelegation::from_events(
            &handoff.input_transcript,
            &entries,
            RealtimeDelegationSource::Handoff,
            preserve_transcript,
        )?;
        let prepared = PreparedRealtimeInput {
            id: Uuid::now_v7(),
            session_id,
            handoff_id: handoff.handoff_id.clone(),
            sources,
            consumed_event_ids,
            input,
            new_items: self.unassigned_handoff_seals(),
            reserved: false,
            reserved_turn_id: None,
            cached_envelope: None,
        };
        self.handoff_inputs
            .insert(handoff.handoff_id.clone(), prepared.clone());
        Some(prepared)
    }

    pub(crate) fn restore_handoffs(&mut self, items: &[codex_history::RolloutItem]) {
        for handoff_id in items
            .iter()
            .filter_map(codex_history::RolloutItem::realtime_handoff_id)
        {
            self.committed_handoff_ids.insert(handoff_id.to_string());
        }
    }

    /// Reserve before queuing the input so overlapping handoffs cannot claim its frames.
    pub(crate) fn reserve_handoff(&mut self, id: Uuid) -> bool {
        let Some(input) = self
            .handoff_inputs
            .values_mut()
            .find(|input| input.id == id)
        else {
            return false;
        };
        if input.reserved {
            return false;
        }
        input.reserved = true;
        true
    }

    pub(crate) fn pending_handoff(&self, id: Uuid) -> Option<PreparedRealtimeInput> {
        self.handoff_inputs
            .values()
            .find(|input| input.id == id)
            .cloned()
    }

    /// Keep the exact annotated input across an uncertain append or flush outcome.
    pub(crate) fn cache_handoff_input(
        &mut self,
        id: Uuid,
        envelope: ResponseItemEnvelope,
    ) -> Option<PreparedRealtimeInput> {
        let input = self
            .handoff_inputs
            .values_mut()
            .find(|input| input.id == id)?;
        input.cached_envelope.get_or_insert(envelope);
        Some(input.clone())
    }

    /// A definite rejection frees its sources; an uncertain seal retains the exact candidate.
    pub(crate) fn reject_handoff(&mut self, id: Uuid) {
        let Some((key, input)) = self
            .handoff_inputs
            .iter_mut()
            .find(|(_, input)| input.id == id)
        else {
            return;
        };
        if input.cached_envelope.is_some() {
            input.reserved = false;
            input.reserved_turn_id = None;
        } else {
            let key = key.clone();
            self.handoff_inputs.remove(&key);
        }
        self.handoff_settled.notify_one();
    }

    pub(crate) fn bind_handoff_to_turn(&mut self, id: Uuid, turn_id: &str) {
        if let Some(input) = self
            .handoff_inputs
            .values_mut()
            .find(|input| input.id == id)
            && input.reserved
        {
            input.reserved_turn_id = Some(turn_id.to_owned());
        }
    }

    pub(crate) fn reject_handoffs_for_turn(&mut self, turn_id: &str) {
        let ids = self
            .handoff_inputs
            .values()
            .filter(|input| input.reserved_turn_id.as_deref() == Some(turn_id))
            .map(|input| input.id)
            .collect::<Vec<_>>();
        for id in ids {
            self.reject_handoff(id);
        }
    }

    /// Subscribe while the state is locked; notify_one retains a permit if settlement races await.
    pub(crate) fn pending_handoff_settlement(&self) -> std::io::Result<HandoffSettlement> {
        if self.handoff_inputs.values().any(|input| input.reserved) {
            return Ok(HandoffSettlement::Wait(std::sync::Arc::clone(
                &self.handoff_settled,
            )));
        }
        if self
            .handoff_inputs
            .values()
            .any(|input| input.cached_envelope.is_some())
        {
            return Err(std::io::Error::other(
                "realtime handoff seal outcome is uncertain",
            ));
        }
        Ok(HandoffSettlement::Settled)
    }

    /// Consume transcript only after the canonical input (or hangup receipt) is durable.
    pub(crate) fn finish_handoff(&mut self, id: Uuid, turn_id: Option<&str>) {
        let Some(handoff_id) = self
            .handoff_inputs
            .iter()
            .find_map(|(key, input)| (input.id == id).then(|| key.clone()))
        else {
            return;
        };
        let Some(prepared) = self.handoff_inputs.remove(&handoff_id) else {
            return;
        };
        self.pending_handoff_seals
            .retain(|item| !prepared.new_items.iter().any(|sealed| sealed.id == item.id));
        self.pending_transcript
            .retain(|item| !prepared.consumed_event_ids.contains(&item.id));
        self.recovered_frame_ids
            .retain(|id| !prepared.consumed_event_ids.contains(id));
        if let Some(turn_id) = turn_id {
            self.realtime_session_by_bem_turn
                .insert(turn_id.to_string(), prepared.session_id);
        }
        self.committed_handoff_ids.insert(handoff_id);
        if prepared.reserved {
            self.handoff_settled.notify_one();
        }
    }
}

fn transcript_bytes(item: &RealtimeItem) -> usize {
    match &item.content {
        RealtimeItemContent::Frame { channels } => channels.voice_transcript.text.len(),
        _ => 0,
    }
}
