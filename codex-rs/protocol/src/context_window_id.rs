//! Context window identity used for compaction, rollout restoration, and request metadata.

use serde::Serialize;
use std::fmt;
use uuid::Uuid;

#[derive(derive_more::Debug, Clone, PartialEq, Eq, Serialize)]
#[debug("{_0}")]
#[serde(transparent)]
pub struct ContextWindowId(String);

impl ContextWindowId {
    pub fn new_uuid_v7() -> Self {
        Self(Uuid::now_v7().to_string())
    }

    /// Reads a persisted window ID, ignoring IDs outside the supported format.
    pub fn parse(value: &str) -> Option<Self> {
        Uuid::parse_str(value)
            .ok()
            .filter(|uuid| uuid.get_version_num() == 7)
            .map(|uuid| Self(uuid.to_string()))
    }
}

impl fmt::Display for ContextWindowId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}
