//! Buffers provisional Guardian output and resamples a terminal denial once.
//! Completions reach the shared accounting path before output replay or retry.

use std::collections::VecDeque;
use std::sync::Arc;

use codex_guardian_reviewer::DenialResample;
use codex_guardian_reviewer::GuardianReviewResponse;
use codex_protocol::models::ResponseItem;
use futures::StreamExt;

use crate::client_common::Prompt;
use crate::client_common::ResponseEvent;
use crate::client_common::SamplingEvent;
use crate::client_common::SamplingStream;
use crate::session::session::Session;
use crate::session::turn::get_last_assistant_message_from_turn;
use crate::tools::ToolRouter;

struct BufferedReview {
    policy: Arc<DenialResample>,
    retry_prompt: Option<Arc<Prompt>>,
    output: Vec<ResponseItem>,
}

#[derive(Default)]
pub(crate) struct ResponseResampling {
    review: Option<BufferedReview>,
    pub(crate) accepted_output: VecDeque<ResponseItem>,
    pub(crate) update_usage_boundary: bool,
}

impl ResponseResampling {
    pub(crate) fn new(session: &Session, prompt: &Arc<Prompt>) -> Self {
        let policy = session
            .services
            .thread_extension_data
            .get::<DenialResample>();
        Self {
            review: policy.map(|policy| BufferedReview {
                policy,
                retry_prompt: Some(Arc::clone(prompt)),
                output: Vec::new(),
            }),
            ..Self::default()
        }
    }

    pub(crate) async fn next(&mut self, stream: &mut SamplingStream) -> Option<SamplingEvent> {
        match self.accepted_output.pop_front() {
            Some(item) => Some(Ok(ResponseEvent::OutputItemDone(item)).into()),
            None => stream.next().await,
        }
    }

    pub(crate) fn filter(
        &mut self,
        event: ResponseEvent,
        router: &ToolRouter,
    ) -> Option<ResponseEvent> {
        let Some(review) = self.review.as_mut() else {
            return Some(event);
        };
        match event {
            ResponseEvent::OutputItemDone(item) => {
                let dispatch = !matches!(router.build_tool_call(item.clone()), Ok(None));
                review.output.push(item);
                if dispatch {
                    // Tool calls proceed immediately, with their preceding reasoning.
                    self.accepted_output
                        .extend(std::mem::take(&mut review.output));
                    self.review = None;
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
            ResponseEvent::Created { .. }
            | ResponseEvent::Completed { .. }
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

    /// Accepts buffered output or returns the original prompt to retry after accounting.
    pub(crate) fn finish_response(
        &mut self,
        end_turn: Option<bool>,
        has_usage: bool,
    ) -> Option<Arc<Prompt>> {
        let review = self.review.as_mut()?;
        let output = std::mem::take(&mut review.output);
        if let Some(prompt) = review.retry_prompt.take()
            && end_turn != Some(false)
            && review.policy.should_resample(GuardianReviewResponse::new(
                get_last_assistant_message_from_turn(output.iter()).unwrap_or_default(),
            ))
        {
            // Keep buffering until the second verdict can be replayed after accounting.
            return Some(prompt);
        }
        self.review = None;
        self.update_usage_boundary = has_usage && !output.is_empty();
        self.accepted_output.extend(output);
        None
    }
}
