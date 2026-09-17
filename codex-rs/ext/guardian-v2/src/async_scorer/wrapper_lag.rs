//! Discounts only an approval's own unscored code-mode wrapper. Missing provenance
//! keeps the full lag; the bounded history never changes scoring or failure order.
//! The score owner's state lock protects this history.

use std::collections::VecDeque;

use codex_extension_api::ToolCallSource;
use codex_extension_api::ToolPayload;
use codex_extension_api::ToolStartInput;
use codex_protocol::ResponseItemId;

struct ToolStart {
    call_id: String,
    index: usize,
    wrapper_item_id: Option<ResponseItemId>,
    parent_wrapper_index: Option<usize>,
}

#[derive(Default)]
pub(super) struct WrapperLag {
    starts: VecDeque<ToolStart>,
}

impl WrapperLag {
    pub(super) fn record(&mut self, input: &ToolStartInput<'_>, index: usize) {
        let starts = &mut self.starts;
        let parent_wrapper_index = match &input.source {
            ToolCallSource::CodeMode { .. } => input.originating_item_id.and_then(|item_id| {
                starts
                    .iter()
                    .rev()
                    .find(|start| start.wrapper_item_id.as_ref() == Some(item_id))
                    .map(|start| start.index)
            }),
            ToolCallSource::Direct => None,
        };
        let wrapper_item_id = if input.tool_name.is_default_namespace()
            && input.tool_name.name == "exec"
            && matches!(input.payload, ToolPayload::Custom { .. })
            && matches!(input.source, ToolCallSource::Direct)
        {
            input.originating_item_id.cloned()
        } else {
            None
        };
        if starts.len() == 256 {
            starts.pop_front();
        }
        starts.push_back(ToolStart {
            call_id: input.call_id.to_owned(),
            index,
            wrapper_item_id,
            parent_wrapper_index,
        });
    }

    pub(super) fn discount(&self, call_id: Option<&str>, latest_scored: usize) -> usize {
        usize::from(
            call_id
                .and_then(|call_id| {
                    self.starts
                        .iter()
                        .rev()
                        .find(|start| start.call_id == call_id)
                        .and_then(|start| start.parent_wrapper_index)
                })
                .is_some_and(|parent| parent > latest_scored),
        )
    }
}
