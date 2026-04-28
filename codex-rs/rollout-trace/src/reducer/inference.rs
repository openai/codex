//! Inference call lifecycle reduction.
//!
//! Conversation request/response normalization lives in the conversation module;
//! this module owns the runtime envelope around those model-facing payloads.

use anyhow::Result;
use anyhow::bail;

use super::TraceReducer;
use crate::model::ExecutionStatus;
use crate::model::ExecutionWindow;
use crate::model::InferenceCall;
use crate::model::InferenceCallId;
use crate::payload::RawPayloadRef;
use crate::raw_event::RawEventSeq;
use crate::raw_event::RawTraceEventPayload;

/// Raw inference-start fields after dispatch has stripped the common event envelope.
///
/// Keeping this as one argument prevents callsites from passing a long list of
/// adjacent strings whose ordering is easy to mix up.
pub(super) struct StartedInferenceCall {
    pub(super) inference_call_id: InferenceCallId,
    pub(super) thread_id: String,
    pub(super) codex_turn_id: String,
    pub(super) model: String,
    pub(super) provider_name: String,
    pub(super) request_payload: RawPayloadRef,
}

impl TraceReducer {
    /// Starts an inference call and reduces its request payload into conversation items.
    ///
    /// Requests are model-visible transcript evidence, so the inference object is only
    /// inserted after the request snapshot has been normalized and linked to the turn.
    pub(super) fn start_inference_call(
        &mut self,
        seq: RawEventSeq,
        wall_time_unix_ms: i64,
        started: StartedInferenceCall,
    ) -> Result<()> {
        if self
            .rollout
            .inference_calls
            .contains_key(&started.inference_call_id)
        {
            bail!(
                "duplicate inference start for {}",
                started.inference_call_id
            );
        }

        let inference_call_id = started.inference_call_id.clone();
        let thread_id = started.thread_id.clone();
        let codex_turn_id = started.codex_turn_id.clone();
        let request_payload = started.request_payload.clone();
        let Some(turn) = self.rollout.codex_turns.get(&codex_turn_id) else {
            bail!(
                "inference start {inference_call_id} referenced unknown codex turn {codex_turn_id}"
            );
        };
        if turn.thread_id != thread_id {
            bail!(
                "inference start {inference_call_id} used thread {thread_id}, \
                 but codex turn {codex_turn_id} belongs to {}",
                turn.thread_id
            );
        }

        let request_item_ids = self.reduce_inference_request(
            wall_time_unix_ms,
            &inference_call_id,
            &thread_id,
            &codex_turn_id,
            &request_payload,
        )?;

        self.thread_mut(&thread_id)?;

        self.rollout.inference_calls.insert(
            inference_call_id.clone(),
            InferenceCall {
                inference_call_id,
                thread_id,
                codex_turn_id,
                execution: ExecutionWindow {
                    started_at_unix_ms: wall_time_unix_ms,
                    started_seq: seq,
                    ended_at_unix_ms: None,
                    ended_seq: None,
                    status: ExecutionStatus::Running,
                },
                model: started.model,
                provider_name: started.provider_name,
                response_id: None,
                upstream_request_id: None,
                request_item_ids,
                response_item_ids: Vec::new(),
                tool_call_ids_started_by_response: Vec::new(),
                usage: None,
                raw_request_payload_id: started.request_payload.raw_payload_id,
                raw_response_payload_id: None,
            },
        );
        Ok(())
    }

    /// Closes any inference streams that are still live when the owning turn ends.
    ///
    /// Normal completion events close the active inference before the turn ends.
    /// If a call is still `Running`, Codex stopped observing that provider stream
    /// earlier and the reduced graph should not present it as live.
    pub(super) fn close_running_inference_calls_for_turn_end(
        &mut self,
        seq: RawEventSeq,
        wall_time_unix_ms: i64,
        codex_turn_id: &str,
        turn_status: &ExecutionStatus,
    ) {
        let inference_status = match turn_status {
            ExecutionStatus::Running => return,
            ExecutionStatus::Completed | ExecutionStatus::Cancelled => ExecutionStatus::Cancelled,
            ExecutionStatus::Failed => ExecutionStatus::Failed,
            ExecutionStatus::Aborted => ExecutionStatus::Aborted,
        };
        for inference in self.rollout.inference_calls.values_mut() {
            if inference.codex_turn_id == codex_turn_id
                && inference.execution.status == ExecutionStatus::Running
            {
                inference.execution.ended_at_unix_ms = Some(wall_time_unix_ms);
                inference.execution.ended_seq = Some(seq);
                inference.execution.status = inference_status.clone();
            }
        }
    }

    /// Completes an inference call and, when present, reduces response output items.
    pub(super) fn complete_inference_call(
        &mut self,
        seq: RawEventSeq,
        wall_time_unix_ms: i64,
        payload: RawTraceEventPayload,
    ) -> Result<()> {
        let (inference_call_id, status, response_id, upstream_request_id, response_payload) =
            match payload {
                RawTraceEventPayload::InferenceCompleted {
                    inference_call_id,
                    response_id,
                    upstream_request_id,
                    response_payload,
                } => (
                    inference_call_id,
                    ExecutionStatus::Completed,
                    response_id,
                    upstream_request_id,
                    Some(response_payload),
                ),
                RawTraceEventPayload::InferenceFailed {
                    inference_call_id,
                    upstream_request_id,
                    partial_response_payload,
                    ..
                } => (
                    inference_call_id,
                    ExecutionStatus::Failed,
                    None,
                    upstream_request_id,
                    partial_response_payload,
                ),
                RawTraceEventPayload::InferenceCancelled {
                    inference_call_id,
                    upstream_request_id,
                    partial_response_payload,
                    ..
                } => (
                    inference_call_id,
                    ExecutionStatus::Cancelled,
                    None,
                    upstream_request_id,
                    partial_response_payload,
                ),
                _ => bail!("complete_inference_call received a non-terminal inference event"),
            };

        if !self
            .rollout
            .inference_calls
            .contains_key(&inference_call_id)
        {
            bail!("inference completion referenced unknown call {inference_call_id}");
        }

        let response_item_ids = response_payload
            .as_ref()
            .map(|payload| {
                self.reduce_inference_response(wall_time_unix_ms, &inference_call_id, payload)
            })
            .transpose()?;
        {
            let Some(inference) = self.rollout.inference_calls.get_mut(&inference_call_id) else {
                bail!("inference call {inference_call_id} disappeared during response reduction");
            };
            inference.response_id = response_id;
            // Turn-end cleanup can close a stream before the async mapper observes
            // cancellation. Preserve that terminal status while still retaining any
            // late partial response evidence from the mapper.
            if inference.execution.status == ExecutionStatus::Running {
                inference.execution.ended_at_unix_ms = Some(wall_time_unix_ms);
                inference.execution.ended_seq = Some(seq);
                inference.execution.status = status;
            }
            // Turn-end cleanup can mark an inference terminal before the stream
            // mapper records its late partial payload. Keep the server request
            // id from that late payload even when the status is already closed.
            if let Some(upstream_request_id) = upstream_request_id {
                inference.upstream_request_id = Some(upstream_request_id);
            }
            if let Some(response_payload) = response_payload {
                inference.raw_response_payload_id = Some(response_payload.raw_payload_id);
            }
            if let Some(response_item_ids) = response_item_ids {
                inference.response_item_ids = response_item_ids;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "inference_tests.rs"]
mod tests;
