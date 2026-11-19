//! Decodes the removed `mcp_attribution` field in old rollouts.
//!
//! Only whether Search MCP may have been used is kept. New rollouts use
//! `model_search_state`.

use serde::Deserialize;
use serde::Deserializer;

const SEARCH_CONNECTOR_ID: &str = "connector_openai_search_service";

#[derive(Deserialize)]
struct LegacyMcpAttribution {
    status: LegacyStatus,
    #[serde(default)]
    sources: Vec<LegacySource>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum LegacyStatus {
    None,
    Complete,
    #[serde(other)]
    Error,
}

#[derive(Deserialize)]
struct LegacySource {
    #[serde(default)]
    connector_id: Option<String>,
}

pub(super) fn deserialize_search_mcp_use<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(Some(
        serde_json::from_value::<LegacyMcpAttribution>(value).map_or(true, |attribution| {
            matches!(attribution.status, LegacyStatus::Error)
                || attribution
                    .sources
                    .iter()
                    .any(|source| source.connector_id.as_deref() == Some(SEARCH_CONNECTOR_ID))
        }),
    ))
}
