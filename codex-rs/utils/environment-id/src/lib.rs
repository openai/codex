//! Validated identifiers for configured Codex execution environments.
//!
//! Environment identifiers are opaque strings. Their contents are not parsed
//! for routing or filesystem semantics, but the shared type enforces the
//! boundary constraints required wherever an identifier is persisted or
//! embedded in a resource URI.

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use std::fmt;
use std::str::FromStr;
use thiserror::Error;
use ts_rs::TS;

/// Maximum UTF-8 byte length accepted for an environment identifier.
pub const MAX_ENVIRONMENT_ID_LEN: usize = 64;

/// An opaque identifier for a configured execution environment.
///
/// The URI path dot segments `.` and `..` are excluded so every valid
/// identifier can be embedded as a single hierarchical URI path segment
/// without changing the meaning of the surrounding path.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, JsonSchema, TS)]
#[serde(transparent)]
#[schemars(with = "String")]
#[ts(type = "string")]
pub struct EnvironmentId(String);

impl EnvironmentId {
    pub fn new(id: impl Into<String>) -> Result<Self, EnvironmentIdError> {
        let id = id.into();
        validate_environment_id(&id)?;
        Ok(Self(id))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EnvironmentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AsRef<str> for EnvironmentId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl FromStr for EnvironmentId {
    type Err = EnvironmentIdError;

    fn from_str(id: &str) -> Result<Self, Self::Err> {
        Self::new(id)
    }
}

impl<'de> Deserialize<'de> for EnvironmentId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

fn validate_environment_id(id: &str) -> Result<(), EnvironmentIdError> {
    if id.is_empty() {
        return Err(EnvironmentIdError::Empty);
    }
    if matches!(id, "." | "..") {
        return Err(EnvironmentIdError::DotSegment(id.to_string()));
    }
    if id.len() > MAX_ENVIRONMENT_ID_LEN {
        return Err(EnvironmentIdError::TooLong {
            length: id.len(),
            max_length: MAX_ENVIRONMENT_ID_LEN,
        });
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum EnvironmentIdError {
    #[error("environment id cannot be empty")]
    Empty,
    #[error("environment id `{0}` cannot be a URI path dot segment")]
    DotSegment(String),
    #[error("environment id is {length} bytes; maximum length is {max_length}")]
    TooLong { length: usize, max_length: usize },
}

#[cfg(test)]
#[path = "environment_id_tests.rs"]
mod tests;
