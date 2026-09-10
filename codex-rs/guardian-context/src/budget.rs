//! Payload-free accounting for composed sections and complete Guardian requests.
//! Text and image bytes stay separate. Hosts provide existing-context estimates;
//! the shared wire estimate reserves image tokens without counting base64 as text.

use std::io::Write;

use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TruncationPolicy;

use crate::ComposedContext;
use crate::composition::SectionDelivery;
use crate::composition::SectionOutput;

pub const SECTION_COST_METRIC: &str = "codex.guardian.context.section_cost";
pub const REQUEST_TOKENS_METRIC: &str = "codex.guardian.context.request_tokens";
/// Fixed across reviewers and models so complete-request distributions align.
pub const REQUEST_TOKENS_BOUNDARIES: &[f64] = &[
    1_000.0,
    4_000.0,
    16_000.0,
    32_000.0,
    64_000.0,
    128_000.0,
    256_000.0,
    400_000.0,
    512_000.0,
    1_000_000.0,
    2_000_000.0,
];
/// One shared scale for the section metric's count, token and byte measurements.
/// Buckets belong to the metric name, not its measurement tag.
pub const SECTION_COST_BOUNDARIES: &[f64] = &[
    0.0,
    1.0,
    2.0,
    4.0,
    8.0,
    16.0,
    64.0,
    256.0,
    1_024.0,
    4_096.0,
    16_384.0,
    65_536.0,
    262_144.0,
    1_048_576.0,
    4_194_304.0,
    8_388_608.0,
    16_777_216.0,
];
// A conservative reservation matching the existing original-image patch ceiling.
const IMAGE_TOKEN_RESERVATION: usize = 10_000;
/// Conservative input ceiling when the model catalog has no authoritative window.
pub const DEFAULT_MAX_INPUT_TOKENS: usize = 128_000;

/// Applies the same configured-window cap and effective percentage as sync review.
/// Async callers supply no parent-model override.
pub fn effective_input_token_limit(
    model: &codex_protocol::openai_models::ModelInfo,
    configured_window: Option<i64>,
) -> usize {
    let supported = model
        .resolved_context_window()
        .unwrap_or(DEFAULT_MAX_INPUT_TOKENS as i64);
    let supported = if model.used_fallback_model_metadata {
        supported.min(DEFAULT_MAX_INPUT_TOKENS as i64)
    } else {
        supported
    };
    let limit = configured_window
        .unwrap_or(supported)
        .min(supported)
        .saturating_mul(model.effective_context_window_percent.clamp(0, 100))
        / 100;
    usize::try_from(limit.max(0)).unwrap_or(usize::MAX)
}

/// Complete input allowance after the host resolves its model and configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequestBudget {
    pub max_input_tokens: usize,
    pub existing_context_tokens: usize,
}

/// Size of section evidence, excluding enclosing message serialization overhead.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SectionCost {
    pub text_bytes: usize,
    pub image_bytes: usize,
    pub image_count: usize,
}

impl SectionCost {
    fn add_content(mut self, item: &ContentItem) -> Self {
        match item {
            ContentItem::InputText { text } | ContentItem::OutputText { text } => {
                self.text_bytes = self.text_bytes.saturating_add(text.len());
            }
            ContentItem::InputImage { image_url, .. } => {
                self.image_bytes = self.image_bytes.saturating_add(image_url.len());
                self.image_count = self.image_count.saturating_add(1);
            }
            ContentItem::InputAudio { audio_url } => {
                // Guardian currently has no audio contributor. Count a future opaque
                // payload conservatively until its consumer supplies modality costs.
                self.text_bytes = self.text_bytes.saturating_add(audio_url.len());
            }
        }
        self
    }

    pub fn measurements(self) -> [(&'static str, usize); 4] {
        [
            ("text_bytes", self.text_bytes),
            (
                "estimated_text_tokens",
                TruncationPolicy::Bytes(self.text_bytes).token_budget(),
            ),
            ("image_bytes", self.image_bytes),
            ("image_count", self.image_count),
        ]
    }
}

impl ComposedContext {
    /// Conservative cost of composed evidence, including message wrappers.
    /// Shared user sections each reserve a wrapper, bounding their coalesced cost.
    pub fn estimated_tokens(&self) -> usize {
        self.sections
            .iter()
            .map(section_tokens)
            .fold(0usize, usize::saturating_add)
    }

    /// Stable section names and numeric costs; never exposes evidence in diagnostics.
    pub fn section_costs(&self) -> impl Iterator<Item = (&'static str, SectionCost)> + '_ {
        self.sections.iter().map(|section| {
            let cost = match &section.delivery {
                SectionDelivery::UserContent(content) => content
                    .iter()
                    .map(|item| &item.content)
                    .fold(SectionCost::default(), SectionCost::add_content),
                SectionDelivery::Message(message) => match message.as_ref() {
                    ResponseItem::Message { content, .. } => content
                        .iter()
                        .fold(SectionCost::default(), SectionCost::add_content),
                    item => SectionCost {
                        text_bytes: ByteCount::item(item),
                        ..SectionCost::default()
                    },
                },
            };
            (section.id, cost)
        })
    }
}

/// Conservative estimate including roles, framing, metadata and opaque checkpoints.
/// Images reserve tokens separately from their wire bytes. This is a budgeting
/// heuristic, not the model's tokenizer or a measurement of server-side usage.
pub fn estimate_input_tokens(item: &ResponseItem) -> usize {
    let content = match item {
        ResponseItem::Message { content, .. } => content.as_slice(),
        _ => &[],
    };
    adjusted_tokens(ByteCount::item(item), content)
}

pub(super) fn content_tokens(item: &ContentItem) -> usize {
    let mut bytes = ByteCount::measure(|counter| serde_json::to_writer(counter, item));
    if let ContentItem::InputText { text } = item {
        let extra_parts = crate::composition::bounded_text_parts(text)
            .count()
            .saturating_sub(1);
        let framing = ByteCount::measure(|counter| {
            serde_json::to_writer(
                counter,
                &ContentItem::InputText {
                    text: String::new(),
                },
            )
        });
        bytes = bytes.saturating_add(extra_parts.saturating_mul(framing.saturating_add(1)));
    }
    adjusted_tokens(bytes, std::slice::from_ref(item))
}

pub(super) fn section_tokens(section: &SectionOutput) -> usize {
    match &section.delivery {
        SectionDelivery::UserContent(content) => content
            .iter()
            .map(|item| content_tokens(&item.content))
            .fold(content_framing_tokens(content.len()), usize::saturating_add),
        SectionDelivery::Message(message) => estimate_input_tokens(message),
    }
}

pub(super) fn content_framing_tokens(item_count: usize) -> usize {
    if item_count == 0 {
        return 0;
    }
    estimate_input_tokens(&crate::composition::user_message(Vec::new()))
        .saturating_add(TruncationPolicy::Bytes(item_count - 1).token_budget())
}

fn adjusted_tokens(mut bytes: usize, content: &[ContentItem]) -> usize {
    for item in content {
        if let ContentItem::InputImage { image_url, .. } = item {
            let payload = ByteCount::measure(|counter| serde_json::to_writer(counter, image_url));
            if payload == usize::MAX {
                return usize::MAX;
            }
            bytes = bytes
                .saturating_sub(payload.saturating_sub(2))
                .saturating_add(TruncationPolicy::Tokens(IMAGE_TOKEN_RESERVATION).byte_budget());
        }
    }
    TruncationPolicy::Bytes(bytes).token_budget()
}

#[derive(Default)]
struct ByteCount(usize);

impl ByteCount {
    fn item(item: &ResponseItem) -> usize {
        Self::measure(|counter| serde_json::to_writer(counter, item))
    }

    fn measure(serialize: impl FnOnce(&mut Self) -> serde_json::Result<()>) -> usize {
        let mut count = Self::default();
        if serialize(&mut count).is_err() {
            return usize::MAX;
        }
        count.0
    }
}

impl Write for ByteCount {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 = self.0.saturating_add(bytes.len());
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
#[path = "budget_tests.rs"]
mod tests;
