//! Fire-and-forget maintenance requests for local rollout storage.

use crate::JsonSchema;
use crate::TS;
use serde::Deserialize;
use serde::Serialize;

/// Acknowledges the compression trigger, not completion. Existing maintenance
/// locks and cooldowns can cause the background pass to skip without doing work.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct RolloutCompressResponse {}
