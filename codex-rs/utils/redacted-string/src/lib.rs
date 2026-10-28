use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use serde::Serializer;
use std::fmt;
use std::ops::Deref;
use std::ops::DerefMut;

/// A string whose `Debug` output is redacted.
#[derive(Clone, Default, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(transparent)]
pub struct RedactedString(String);

impl RedactedString {
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl Deref for RedactedString {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for RedactedString {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<String> for RedactedString {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for RedactedString {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl fmt::Debug for RedactedString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

/// An API response string whose serialized form always hides the value.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct SerdeRedactedString(RedactedString);

impl SerdeRedactedString {
    pub const PLACEHOLDER: &'static str = "[redacted]";
}

impl From<RedactedString> for SerdeRedactedString {
    fn from(value: RedactedString) -> Self {
        Self(value)
    }
}

impl Serialize for SerdeRedactedString {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::PLACEHOLDER)
    }
}
