//! Reducer support for the remote compaction lifecycle.
//!
//! This module owns request/checkpoint bookkeeping. Conversation item reconciliation stays in
//! `conversation` because it depends on the same normalization and reuse invariants as inference
//! requests.

use anyhow::Result;
use anyhow::bail;

use super::TraceReducer;
use crate::model::Compaction;
use crate::model::CompactionRequest;
use crate::model::CompactionRequestId;
use crate::model::ExecutionStatus;
use crate::model::ExecutionWindow;
use crate::payload::RawPayloadRef;
use crate::raw_event::RawEventSeq;

impl TraceReducer {
    /// Starts one upstream request attempt for a compaction operation.
    pub(super) fn start_compaction_request(
        &mut self,
        seq: RawEventSeq,
        wall_time_unix_ms: i64,
        started: StartedCompactionRequest,
    ) -> Result<()> {
        if self
            .rollout
            .compaction_requests
            .contains_key(&started.compaction_request_id)
        {
            bail!(
                "duplicate compaction request start for {}",
                started.compaction_request_id
            );
        }
        self.thread_mut(&started.thread_id)?;
        let Some(turn) = self.rollout.codex_turns.get(&started.codex_turn_id) else {
            bail!(
                "compaction request {} referenced unknown codex turn {}",
                started.compaction_request_id,
                started.codex_turn_id
            );
        };
        if turn.thread_id != started.thread_id {
            bail!(
                "compaction request {} used thread {}, but codex turn {} belongs to {}",
                started.compaction_request_id,
                started.thread_id,
                started.codex_turn_id,
                turn.thread_id
            );
        }

        self.rollout.compaction_requests.insert(
            started.compaction_request_id.clone(),
            CompactionRequest {
                compaction_request_id: started.compaction_request_id,
                compaction_id: started.compaction_id,
                thread_id: started.thread_id,
                codex_turn_id: started.codex_turn_id,
                execution: ExecutionWindow {
                    started_at_unix_ms: wall_time_unix_ms,
                    started_seq: seq,
                    ended_at_unix_ms: None,
                    ended_seq: None,
                    status: ExecutionStatus::Running,
                },
                model: started.model,
                provider_name: started.provider_name,
                raw_request_payload_id: started.request_payload.raw_payload_id,
                raw_response_payload_id: None,
            },
        );
        Ok(())
    }

    /// Completes an upstream compaction request attempt without modifying conversation history.
    ///
    /// The request/response payloads are evidence for the remote call. The live
    /// conversation changes only when a separate install event provides the checkpoint.
    pub(super) fn complete_compaction_request(
        &mut self,
        seq: RawEventSeq,
        wall_time_unix_ms: i64,
        compaction_id: String,
        compaction_request_id: CompactionRequestId,
        status: ExecutionStatus,
        response_payload: Option<RawPayloadRef>,
    ) -> Result<()> {
        let Some(request) = self
            .rollout
            .compaction_requests
            .get_mut(&compaction_request_id)
        else {
            bail!(
                "compaction request completion referenced unknown request {compaction_request_id}"
            );
        };
        if request.compaction_id != compaction_id {
            bail!(
                "compaction request {compaction_request_id} completion used compaction {compaction_id}, but start used {}",
                request.compaction_id
            );
        }
        request.execution.ended_at_unix_ms = Some(wall_time_unix_ms);
        request.execution.ended_seq = Some(seq);
        request.execution.status = status;
        request.raw_response_payload_id = response_payload.map(|payload| payload.raw_payload_id);
        Ok(())
    }

    /// Installs a compaction checkpoint into the reduced conversation graph.
    ///
    /// This is the semantic boundary where replacement history becomes the live
    /// thread history; request attempts alone do not imply that change.
    pub(super) fn reduce_compaction_installed_event(
        &mut self,
        wall_time_unix_ms: i64,
        thread_id: String,
        codex_turn_id: String,
        compaction_id: String,
        checkpoint_payload: RawPayloadRef,
    ) -> Result<()> {
        if self.rollout.compactions.contains_key(&compaction_id) {
            bail!("duplicate compaction install for {compaction_id}");
        }
        self.thread_mut(&thread_id)?;
        let Some(turn) = self.rollout.codex_turns.get(&codex_turn_id) else {
            bail!(
                "compaction install {compaction_id} referenced unknown codex turn {codex_turn_id}"
            );
        };
        if turn.thread_id != thread_id {
            bail!(
                "compaction install {compaction_id} used thread {thread_id}, but codex turn {codex_turn_id} belongs to {}",
                turn.thread_id
            );
        }
        let checkpoint = self.reduce_compaction_checkpoint(
            wall_time_unix_ms,
            &thread_id,
            codex_turn_id.as_str(),
            &compaction_id,
            &checkpoint_payload,
        )?;
        let request_ids = self
            .rollout
            .compaction_requests
            .values()
            .filter(|request| request.compaction_id == compaction_id)
            .map(|request| request.compaction_request_id.clone())
            .collect();

        self.pending_compaction_replacement_item_ids
            .insert(thread_id.clone(), checkpoint.replacement_item_ids.clone());
        self.rollout.compactions.insert(
            compaction_id.clone(),
            Compaction {
                compaction_id,
                thread_id,
                codex_turn_id,
                installed_at_unix_ms: wall_time_unix_ms,
                marker_item_id: checkpoint.marker_item_id,
                request_ids,
                input_item_ids: checkpoint.input_item_ids,
                replacement_item_ids: checkpoint.replacement_item_ids,
            },
        );
        Ok(())
    }
}

/// Raw compaction-request start fields after dispatch has stripped the event envelope.
pub(super) struct StartedCompactionRequest {
    pub(super) compaction_id: String,
    pub(super) compaction_request_id: String,
    pub(super) thread_id: String,
    pub(super) codex_turn_id: String,
    pub(super) model: String,
    pub(super) provider_name: String,
    pub(super) request_payload: RawPayloadRef,
}
