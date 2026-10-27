use std::fmt;
use std::ops::Deref;

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

/// A Responses API item ID. New IDs require an explicit prefix; deserialization
/// remains permissive so legacy rollouts can still be read.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema, TS,
)]
#[serde(transparent)]
#[schemars(with = "String")]
#[ts(type = "string")]
pub struct ResponseItemId(String);

impl ResponseItemId {
    pub fn new(prefix: &str) -> Self {
        Self::with_suffix(prefix, uuid::Uuid::now_v7())
    }

    pub fn with_suffix(prefix: &str, suffix: impl fmt::Display) -> Self {
        Self(format!("{prefix}_{suffix}"))
    }

    /// Reserved for physical user inputs admitted by the host. Raw item
    /// ingestion must reject this namespace when it accepts caller-supplied IDs.
    pub fn for_admitted_user_input(uuid: uuid::Uuid) -> Self {
        Self::with_suffix("msg_input", uuid)
    }

    pub fn is_admitted_user_input(&self) -> bool {
        self.0.starts_with("msg_input_")
    }

    pub fn from_server(value: String) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_prefixed(&self) -> bool {
        self.split_once('_')
            .is_some_and(|(prefix, suffix)| !prefix.is_empty() && !suffix.is_empty())
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

impl From<ResponseItemId> for String {
    fn from(value: ResponseItemId) -> Self {
        value.0
    }
}

#[cfg(test)]
#[path = "response_item_id_tests.rs"]
mod tests;
