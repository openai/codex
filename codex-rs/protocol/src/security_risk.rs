use chrono::DateTime;
use chrono::Utc;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;

/// A durable, thread-owned snapshot of security risk classifier scores.
///
/// These records belong to rollout history only and must not enter model-visible
/// conversation context or user-visible thread item projections.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SecurityRiskScore {
    pub scores: BTreeMap<String, f64>,
    /// When sampling started, if this snapshot was written by a timestamp-aware client.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub sampled_at: Option<DateTime<Utc>>,
}
