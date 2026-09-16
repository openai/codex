//! Selects the Luna checkpoint once under the session's context mode.
//! Sampling and fast approval share eligibility; legacy omission stays distinct from rejection.

use codex_core::context::GuardianContextMode;
use codex_extension_api::ConversationHistorySnapshot;
use codex_extension_api::ResponseItem;
use codex_protocol::protocol::TruncationPolicy;

use super::config::GuardianV2Config;
use super::sampler::LunaSampler;

#[derive(Debug, Eq, PartialEq)]
pub(super) enum ParentCompactionError {
    RequiresSync,
    Serialization,
    Oversized,
    Unusable,
}

pub(super) struct ParentCompaction {
    pub(super) item: Option<ResponseItem>,
    pub(super) model_hash: Option<String>,
}

pub(super) fn select_parent_compaction(
    mode: GuardianContextMode,
    config: &GuardianV2Config,
    history: &dyn ConversationHistorySnapshot,
    sampler: &LunaSampler,
    legacy_model_hash: Option<&str>,
) -> Result<ParentCompaction, ParentCompactionError> {
    let checkpoint = history.latest_compaction();
    let model_hash = match mode {
        GuardianContextMode::Legacy => legacy_model_hash,
        GuardianContextMode::ThreadOwned => checkpoint.and_then(|checkpoint| checkpoint.model_hash),
    };
    if mode == GuardianContextMode::ThreadOwned
        && checkpoint.is_some()
        && (!config.reuse_parent_compaction || !sampler.supports_parent_compaction(model_hash))
    {
        return Err(ParentCompactionError::RequiresSync);
    }
    let item = if config.reuse_parent_compaction {
        match encrypted_parent_compaction(checkpoint, config.max_parent_compaction_tokens) {
            Ok(item) => item,
            Err(ParentCompactionError::Unusable) if mode == GuardianContextMode::Legacy => None,
            Err(error) => return Err(error),
        }
    } else {
        None
    };
    // Legacy has raw history, so it can omit an incompatible checkpoint. Validate its size
    // before omission, preserving the previous failure behavior for oversized items.
    let item = item.filter(|_| {
        mode == GuardianContextMode::ThreadOwned || sampler.supports_parent_compaction(model_hash)
    });
    Ok(ParentCompaction {
        item,
        model_hash: model_hash.map(str::to_owned),
    })
}

// An unusable latest compaction must never fall back to an older one. Missing
// encrypted content is rejected here; only legacy callers may omit that checkpoint.
fn encrypted_parent_compaction(
    checkpoint: Option<codex_history::CompactionCheckpoint<'_>>,
    max_parent_compaction_tokens: usize,
) -> Result<Option<ResponseItem>, ParentCompactionError> {
    let max_compaction_bytes = TruncationPolicy::Tokens(max_parent_compaction_tokens).byte_budget();
    let Some(checkpoint) = checkpoint else {
        return Ok(None);
    };
    if !checkpoint.is_usable() {
        return Err(ParentCompactionError::Unusable);
    }
    let item = checkpoint.item;
    let serialized = serde_json::to_vec(item).map_err(|_| ParentCompactionError::Serialization)?;
    if serialized.len() > max_compaction_bytes {
        return Err(ParentCompactionError::Oversized);
    }

    Ok(Some(item.clone()))
}

#[cfg(test)]
#[path = "parent_compaction_tests.rs"]
mod tests;
