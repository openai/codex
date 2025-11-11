use std::collections::VecDeque;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use futures::Stream;

use crate::error::Result;
use crate::stream::ResponseEvent;

pub trait AggregateStreamExt: Stream<Item = Result<ResponseEvent>> + Sized {
    fn aggregate(self) -> AggregatedChatStream<Self>
    where
        Self: Unpin,
    {
        AggregatedChatStream::new(self, AggregateMode::AggregatedOnly)
    }

    fn streaming_mode(self) -> AggregatedChatStream<Self>
    where
        Self: Unpin,
    {
        AggregatedChatStream::new(self, AggregateMode::Streaming)
    }
}

impl<S> AggregateStreamExt for S where S: Stream<Item = Result<ResponseEvent>> + Sized + Unpin {}

enum AggregateMode {
    AggregatedOnly,
    Streaming,
}

pub struct AggregatedChatStream<S> {
    inner: S,
    cumulative: String,
    cumulative_reasoning: String,
    pending: VecDeque<ResponseEvent>,
    mode: AggregateMode,
}

impl<S> AggregatedChatStream<S>
where
    S: Stream<Item = Result<ResponseEvent>> + Unpin,
{
    fn new(inner: S, mode: AggregateMode) -> Self {
        Self {
            inner,
            cumulative: String::new(),
            cumulative_reasoning: String::new(),
            pending: VecDeque::new(),
            mode,
        }
    }
}

impl<S> Stream for AggregatedChatStream<S>
where
    S: Stream<Item = Result<ResponseEvent>> + Unpin,
{
    type Item = Result<ResponseEvent>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some(ev) = self.pending.pop_front() {
            return Poll::Ready(Some(Ok(ev)));
        }

        loop {
            match Pin::new(&mut self.inner).poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Ready(Some(Err(err))) => {
                    return Poll::Ready(Some(Err(err)));
                }
                Poll::Ready(Some(Ok(ResponseEvent::OutputItemDone(item)))) => {
                    let is_assistant_message = matches!(
                        &item,
                        ResponseItem::Message { role, .. } if role == "assistant"
                    );

                    if is_assistant_message {
                        if let ResponseItem::Message { role, content, .. } = item {
                            let mut text = String::new();
                            for c in content {
                                match c {
                                    ContentItem::InputText { text: t }
                                    | ContentItem::OutputText { text: t } => text.push_str(&t),
                                    ContentItem::InputImage { image_url } => {
                                        text.push_str(&image_url)
                                    }
                                }
                            }
                            self.cumulative.push_str(&text);
                            if matches!(self.mode, AggregateMode::Streaming) {
                                let output_item =
                                    ResponseEvent::OutputItemDone(ResponseItem::Message {
                                        id: None,
                                        role,
                                        content: vec![ContentItem::OutputText {
                                            text: self.cumulative.clone(),
                                        }],
                                    });
                                self.pending.push_back(output_item);
                            }
                        }
                    } else {
                        return Poll::Ready(Some(Ok(ResponseEvent::OutputItemDone(
                            item,
                        ))));
                    }
                }
                Poll::Ready(Some(Ok(ResponseEvent::OutputItemAdded(item)))) => {
                    if !matches!(
                        &item,
                        ResponseItem::Message { role, .. } if role == "assistant"
                    ) {
                        return Poll::Ready(Some(Ok(ResponseEvent::OutputItemAdded(
                            item,
                        ))));
                    }
                }
                Poll::Ready(Some(Ok(ResponseEvent::ReasoningContentDelta(delta)))) => {
                    self.cumulative_reasoning.push_str(&delta);
                    if matches!(self.mode, AggregateMode::Streaming) {
                        let ev =
                            ResponseEvent::ReasoningContentDelta(self.cumulative_reasoning.clone());
                        self.pending.push_back(ev);
                    }
                }
                Poll::Ready(Some(Ok(ResponseEvent::ReasoningSummaryDelta(delta)))) => {
                    if matches!(self.mode, AggregateMode::Streaming) {
                        let ev = ResponseEvent::ReasoningSummaryDelta(delta);
                        self.pending.push_back(ev);
                    }
                }
                Poll::Ready(Some(Ok(ResponseEvent::Completed {
                    response_id,
                    token_usage,
                }))) => {
                    let assistant_event = ResponseEvent::OutputItemDone(ResponseItem::Message {
                        id: None,
                        role: "assistant".to_string(),
                        content: vec![ContentItem::OutputText {
                            text: self.cumulative.clone(),
                        }],
                    });
                    let completion_event = ResponseEvent::Completed {
                        response_id,
                        token_usage,
                    };

                    if matches!(self.mode, AggregateMode::Streaming) {
                        self.pending.push_back(assistant_event);
                        self.pending.push_back(completion_event);
                    } else {
                        return Poll::Ready(Some(Ok(assistant_event)));
                    }
                }
                Poll::Ready(Some(Ok(ev))) => {
                    return Poll::Ready(Some(Ok(ev)));
                }
            }

            if let Some(ev) = self.pending.pop_front() {
                return Poll::Ready(Some(Ok(ev)));
            }
        }
    }
}
