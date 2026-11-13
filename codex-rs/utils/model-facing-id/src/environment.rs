//! Distinguishes client executor IDs from aliases accepted in model-facing environment arguments.

use std::fmt;

use serde::Deserialize;
use serde::Serialize;

use crate::ModelFacingId;
use crate::ModelIdKind;

/// An ID assigned to an executor by the client.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RegisteredEnvironmentId(String);

impl RegisteredEnvironmentId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RegisteredEnvironmentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The environment namespace supplies the prefix when this value is displayed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ModelEnvironmentId(u64);

impl fmt::Display for ModelEnvironmentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}",
            RegisteredEnvironmentId::PREFIX,
            crate::padded_digits(self.0)
        )
    }
}

impl ModelIdKind for RegisteredEnvironmentId {
    const PREFIX: &'static str = "env";
    type Alias = ModelEnvironmentId;
}

impl From<ModelFacingId> for ModelEnvironmentId {
    fn from(id: ModelFacingId) -> Self {
        Self(id.value)
    }
}
