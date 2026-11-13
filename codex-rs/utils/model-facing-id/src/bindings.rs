//! Stable names for external objects in low-cardinality ID spaces.

use std::hash::Hash;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::ModelFacingId;
use crate::ModelFacingIdGenerator;
use crate::ModelFacingIdGeneratorState;
use crate::RestoreError;

/// The key type selects the alias type and a unique, stable prefix. Its JSON encoding must
/// distinguish unequal keys and remain stable across releases.
pub trait ModelIdKind: Eq + Hash + Serialize + DeserializeOwned {
    const PREFIX: &'static str;
    type Alias: From<ModelFacingId>;
}

#[derive(Clone)]
pub struct ModelIdKey {
    pub kind: &'static str,
    pub key: String,
}

impl ModelIdKey {
    pub fn new<K: ModelIdKind>(key: &K) -> serde_json::Result<Self> {
        let mut key = serde_json::to_value(key)?;
        key.sort_all_objects();
        Ok(Self {
            kind: K::PREFIX,
            key: key.to_string(),
        })
    }
}

/// A namespace needs only its initial generator state and the external keys in allocation order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelIdMap {
    pub initial: ModelFacingIdGeneratorState,
    pub keys: Vec<String>,
}

impl ModelIdMap {
    pub fn new(seed: u64, kind: &'static str) -> Self {
        Self {
            initial: ModelFacingIdGenerator::new(seed, kind).snapshot(),
            keys: Vec::new(),
        }
    }

    pub fn lookup(&self, key: &str) -> Result<Option<ModelFacingId>, RestoreError> {
        self.keys
            .iter()
            .position(|bound| bound == key)
            .map(|ordinal| self.initial.id_at(ordinal as u64))
            .transpose()
    }
}
