use std::cmp::Ordering;
use std::fmt;
use std::hash::Hash;
use std::hash::Hasher;
use std::ops::Deref;

use codex_utils_model_facing_id::ModelFacingId;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

/// A Responses API item ID. New IDs require an explicit prefix; deserialization
/// remains permissive so legacy rollouts can still be read.
#[derive(Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(untagged)]
pub enum ResponseItemId {
    Text(String),
    // TypeScript describes the API, where both variants are strings.
    #[ts(skip)]
    Generated {
        id: String,
        id_allocation: Box<ResponseItemIdAllocation>,
    },
}

/// The generator allocation and the Responses-specific parts of a structured item ID.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ResponseItemIdAllocation {
    /// Item kind, e.g. `msg` in `msg_item042`. Bare request reservations omit it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    #[serde(flatten)]
    pub model_id: ModelFacingId,
    /// Provider-assigned tail, including its separator.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suffix: Option<String>,
}

/// Responses and raw item events exchange optional string IDs.
#[derive(Deserialize, JsonSchema)]
#[serde(transparent)]
pub(crate) struct OptionalWireItemId(Option<String>);

impl OptionalWireItemId {
    pub(crate) fn serialize<S: serde::Serializer>(
        id: &Option<ResponseItemId>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        id.as_deref().serialize(serializer)
    }

    pub(crate) fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<ResponseItemId>, D::Error> {
        Ok(<Self as Deserialize>::deserialize(deserializer)?
            .0
            .map(ResponseItemId::from_server))
    }
}

impl ResponseItemId {
    pub fn with_suffix(prefix: &str, suffix: impl fmt::Display) -> Self {
        Self::Text(format!("{prefix}_{suffix}"))
    }

    /// Keep the allocation that produced this ID while rendering a string on the Responses wire.
    pub fn generated(kind: &str, model_id: ModelFacingId) -> Self {
        let (prefix, id) = if kind == model_id.salt() {
            (None, model_id.to_string())
        } else {
            (Some(kind.to_owned()), format!("{kind}_{model_id}"))
        };
        Self::Text(id).with_allocation(ResponseItemIdAllocation {
            prefix,
            model_id,
            suffix: None,
        })
    }

    /// Restore a known allocation while keeping the exact wire ID.
    pub fn with_allocation(self, allocation: ResponseItemIdAllocation) -> Self {
        Self::Generated {
            id: self.into(),
            id_allocation: Box::new(allocation),
        }
    }

    pub fn allocation(&self) -> Option<&ResponseItemIdAllocation> {
        match self {
            Self::Text(_) => None,
            Self::Generated { id_allocation, .. } => Some(id_allocation),
        }
    }

    /// Reserved for physical user inputs admitted by the host. Raw item
    /// ingestion must reject this namespace when it accepts caller-supplied IDs.
    pub fn for_admitted_user_input(uuid: uuid::Uuid) -> Self {
        Self::with_suffix("msg_input", uuid)
    }

    pub fn is_admitted_user_input(&self) -> bool {
        self.starts_with("msg_input_")
    }

    pub fn from_server(value: String) -> Self {
        Self::Text(value)
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Text(id) | Self::Generated { id, .. } => id,
        }
    }

    pub fn is_prefixed(&self) -> bool {
        self.split_once('_')
            .is_some_and(|(prefix, suffix)| !prefix.is_empty() && !suffix.is_empty())
    }
}

// Lookups by a wire ID must also find an item restored with allocation provenance.
impl PartialEq for ResponseItemId {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}
impl Eq for ResponseItemId {}
impl Hash for ResponseItemId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}
impl Ord for ResponseItemId {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_str().cmp(other.as_str())
    }
}
impl PartialOrd for ResponseItemId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Deref for ResponseItemId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl AsRef<str> for ResponseItemId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for ResponseItemId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl fmt::Debug for ResponseItemId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("ResponseItemId")
            .field(&self.as_str())
            .finish()
    }
}

impl From<ResponseItemId> for String {
    fn from(value: ResponseItemId) -> Self {
        match value {
            ResponseItemId::Text(id) | ResponseItemId::Generated { id, .. } => id,
        }
    }
}

#[cfg(test)]
#[path = "response_item_id_tests.rs"]
mod tests;
