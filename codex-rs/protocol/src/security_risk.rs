use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

/// A durable, thread-owned security risk classifier score.
///
/// These records belong to rollout history only and must not enter model-visible
/// conversation context or user-visible thread item projections.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SecurityRiskScore {
    pub category: String,
    pub score: f64,
}
