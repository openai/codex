use std::collections::HashMap;

use serde_json::Value as JsonValue;

use crate::StoreLoadMode;

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub(crate) enum StoredValues {
    Enabled(HashMap<String, JsonValue>),
    #[default]
    Disabled,
}

impl StoredValues {
    pub(crate) fn from_mode(mode: StoreLoadMode) -> Self {
        match mode {
            StoreLoadMode::Enabled => Self::Enabled(HashMap::new()),
            StoreLoadMode::Disabled => Self::Disabled,
        }
    }

    pub(crate) fn is_enabled(&self) -> bool {
        matches!(self, Self::Enabled(_))
    }

    pub(crate) fn insert(&mut self, key: String, value: JsonValue) {
        if let Self::Enabled(values) = self {
            values.insert(key, value);
        }
    }

    pub(crate) fn get(&self, key: &str) -> Option<&JsonValue> {
        match self {
            Self::Enabled(values) => values.get(key),
            Self::Disabled => None,
        }
    }

    pub(crate) fn extend(&mut self, writes: Self) {
        match (self, writes) {
            (Self::Enabled(values), Self::Enabled(writes)) => values.extend(writes),
            (Self::Disabled, Self::Disabled) => {}
            (Self::Enabled(_), Self::Disabled) | (Self::Disabled, Self::Enabled(_)) => {
                debug_assert!(false, "stored value modes must match");
            }
        }
    }
}
