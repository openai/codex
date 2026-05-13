use std::fmt::Display;

use schemars::JsonSchema;
use schemars::r#gen::SchemaGenerator;
use schemars::schema::Schema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;
use uuid::Uuid;

use crate::ThreadId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, TS, Hash)]
#[ts(type = "string")]
pub struct SessionId {
    pub(crate) uuid: Uuid,
}

impl SessionId {
    pub fn new() -> Self {
        Self {
            uuid: Uuid::now_v7(),
        }
    }

    pub fn from_string(s: &str) -> Result<Self, uuid::Error> {
        Ok(Self {
            uuid: Uuid::parse_str(s)?,
        })
    }
}

impl TryFrom<&str> for SessionId {
    type Error = uuid::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::from_string(value)
    }
}

impl TryFrom<String> for SessionId {
    type Error = uuid::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_string(value.as_str())
    }
}

impl From<SessionId> for String {
    fn from(value: SessionId) -> Self {
        value.to_string()
    }
}

impl From<ThreadId> for SessionId {
    fn from(value: ThreadId) -> Self {
        Self { uuid: value.uuid }
    }
}

impl From<SessionId> for ThreadId {
    fn from(value: SessionId) -> Self {
        ThreadId { uuid: value.uuid }
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.uuid, f)
    }
}

impl Serialize for SessionId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(&self.uuid)
    }
}

impl<'de> Deserialize<'de> for SessionId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        let uuid = Uuid::parse_str(&value).map_err(serde::de::Error::custom)?;
        Ok(Self { uuid })
    }
}

impl JsonSchema for SessionId {
    fn schema_name() -> String {
        "SessionId".to_string()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        <String>::json_schema(generator)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_id_default_is_not_zeroes() {
        let id = SessionId::default();
        assert_ne!(id.uuid, Uuid::nil());
    }

    #[test]
    fn converts_to_and_from_thread_id() {
        let thread_id = ThreadId::new();
        let session_id = SessionId::from(thread_id);

        assert_eq!(ThreadId::from(session_id), thread_id);
    }
}
