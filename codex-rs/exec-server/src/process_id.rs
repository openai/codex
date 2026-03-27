use std::borrow::Borrow;
use std::fmt;
use std::ops::Deref;

use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProcessId(String);

impl ProcessId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl Deref for ProcessId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl Borrow<str> for ProcessId {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<str> for ProcessId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for ProcessId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<String> for ProcessId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for ProcessId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<&String> for ProcessId {
    fn from(value: &String) -> Self {
        Self(value.clone())
    }
}

impl From<ProcessId> for String {
    fn from(value: ProcessId) -> Self {
        value.0
    }
}
