//! Encode queued board posts as whole, ordered originals within the reviewer's model budget.

use crate::EvidenceId;
use crate::sampling::context_input;
use anyhow::ensure;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TruncationPolicy;
use serde::Deserialize;
use serde::Serialize;
use serde_json::value::RawValue;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RoutedOriginal<'a> {
    id: EvidenceId,
    #[serde(borrow)]
    original: &'a RawValue,
}

#[derive(Serialize)]
struct DeliveryInput<'a> {
    sources: Vec<RoutedOriginal<'a>>,
}

/// Encode one whole-original batch in source order. Use the reviewer's selected model budget.
pub(super) fn evidence_batch<'a>(
    posts: impl Iterator<Item = (u64, &'a str)>,
    policy: TruncationPolicy,
) -> anyhow::Result<(ResponseItem, usize)> {
    let mut input = DeliveryInput {
        sources: Vec::new(),
    };
    let mut bytes = serde_json::to_string(&input)?.len();
    for (ordinal, text) in posts {
        let additional = text.len() + usize::from(!input.sources.is_empty());
        if additional > policy.byte_budget().saturating_sub(bytes) {
            ensure!(
                !input.sources.is_empty(),
                "original evidence exceeds the model's tool-output budget"
            );
            break;
        }
        let source: RoutedOriginal<'_> = serde_json::from_str(text)?;
        ensure!(
            source.id.ordinal == ordinal,
            "selected board post has a different source ordinal"
        );
        input.sources.push(source);
        bytes += additional;
    }
    ensure!(!input.sources.is_empty(), "empty evidence batch");
    Ok((
        context_input("guardian_original_evidence", &input, policy)?,
        input.sources.len(),
    ))
}
