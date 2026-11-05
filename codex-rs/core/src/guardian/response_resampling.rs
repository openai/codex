//! Buffers provisional Guardian output and resamples a terminal denial once.
//! Completions reach the shared accounting path before output replay or retry.

use std::collections::VecDeque;
use std::future::Future;
use std::sync::Arc;

use codex_async_utils::OrCancelExt;
use codex_guardian_reviewer::DenialResample;
use codex_guardian_reviewer::GuardianReviewResponse;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::models::ResponseItem;
use futures::StreamExt;
use tokio_util::sync::CancellationToken;

use crate::client_common::ResponseEvent;
use crate::client_common::ResponseStream;
use crate::session::session::Session;
use crate::session::turn::get_last_assistant_message_from_turn;
use crate::tools::ToolRouter;

#[derive(Default)]
pub(crate) struct ResponseResampling {
    policy: Option<Arc<DenialResample>>,
    buffered_output: Option<Vec<ResponseItem>>,
    pub(crate) accepted_output: VecDeque<ResponseItem>,
    pub(crate) retry_pending: bool,
    pub(crate) update_usage_boundary: bool,
}

impl ResponseResampling {
    pub(crate) fn new(session: &Session) -> Self {
        let policy = session
            .services
            .thread_extension_data
            .get::<DenialResample>();
        Self {
            buffered_output: policy.as_ref().map(|_| Vec::new()),
            policy,
            ..Self::default()
        }
    }

    pub(crate) async fn next(
        &mut self,
        stream: &mut ResponseStream,
    ) -> Option<CodexResult<ResponseEvent>> {
        match self.accepted_output.pop_front() {
            Some(item) => Some(Ok(ResponseEvent::OutputItemDone(item))),
            None => stream.next().await,
        }
    }

    pub(crate) fn filter(
        &mut self,
        event: ResponseEvent,
        router: &ToolRouter,
    ) -> Option<ResponseEvent> {
        let (Some(policy), Some(output)) = (&self.policy, self.buffered_output.as_mut()) else {
            return Some(event);
        };
        match event {
            ResponseEvent::OutputItemDone(item) => {
                let dispatch = !matches!(router.build_tool_call(item.clone()), Ok(None));
                output.push(item);
                if dispatch {
                    // Tool calls proceed immediately, with their preceding reasoning.
                    self.accepted_output.extend(std::mem::take(output));
                    self.buffered_output = None;
                }
                return None;
            }
            ResponseEvent::OutputItemAdded(_)
            | ResponseEvent::OutputTextDelta(_)
            | ResponseEvent::ToolCallInputDelta { .. }
            | ResponseEvent::ReasoningSummaryDelta { .. }
            | ResponseEvent::ReasoningSummaryDone { .. }
            | ResponseEvent::ReasoningContentDelta { .. }
            | ResponseEvent::ReasoningSummaryPartAdded { .. } => return None,
            ResponseEvent::Completed {
                end_turn,
                ref token_usage,
                ..
            } => {
                let output = std::mem::take(output);
                self.buffered_output = None;
                if end_turn != Some(false)
                    && policy.should_resample(GuardianReviewResponse::new(
                        get_last_assistant_message_from_turn(output.iter()).unwrap_or_default(),
                    ))
                {
                    self.retry_pending = true;
                } else {
                    self.accepted_output.extend(output);
                }
                // Account for this completion before replaying any buffered output.
                self.update_usage_boundary =
                    token_usage.is_some() && !self.accepted_output.is_empty();
            }
            ResponseEvent::Created { .. }
            | ResponseEvent::SafetyBuffering(_)
            | ResponseEvent::ServerModel(_)
            | ResponseEvent::ModelVerifications(_)
            | ResponseEvent::TurnModerationMetadata(_)
            | ResponseEvent::GuardianAssessment(_)
            | ResponseEvent::ServerReasoningIncluded(_)
            | ResponseEvent::RateLimits(_)
            | ResponseEvent::ModelsEtag(_) => {}
        }
        Some(event)
    }

    /// Called after the denied completion has been accounted for, with the unchanged request.
    pub(crate) async fn retry(
        &mut self,
        request: impl Future<Output = CodexResult<ResponseStream>> + Send,
        cancellation_token: &CancellationToken,
    ) -> CodexResult<ResponseStream> {
        self.retry_pending = false;
        self.buffered_output = Some(Vec::new());
        request.or_cancel(cancellation_token).await?
    }
}
