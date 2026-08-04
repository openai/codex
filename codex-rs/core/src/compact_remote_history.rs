use std::borrow::Borrow;

use crate::context_manager::estimate_item_token_count;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;

const ATTACHED_NOTICE_MARKERS: &[(&str, &str)] =
    &[("<image_resize_notice>", "</image_resize_notice>")];

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HistoryItemGroup<T> {
    pub(crate) source: T,
    pub(crate) attached_notice: Option<T>,
}

impl<T: Borrow<ResponseItem>> HistoryItemGroup<T> {
    pub(crate) fn into_items(self) -> impl Iterator<Item = T> {
        std::iter::once(self.source).chain(self.attached_notice)
    }

    pub(crate) fn estimated_token_count(&self) -> i128 {
        let source_tokens = i128::from(estimate_item_token_count(self.source.borrow()));
        let notice_tokens = self.attached_notice.as_ref().map_or(0, |notice| {
            i128::from(estimate_item_token_count(notice.borrow()))
        });
        source_tokens.saturating_add(notice_tokens)
    }
}

pub(crate) fn history_item_groups<I>(items: I) -> impl Iterator<Item = HistoryItemGroup<I::Item>>
where
    I: IntoIterator,
    I::Item: Borrow<ResponseItem>,
{
    let mut items = items.into_iter().peekable();
    std::iter::from_fn(move || {
        let source = items.next()?;
        let attached_notice = items.next_if(|notice| is_attached_notice(notice.borrow()));
        Some(HistoryItemGroup {
            source,
            attached_notice,
        })
    })
}

fn is_attached_notice(notice: &ResponseItem) -> bool {
    matches!(
        notice,
        ResponseItem::Message { role, content, .. }
            if role == "developer"
                && matches!(
                    content.as_slice(),
                    [ContentItem::InputText { text }]
                        if ATTACHED_NOTICE_MARKERS.iter().any(|(opening, closing)| {
                            text.starts_with(opening) && text.ends_with(closing)
                        })
                )
    )
}
