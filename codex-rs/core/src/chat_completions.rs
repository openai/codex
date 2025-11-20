use crate::client_common::ResponseEvent;
use crate::error::Result;
use futures::Stream;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

#[derive(Copy, Clone, Eq, PartialEq)]
enum AggregateMode {
    AggregatedOnly,
    Streaming,
}

/// Stream adapter that merges token deltas into a single assistant message per turn.
pub(crate) struct AggregatedChatStream<S> {
    inner: S,
    cumulative: String,
    cumulative_reasoning: String,
    pending: std::collections::VecDeque<ResponseEvent>,
    mode: AggregateMode,
}

impl<S> Stream for AggregatedChatStream<S>
where
    S: Stream<Item = Result<ResponseEvent>> + Unpin,
{
    type Item = Result<ResponseEvent>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if let Some(ev) = this.pending.pop_front() {
            return Poll::Ready(Some(Ok(ev)));
        }

        loop {
            match Pin::new(&mut this.inner).poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Ready(Some(Err(e))) => return Poll::Ready(Some(Err(e))),
                Poll::Ready(Some(Ok(ResponseEvent::OutputItemDone(item)))) => {
                    let is_assistant_message = matches!(
                        &item,
                        codex_protocol::models::ResponseItem::Message { role, .. }
                        if role == "assistant"
                    );

                    if is_assistant_message {
                        match this.mode {
                            AggregateMode::AggregatedOnly => {
                                if this.cumulative.is_empty()
                                    && let codex_protocol::models::ResponseItem::Message {
                                        content,
                                        ..
                                    } = &item
                                    && let Some(text) = content.iter().find_map(|c| match c {
                                        codex_protocol::models::ContentItem::OutputText {
                                            text,
                                        } => Some(text),
                                        _ => None,
                                    })
                                {
                                    this.cumulative.push_str(text);
                                }
                                continue;
                            }
                            AggregateMode::Streaming => {
                                if this.cumulative.is_empty() {
                                    return Poll::Ready(Some(Ok(ResponseEvent::OutputItemDone(
                                        item,
                                    ))));
                                } else {
                                    continue;
                                }
                            }
                        }
                    }

                    return Poll::Ready(Some(Ok(ResponseEvent::OutputItemDone(item))));
                }
                Poll::Ready(Some(Ok(ResponseEvent::RateLimits(snapshot)))) => {
                    return Poll::Ready(Some(Ok(ResponseEvent::RateLimits(snapshot))));
                }
                Poll::Ready(Some(Ok(ResponseEvent::Completed {
                    response_id,
                    token_usage,
                }))) => {
                    let mut emitted_any = false;

                    if !this.cumulative_reasoning.is_empty()
                        && matches!(this.mode, AggregateMode::AggregatedOnly)
                    {
                        let aggregated_reasoning =
                            codex_protocol::models::ResponseItem::Reasoning {
                                id: String::new(),
                                summary: Vec::new(),
                                content: Some(vec![
                                    codex_protocol::models::ReasoningItemContent::ReasoningText {
                                        text: std::mem::take(&mut this.cumulative_reasoning),
                                    },
                                ]),
                                encrypted_content: None,
                            };
                        this.pending
                            .push_back(ResponseEvent::OutputItemDone(aggregated_reasoning));
                        emitted_any = true;
                    }

                    if !this.cumulative.is_empty() {
                        let aggregated_message = codex_protocol::models::ResponseItem::Message {
                            id: None,
                            role: "assistant".to_string(),
                            content: vec![codex_protocol::models::ContentItem::OutputText {
                                text: std::mem::take(&mut this.cumulative),
                            }],
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
                Poll::Ready(Some(Ok(ResponseEvent::Created))) => {
                    continue;
                }
                Poll::Ready(Some(Ok(ResponseEvent::OutputTextDelta(delta)))) => {
                    this.cumulative.push_str(&delta);
                    if matches!(this.mode, AggregateMode::Streaming) {
                        return Poll::Ready(Some(Ok(ResponseEvent::OutputTextDelta(delta))));
                    } else {
                        continue;
                    }
                }
                Poll::Ready(Some(Ok(ResponseEvent::ReasoningContentDelta {
                    delta,
                    content_index,
                }))) => {
                    this.cumulative_reasoning.push_str(&delta);
                    if matches!(this.mode, AggregateMode::Streaming) {
                        return Poll::Ready(Some(Ok(ResponseEvent::ReasoningContentDelta {
                            delta,
                            content_index,
                        })));
                    } else {
                        continue;
                    }
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

/// Extension trait that activates aggregation on any stream of `ResponseEvent`.
pub(crate) trait AggregateStreamExt: Stream<Item = Result<ResponseEvent>> + Sized {
    fn aggregate(self) -> AggregatedChatStream<Self> {
        AggregatedChatStream::new(self, AggregateMode::AggregatedOnly)
    }

    fn streaming_mode(self) -> AggregatedChatStream<Self> {
        AggregatedChatStream::new(self, AggregateMode::Streaming)
    }
}

impl<T> AggregateStreamExt for T where T: Stream<Item = Result<ResponseEvent>> + Sized {}

impl<S> AggregatedChatStream<S> {
    fn new(inner: S, mode: AggregateMode) -> Self {
        AggregatedChatStream {
            inner,
            cumulative: String::new(),
            cumulative_reasoning: String::new(),
            pending: std::collections::VecDeque::new(),
            mode,
        }
    }
}
