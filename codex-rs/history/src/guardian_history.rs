//! Model-invisible Guardian transcript checkpoints and retained-section delivery proof.
//! Entries preserve rollback provenance; their flattened wire shape still reads as
//! ResponseItem on older hosts, and old metadata-free checkpoints remain readable.

use std::borrow::Cow;

use codex_protocol::models::ResponseItem;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

use crate::CodexHarnessMetadata;
use crate::ResponseItemEnvelope;

/// Host omission notices delivered by a complete retained-instructions section.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GuardianRetainedOmissions {
    pub user_instructions: bool,
    pub assistant_context: bool,
}

/// Original review evidence, separate from the compacted model conversation.
/// Hosts enforce transcript retention limits both when saving and restoring it.
#[derive(Clone, PartialEq)]
pub struct GuardianHistoryCheckpoint(pub Vec<ResponseItemEnvelope>);

#[derive(Serialize, Deserialize, JsonSchema)]
struct Entry<'a> {
    #[serde(flatten)]
    item: Cow<'a, ResponseItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    guardian_metadata: Option<Cow<'a, CodexHarnessMetadata>>,
}

impl Serialize for GuardianHistoryCheckpoint {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.0.iter().map(|entry| Entry {
            item: Cow::Borrowed(&entry.item),
            guardian_metadata: entry.metadata.as_ref().map(Cow::Borrowed),
        }))
    }
}

impl<'de> Deserialize<'de> for GuardianHistoryCheckpoint {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self(
            Vec::<Entry<'_>>::deserialize(deserializer)?
                .into_iter()
                .map(|entry| ResponseItemEnvelope {
                    item: entry.item.into_owned(),
                    metadata: entry.guardian_metadata.map(Cow::into_owned),
                })
                .collect(),
        ))
    }
}

impl JsonSchema for GuardianHistoryCheckpoint {
    fn schema_name() -> String {
        "GuardianHistoryCheckpoint".to_owned()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::schema::Schema {
        Vec::<Entry<'_>>::json_schema(generator)
    }
}

impl std::fmt::Debug for GuardianHistoryCheckpoint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GuardianHistoryCheckpoint")
            .field("items", &self.0.len())
            .finish()
    }
}
