//! Checks history estimates with and without trusted file-audio durations.
#![cfg(test)]

use super::*;
use crate::context_manager::ContextManager;
use codex_protocol::models::BaseInstructions;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::user_input::UserInput;
use codex_protocol_test_support::uuid_v7_item_id_generator_for_test;
use codex_utils_output_truncation::TruncationPolicy;
use pretty_assertions::assert_eq;

#[test]
fn history_sums_measured_and_unknown_file_audio() {
    let mut history = ContextManager::new(uuid_v7_item_id_generator_for_test());
    let item = ResponseInputItem::from(
        ["short", "unknown"]
            .map(|file_id| UserInput::Audio {
                audio: AudioReference::File {
                    file_id: file_id.into(),
                },
            })
            .to_vec(),
    );
    history.record_items(
        [&ResponseItem::from(item)],
        TruncationPolicy::Tokens(10_000),
    );
    let base = BaseInstructions {
        text: String::new(),
        provenance: None,
    };
    assert_eq!(
        history.estimate_token_count_with_base_instructions(&base),
        Some(24_000)
    );
    history.file_audio_durations = Arc::new(FileAudioDurations(BTreeMap::from([(
        "short".into(),
        Duration::from_secs(30),
    )])));
    assert_eq!(
        history.estimate_token_count_with_base_instructions(&base),
        Some(12_300)
    );
    assert_eq!(history.estimated_tokens_since_last_usage(), 12_300);
}
