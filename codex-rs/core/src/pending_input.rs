use codex_protocol::models::ResponseInputItem;
use codex_protocol::protocol::InjectedMessageEvent;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GeneratedMessageInput {
    pub(crate) item: ResponseInputItem,
    pub(crate) injected_event: InjectedMessageEvent,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PendingInputItem {
    Plain(ResponseInputItem),
    GeneratedMessage(GeneratedMessageInput),
}

impl PendingInputItem {
    pub(crate) fn generated_timer_source(&self) -> Option<&str> {
        match self {
            Self::Plain(_) => None,
            Self::GeneratedMessage(generated) => generated
                .injected_event
                .source
                .starts_with("timer ")
                .then_some(generated.injected_event.source.as_str()),
        }
    }

    #[cfg(test)]
    pub(crate) fn into_model_input(self) -> ResponseInputItem {
        match self {
            Self::Plain(item) => item,
            Self::GeneratedMessage(generated) => generated.item,
        }
    }
}

impl From<ResponseInputItem> for PendingInputItem {
    fn from(item: ResponseInputItem) -> Self {
        Self::Plain(item)
    }
}
