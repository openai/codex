//! Caller-specific explicit access programs advertised by the model catalog.
//!
//! Discovery metadata does not grant access; inference still enforces authorization.

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use ts_rs::TS;

use crate::turn_input::CyberAccessProgram;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, TS, JsonSchema)]
pub struct ModelAccessPrograms {
    /// Accepted explicit selections. An empty list is distinct from missing metadata.
    #[serde(deserialize_with = "deserialize_known_cyber_access_programs")]
    pub cyber: Vec<CyberAccessProgram>,
}

fn deserialize_known_cyber_access_programs<'de, D>(
    deserializer: D,
) -> Result<Vec<CyberAccessProgram>, D::Error>
where
    D: Deserializer<'de>,
{
    // New server programs must not prevent older clients from loading the catalog.
    Ok(Vec::<String>::deserialize(deserializer)?
        .into_iter()
        .filter_map(|program| serde_json::from_value(serde_json::Value::String(program)).ok())
        .collect())
}
