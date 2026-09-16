//! Latest-checkpoint selection keeps structural validity and producer provenance together.

use super::*;
use crate::CodexHarnessMetadata;
use codex_protocol::ResponseItemId;
use pretty_assertions::assert_eq;

#[test]
fn latest_checkpoint_keeps_its_own_producer_even_when_unusable() {
    let older = ResponseItemEnvelope {
        item: ResponseItem::Compaction {
            id: Some(ResponseItemId::new("old")),
            encrypted_content: "older checkpoint".to_owned(),
            internal_chat_message_metadata_passthrough: None,
        },
        metadata: Some(CodexHarnessMetadata {
            compaction_model_hash: Some("producer".to_owned()),
            ..Default::default()
        }),
    };
    for content in [None, Some(String::new()), Some("new checkpoint".to_owned())] {
        let usable = content.as_ref().is_some_and(|text| !text.is_empty());
        let latest = ResponseItem::ContextCompaction {
            id: Some(ResponseItemId::new("new")),
            encrypted_content: content,
            internal_chat_message_metadata_passthrough: None,
        };
        let items = [older.clone(), latest.clone().into()];
        let checkpoint = CompactionCheckpoint::latest(&items).expect("latest checkpoint");
        assert_eq!(
            checkpoint,
            CompactionCheckpoint {
                item: &latest,
                model_hash: None
            },
        );
        assert_eq!(checkpoint.is_usable(), usable);
        assert!(!checkpoint.is_compatible_with(Some("producer")));
    }
    let items = [older];
    let checkpoint = CompactionCheckpoint::latest(&items).expect("older checkpoint");
    assert!(checkpoint.is_usable());
    assert!(checkpoint.is_compatible_with(Some("producer")));
    assert!(!checkpoint.is_compatible_with(Some("different")));
    assert!(!checkpoint.is_compatible_with(/*reviewer_model_hash*/ None));
}
