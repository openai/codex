use code_mode_impl::ImageDetail as CodeModeImageDetail;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::ImageDetail;

use super::code_mode_impl;

trait IntoProtocol<T> {
    fn into_protocol(self) -> T;
}

pub(super) fn into_function_call_output_content_items(
    items: Vec<code_mode_impl::FunctionCallOutputContentItem>,
) -> Vec<FunctionCallOutputContentItem> {
    items.into_iter().map(IntoProtocol::into_protocol).collect()
}

impl IntoProtocol<ImageDetail> for CodeModeImageDetail {
    fn into_protocol(self) -> ImageDetail {
        let value = self;
        match value {
            CodeModeImageDetail::Auto => ImageDetail::Auto,
            CodeModeImageDetail::Low => ImageDetail::Low,
            CodeModeImageDetail::High => ImageDetail::High,
            CodeModeImageDetail::Original => ImageDetail::Original,
        }
    }
}

impl IntoProtocol<FunctionCallOutputContentItem> for code_mode_impl::FunctionCallOutputContentItem {
    fn into_protocol(self) -> FunctionCallOutputContentItem {
        let value = self;
        match value {
            code_mode_impl::FunctionCallOutputContentItem::InputText { text } => {
                FunctionCallOutputContentItem::InputText { text }
            }
            code_mode_impl::FunctionCallOutputContentItem::InputImage { image_url, detail } => {
                FunctionCallOutputContentItem::InputImage {
                    image_url,
                    detail: detail.map(IntoProtocol::into_protocol),
                }
            }
        }
    }
}
