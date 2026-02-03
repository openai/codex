use crate::common::ResponseEvent;
use crate::common::ResponseStream;
use crate::error::ApiError;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ResponseItem;
use futures::Stream;
use std::collections::VecDeque;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

/// Stream adapter that merges token deltas into a single assistant message per turn.
pub struct AggregatedStream {
    inner: ResponseStream,
    cumulative: String,
    cumulative_reasoning: String,
    pending: VecDeque<ResponseEvent>,
}

impl Stream for AggregatedStream {
    type Item = Result<ResponseEvent, ApiError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if let Some(ev) = this.pending.pop_front() {
            return Poll::Ready(Some(Ok(ev)));
        }

        loop {
            match Pin::new(&mut this.inner).poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Ready(Some(Err(err))) => return Poll::Ready(Some(Err(err))),
                Poll::Ready(Some(Ok(ResponseEvent::OutputItemDone(item)))) => {
                    let is_assistant_message = matches!(
                        &item,
                        ResponseItem::Message { role, .. } if role == "assistant"
                    );

                    if is_assistant_message {
                        if this.cumulative.is_empty()
                            && let ResponseItem::Message { content, .. } = &item
                            && let Some(text) = content.iter().find_map(|c| match c {
                                ContentItem::OutputText { text } => Some(text),
                                _ => None,
                            })
                        {
                            this.cumulative.push_str(text);
                        }
                        continue;
                    }

                    return Poll::Ready(Some(Ok(ResponseEvent::OutputItemDone(item))));
                }
                Poll::Ready(Some(Ok(ResponseEvent::ServerReasoningIncluded(included)))) => {
                    return Poll::Ready(Some(Ok(ResponseEvent::ServerReasoningIncluded(included))));
                }
                Poll::Ready(Some(Ok(ResponseEvent::RateLimits(snapshot)))) => {
                    return Poll::Ready(Some(Ok(ResponseEvent::RateLimits(snapshot))));
                }
                Poll::Ready(Some(Ok(ResponseEvent::ModelsEtag(etag)))) => {
                    return Poll::Ready(Some(Ok(ResponseEvent::ModelsEtag(etag))));
                }
                Poll::Ready(Some(Ok(ResponseEvent::Completed {
                    response_id,
                    token_usage,
                }))) => {
                    let mut emitted_any = false;

                    if !this.cumulative_reasoning.is_empty() {
                        let aggregated_reasoning = ResponseItem::Reasoning {
                            id: String::new(),
                            summary: Vec::new(),
                            content: Some(vec![ReasoningItemContent::ReasoningText {
                                text: std::mem::take(&mut this.cumulative_reasoning),
                            }]),
                            encrypted_content: None,
                        };
                        this.pending
                            .push_back(ResponseEvent::OutputItemDone(aggregated_reasoning));
                        emitted_any = true;
                    }

                    if !this.cumulative.is_empty() {
                        let aggregated_message = ResponseItem::Message {
                            id: None,
                            role: "assistant".to_string(),
                            content: vec![ContentItem::OutputText {
                                text: std::mem::take(&mut this.cumulative),
                            }],
                            end_turn: None,
                            phase: None,
                        };
                        this.pending
                            .push_back(ResponseEvent::OutputItemDone(aggregated_message));
                        emitted_any = true;
                    }

                    if emitted_any {
                        this.pending.push_back(ResponseEvent::Completed {
                            response_id: response_id.clone(),
                            token_usage: token_usage.clone(),
                        });
                        if let Some(ev) = this.pending.pop_front() {
                            return Poll::Ready(Some(Ok(ev)));
                        }
                    }

                    return Poll::Ready(Some(Ok(ResponseEvent::Completed {
                        response_id,
                        token_usage,
                    })));
                }
                Poll::Ready(Some(Ok(ResponseEvent::Created))) => continue,
                Poll::Ready(Some(Ok(ResponseEvent::OutputTextDelta(delta)))) => {
                    this.cumulative.push_str(&delta);
                    continue;
                }
                Poll::Ready(Some(Ok(ResponseEvent::ReasoningContentDelta {
                    delta,
                    content_index: _,
                }))) => {
                    this.cumulative_reasoning.push_str(&delta);
                    continue;
                }
                Poll::Ready(Some(Ok(ResponseEvent::ReasoningSummaryDelta { .. }))) => continue,
                Poll::Ready(Some(Ok(ResponseEvent::ReasoningSummaryPartAdded { .. }))) => continue,
                Poll::Ready(Some(Ok(ResponseEvent::OutputItemAdded(item)))) => {
                    return Poll::Ready(Some(Ok(ResponseEvent::OutputItemAdded(item))));
                }
            }
        }
    }
}

pub trait AggregateStreamExt {
    fn aggregate(self) -> AggregatedStream;
}

impl AggregateStreamExt for ResponseStream {
    fn aggregate(self) -> AggregatedStream {
        AggregatedStream::new(self)
    }
}

impl AggregatedStream {
    fn new(inner: ResponseStream) -> Self {
        AggregatedStream {
            inner,
            cumulative: String::new(),
            cumulative_reasoning: String::new(),
            pending: VecDeque::new(),
        }
    }
}
