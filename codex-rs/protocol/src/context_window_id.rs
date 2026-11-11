//! Rollouts store a UUIDv7 string or a window allocation. Request metadata uses its display form.

use codex_utils_model_facing_id::ModelFacingId;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use std::fmt;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum ContextWindowId {
    Text(String),
    Generated(Box<ModelFacingId>),
}

impl ContextWindowId {
    pub fn new_uuid_v7() -> Self {
        Self::Text(Uuid::now_v7().to_string())
    }

    pub fn allocation(&self) -> Option<&ModelFacingId> {
        match self {
            Self::Text(_) => None,
            Self::Generated(id) => (id.salt() == "window").then_some(id),
        }
    }

    /// Parses a UUIDv7 window ID from a rollout string.
    pub fn parse(value: &str) -> Option<Self> {
        Uuid::parse_str(value)
            .ok()
            .filter(|uuid| uuid.get_version_num() == 7)
            .map(|uuid| Self::Text(uuid.to_string()))
    }

    /// Restore the window without discarding its allocation ordinal.
    pub fn validated(&self) -> Option<Self> {
        match self {
            Self::Text(text) => Self::parse(text),
            Self::Generated(_) => self.allocation().map(|_| self.clone()),
        }
    }
}

impl fmt::Display for ContextWindowId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(text) => formatter.write_str(text),
            Self::Generated(id) => id.fmt(formatter),
        }
    }
}

#[cfg(test)]
#[path = "context_window_id_tests.rs"]
mod tests;
