//! Bounded retention and per-observation rendering for Code Mode output.
//!
//! Keeps omission accounting inside this module until rendered. No actor or
//! transport state belongs here.

use codex_code_mode_protocol::CodeModeOutputTruncation;
use codex_code_mode_protocol::FunctionCallOutputContentItem as OutputItem;
use codex_utils_output_truncation::OutputBoundary;
use codex_utils_string::truncate_middle_with_marker;

mod output_collection;
mod output_render;

pub(crate) use output_collection::BoundedOutput;
#[cfg(test)]
pub(crate) use output_collection::MAX_OUTPUT_ITEMS;
pub(crate) use output_render::render_output;

/// Bound the buffered server diagnostic independently of model-visible output.
pub(crate) const MAX_SERVER_DIAGNOSTIC_BYTES: usize = 64 * 1024;

/// The actor transfers the whole collector at each observation, so a failed
/// synchronous send can restore the original state, including any omissions.
#[derive(Clone, Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub(crate) enum OutputCollector {
    Client(Vec<OutputItem>),
    /// Allocate storage only when the observation receives output.
    Server(Option<Box<BoundedOutput>>),
}

impl OutputCollector {
    pub(crate) fn new(mode: CodeModeOutputTruncation) -> Self {
        match mode {
            CodeModeOutputTruncation::Client => Self::Client(Vec::new()),
            CodeModeOutputTruncation::Server => Self::Server(None),
        }
    }

    pub(crate) fn push(&mut self, item: OutputItem) {
        match self {
            Self::Client(items) => items.push(item),
            Self::Server(output) => output.get_or_insert_with(Box::default).push(item),
        }
    }

    /// Server errors share the observation's output budget; their diagnostic
    /// has an independent bound. Client errors retain their original formatting.
    pub(crate) fn collect_error(&mut self, error_text: String) -> String {
        match self {
            Self::Client(_) => error_text,
            Self::Server(output) => {
                output
                    .get_or_insert_with(Box::default)
                    .push_error(&error_text);
                // Reuse small errors without retaining oversized backing storage.
                if error_text.capacity() <= MAX_SERVER_DIAGNOSTIC_BYTES {
                    error_text
                } else {
                    truncate_middle_with_marker(&error_text, MAX_SERVER_DIAGNOSTIC_BYTES, "…")
                }
            }
        }
    }

    pub(crate) fn take(&mut self) -> Self {
        match self {
            Self::Client(items) => Self::Client(std::mem::take(items)),
            Self::Server(output) => Self::Server(output.take()),
        }
    }

    pub(crate) fn try_deliver(&mut self, send: impl FnOnce(Self) -> Result<(), Self>) -> bool {
        if let Err(undelivered) = send(self.take()) {
            *self = undelivered;
            return false;
        }
        true
    }

    /// Flatten the retained state only after the observer owns it.
    pub(crate) fn into_collected_output(self) -> CollectedOutput {
        match self {
            Self::Client(items) => items.into(),
            Self::Server(Some(output)) => output.into_collected_output(),
            Self::Server(None) => CollectedOutput::default(),
        }
    }
}

/// Retained content and where the collector had to drop data. Loss is private
/// to the runtime until it is rendered as text after the observation's budget.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CollectedOutput {
    items: Vec<OutputItem>,
    gaps: Vec<OutputGap>,
}

impl From<Vec<OutputItem>> for CollectedOutput {
    fn from(items: Vec<OutputItem>) -> Self {
        Self {
            items,
            gaps: Vec::new(),
        }
    }
}

impl CollectedOutput {
    fn push_gap(&mut self, boundary: OutputBoundary, omitted: OutputOmission) {
        if let Some(gap) = self.gaps.last_mut()
            && gap.boundary == boundary
        {
            gap.omitted.combine(omitted);
        } else {
            self.gaps.push(OutputGap { boundary, omitted });
        }
    }

    /// Takes the unformatted items. Only for client-side observations, where
    /// the caller already owns truncation and the input was not bounded.
    pub(crate) fn into_untruncated_items(self) -> Vec<OutputItem> {
        self.items
    }
}

/// Missing output at a boundary in the retained logical items. A nonzero
/// UTF-8 byte offset marks loss inside a single text item.
#[derive(Clone, Copy, Debug, PartialEq)]
struct OutputGap {
    boundary: OutputBoundary,
    omitted: OutputOmission,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct OutputOmission {
    text_bytes: u64,
    text_items: u64,
    image_items: u64,
    audio_items: u64,
}

impl OutputOmission {
    fn combine(&mut self, other: Self) {
        self.text_bytes = self.text_bytes.saturating_add(other.text_bytes);
        self.text_items = self.text_items.saturating_add(other.text_items);
        self.image_items = self.image_items.saturating_add(other.image_items);
        self.audio_items = self.audio_items.saturating_add(other.audio_items);
    }
}
