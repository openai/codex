//! Payload-free accounting for composed sections and complete Guardian requests.
//! Elastic section policies are validated without changing fixed request preparation.
//! Text and image bytes stay separate. Hosts provide existing-context estimates;
//! the shared wire estimate reserves image tokens without counting base64 as text.

use codex_protocol::openai_models::CatalogModel;
use codex_protocol::openai_models::ModelOrigin;
use std::io::Write;

use codex_protocol::models::AudioReference;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ImageReference;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TruncationPolicy;

use crate::ComposedContext;
use crate::composition::SectionContent;
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

/// Returns a bounded, sortable label with 20k steps and a final 256k cutoff.
/// Upper bounds are inclusive; zero belongs to the first bucket and values above 256k to the last.
pub fn request_token_bucket(tokens: usize) -> &'static str {
    const BOUNDARIES: [usize; 13] = [
        20_000, 40_000, 60_000, 80_000, 100_000, 120_000, 140_000, 160_000, 180_000, 200_000,
        220_000, 240_000, 256_000,
    ];
    const LABELS: [&str; 14] = [
        "01_0-20k",
        "02_20k-40k",
        "03_40k-60k",
        "04_60k-80k",
        "05_80k-100k",
        "06_100k-120k",
        "07_120k-140k",
        "08_140k-160k",
        "09_160k-180k",
        "10_180k-200k",
        "11_200k-220k",
        "12_220k-240k",
        "13_240k-256k",
        "14_256k-max",
    ];
    LABELS[BOUNDARIES.partition_point(|bound| tokens > *bound)]
}

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
pub fn effective_input_token_limit(model: &CatalogModel, configured_window: Option<i64>) -> usize {
    let supported = model
        .resolved_context_window()
        .unwrap_or(DEFAULT_MAX_INPUT_TOKENS as i64);
    let supported = if model.origin == ModelOrigin::Synthetic {
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

/// Section policies named for the source section that owns the evidence.
/// The transcript has separate message and tool groups despite sharing one section.
/// Entry caps apply to section text; minimum allocations reserve optional evidence.
#[derive(
    Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ElasticSection {
    TranscriptMessages,
    TranscriptTools,
    #[serde(alias = "root_messages")]
    RootConversationMessages,
    #[serde(alias = "retained_instructions")]
    RetainedUserInstructions,
    #[serde(alias = "user_answers")]
    TrustedUserAnswers,
    #[serde(alias = "sender_messages")]
    SenderUserMessages,
    #[serde(
        alias = "tool_descriptions",
        alias = "planned_action_tool_descriptions"
    )]
    PlannedAction,
    PreviousReviews,
    TrustedTools,
    TrustedSkills,
    #[serde(alias = "images")]
    TranscriptImages,
    #[serde(alias = "repl_evidence")]
    NodeReplEvidence,
    PermissionContext,
}

/// Per-group elastic limits, validated when decoded; not an activation switch.
/// Entry caps apply to required and optional supporting text; exact actions stay whole.
/// Omitted groups use the synchronous entry caps,
/// reserving one capped entry as their minimum.
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ElasticBudgetConfig {
    #[serde(default)]
    pub sections: std::collections::BTreeMap<ElasticSection, ElasticSectionLimits>,
}

impl ElasticBudgetConfig {
    pub(crate) fn section_limits(&self, section: ElasticSection) -> ElasticSectionLimits {
        self.sections.get(&section).copied().unwrap_or_else(|| {
            let entries = crate::ContextProfile::synchronous().transcript.entry_limits;
            let tokens = if section == ElasticSection::TranscriptMessages {
                entries.message_tokens
            } else if section == ElasticSection::TranscriptTools {
                entries.tool_tokens
            } else {
                crate::preparation::MAX_INSTRUCTION_TOKENS
            };
            ElasticSectionLimits {
                min_total_section_tokens: tokens,
                max_section_entry_tokens: tokens,
            }
        })
    }
}

/// Demand-capped optional minimum allocation and per-entry text allowance for all evidence.
/// The minimum is a reservation, not a maximum: a section may receive spare capacity.
/// Preparation must cap text entries once, including framing and omission markers,
/// before measuring demand. Entries that fit keep their original representation.
/// Entry caps cannot exceed the existing text-part delivery allowance.
/// Exact actions and opaque native payloads stay whole and use global allocation space.
/// Images stay whole and use section/global space rather than the text entry cap.
/// Minimum reservations scale down together to fit the available optional budget.
/// These settings do not change storage bounds or activate elastic allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub struct ElasticSectionLimits {
    min_total_section_tokens: usize,
    max_section_entry_tokens: usize,
}

impl ElasticSectionLimits {
    /// Reject empty quotas, oversized entries and minimums that cannot hold one capped entry.
    pub fn new(
        min_total_section_tokens: usize,
        max_section_entry_tokens: usize,
    ) -> Result<Self, ElasticSectionLimitsError> {
        if min_total_section_tokens == 0 || max_section_entry_tokens == 0 {
            return Err(ElasticSectionLimitsError::ZeroTokens);
        }
        if max_section_entry_tokens > min_total_section_tokens {
            return Err(ElasticSectionLimitsError::EntryExceedsMinimum);
        }
        if max_section_entry_tokens > crate::composition::MAX_TEXT_PART_ESTIMATED_TOKENS {
            return Err(ElasticSectionLimitsError::EntryExceedsDeliveryLimit);
        }
        Ok(Self {
            min_total_section_tokens,
            max_section_entry_tokens,
        })
    }

    pub fn min_total_section_tokens(self) -> usize {
        self.min_total_section_tokens
    }

    pub fn max_section_entry_tokens(self) -> usize {
        self.max_section_entry_tokens
    }
}

impl<'de> serde::Deserialize<'de> for ElasticSectionLimits {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Fields {
            min_total_section_tokens: usize,
            max_section_entry_tokens: usize,
        }

        let fields = Fields::deserialize(deserializer)?;
        Self::new(
            fields.min_total_section_tokens,
            fields.max_section_entry_tokens,
        )
        .map_err(serde::de::Error::custom)
    }
}

/// Invalid elastic limits cannot enter preparation through construction or decoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ElasticSectionLimitsError {
    ZeroTokens,
    EntryExceedsMinimum,
    EntryExceedsDeliveryLimit,
}

impl std::fmt::Display for ElasticSectionLimitsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroTokens => {
                formatter.write_str("elastic section token limits must be greater than zero")
            }
            Self::EntryExceedsMinimum => formatter
                .write_str("min_total_section_tokens must be at least max_section_entry_tokens"),
            Self::EntryExceedsDeliveryLimit => write!(
                formatter,
                "max_section_entry_tokens must not exceed {}",
                crate::composition::MAX_TEXT_PART_ESTIMATED_TOKENS
            ),
        }
    }
}

impl std::error::Error for ElasticSectionLimitsError {}

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
            ContentItem::InputText { text } | ContentItem::OutputText { text, .. } => {
                self.text_bytes = self.text_bytes.saturating_add(text.len());
            }
            ContentItem::InputImage {
                image: ImageReference::Inline { image_url },
                ..
            } => {
                self.image_bytes = self.image_bytes.saturating_add(image_url.len());
                self.image_count = self.image_count.saturating_add(1);
            }
            ContentItem::InputImage {
                image: ImageReference::File { .. },
                ..
            } => {
                self.image_count = self.image_count.saturating_add(1);
            }
            ContentItem::InputAudio {
                audio: AudioReference::Inline { audio_url },
            } => {
                // Guardian has no audio contributor. Keep the existing inline
                // transport cost, but do not treat a file handle as evidence.
                self.text_bytes = self.text_bytes.saturating_add(audio_url.len());
            }
            ContentItem::InputAudio {
                audio: AudioReference::File { .. },
            } => {}
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
        let mut costs: Vec<(&'static str, SectionCost)> = Vec::new();
        for section in &self.sections {
            let cost = match &section.delivery {
                SectionDelivery::UserContent(content) => content
                    .iter()
                    .map(|item| &item.content)
                    .fold(SectionCost::default(), |mut cost, item| match item {
                        SectionContent::Transcript(text) => {
                            cost.text_bytes =
                                cost.text_bytes.saturating_add(text.rendered().text_bytes);
                            cost
                        }
                        SectionContent::Other(item) | SectionContent::Exact(item) => {
                            cost.add_content(item)
                        }
                    }),
                SectionDelivery::Message(message) => match &message.content.item {
                    ResponseItem::Message { content, .. } => content
                        .iter()
                        .fold(SectionCost::default(), SectionCost::add_content),
                    item => SectionCost {
                        text_bytes: ByteCount::item(item).saturating_add(
                            crate::native_tool_call::evidence_output(item)
                                .as_ref()
                                .map_or(0, ByteCount::item),
                        ),
                        ..SectionCost::default()
                    },
                },
            };
            // Native messages split a transcript into adjacent deliveries, but
            // telemetry must still report one total for the logical section.
            if let Some((id, total)) = costs.last_mut()
                && *id == section.id
            {
                total.text_bytes = total.text_bytes.saturating_add(cost.text_bytes);
                total.image_bytes = total.image_bytes.saturating_add(cost.image_bytes);
                total.image_count = total.image_count.saturating_add(cost.image_count);
            } else {
                costs.push((section.id, cost));
            }
        }
        costs.into_iter()
    }
}

/// Coarse estimate including roles, framing, metadata and opaque checkpoints.
/// Images reserve tokens separately from their wire bytes. This is a budgeting
/// heuristic, not the model's tokenizer or a measurement of server-side usage.
pub fn estimate_input_tokens(item: &ResponseItem) -> usize {
    estimate_input_tokens_from_serialized_len(item, ByteCount::item(item))
}

/// Estimate input tokens while reusing the length of an existing serialization.
/// `serialized_len` must be the byte length of the item's `serde_json` encoding.
pub fn estimate_input_tokens_from_serialized_len(
    item: &ResponseItem,
    serialized_len: usize,
) -> usize {
    if serialized_len == usize::MAX {
        return usize::MAX;
    }
    if let Some(encrypted_content) = compaction_ciphertext(item) {
        let framing =
            TruncationPolicy::Bytes(serialized_len.saturating_sub(encrypted_content.len()))
                .token_budget();
        let content =
            codex_protocol::protocol::estimate_compaction_content_length(encrypted_content.len());
        return framing.saturating_add(TruncationPolicy::Bytes(content).token_budget());
    }
    let content = match item {
        ResponseItem::Message { content, .. } => content.as_slice(),
        _ => &[],
    };
    adjusted_tokens(serialized_len, content)
}

fn compaction_ciphertext(item: &ResponseItem) -> Option<&str> {
    match item {
        ResponseItem::Compaction {
            encrypted_content, ..
        }
        | ResponseItem::ContextCompaction {
            encrypted_content: Some(encrypted_content),
            ..
        } => Some(encrypted_content),
        _ => None,
    }
}

pub(super) fn section_content_tokens(item: &SectionContent) -> usize {
    match item {
        SectionContent::Transcript(record) => record.rendered().tokens,
        SectionContent::Other(item) | SectionContent::Exact(item) => content_tokens(item),
    }
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
            .map(|item| section_content_tokens(&item.content))
            .fold(content_framing_tokens(content.len()), usize::saturating_add),
        SectionDelivery::Message(message) => {
            crate::native_tool_call::estimate_tokens(&message.content.item)
        }
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
        if let ContentItem::InputImage { image, .. } = item {
            if let ImageReference::Inline { image_url } = image {
                let payload =
                    ByteCount::measure(|counter| serde_json::to_writer(counter, image_url));
                if payload == usize::MAX {
                    return usize::MAX;
                }
                bytes = bytes.saturating_sub(payload.saturating_sub(2));
            }
            bytes = bytes
                .saturating_add(TruncationPolicy::Tokens(IMAGE_TOKEN_RESERVATION).byte_budget());
        }
    }
    TruncationPolicy::Bytes(bytes).token_budget()
}

#[derive(Default)]
pub(crate) struct ByteCount(usize);

impl ByteCount {
    pub(crate) fn item(item: &ResponseItem) -> usize {
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
