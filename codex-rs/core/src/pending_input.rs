//! Pending input queued for delivery into a future or active regular turn.
//!
//! Most pending input is an ordinary model input item. Generated input, such as
//! a fired timer or external queued message, carries the model-visible item plus
//! a separate display event so clients can render the human-facing content
//! without parsing the XML envelope recorded in model history.

use codex_protocol::models::ResponseInputItem;
use codex_protocol::protocol::InjectedMessageEvent;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PendingInputItem {
    item: ResponseInputItem,
    injected_event: Option<InjectedMessageEvent>,
}

impl PendingInputItem {
    pub(crate) fn injected(item: ResponseInputItem, event: InjectedMessageEvent) -> Self {
        Self {
            item,
            injected_event: Some(event),
        }
    }

    pub(crate) fn timer_source(&self) -> Option<&str> {
        self.injected_event
            .as_ref()
            .filter(|event| event.source.starts_with("timer "))
            .map(|event| event.source.as_str())
    }

    pub(crate) fn into_parts(self) -> (ResponseInputItem, Option<InjectedMessageEvent>) {
        (self.item, self.injected_event)
    }

    #[cfg(test)]
    pub(crate) fn into_model_input(self) -> ResponseInputItem {
        self.item
    }
}

impl From<ResponseInputItem> for PendingInputItem {
    fn from(item: ResponseInputItem) -> Self {
        Self {
            item,
            injected_event: None,
        }
    }
}
