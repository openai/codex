//! JSON-RPC wire envelopes used by exec-server.
//!
//! Exec-server uses the Codex JSON-RPC dialect, which omits the
//! `"jsonrpc": "2.0"` field on the wire.

use std::fmt;

use codex_protocol::protocol::W3cTraceContext;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::de;
use serde::de::DeserializeSeed;
use serde::de::MapAccess;
use serde::de::SeqAccess;
use serde::de::Visitor;
use serde_json::Map;
use serde_json::Number;
use serde_json::Value;

pub const JSONRPC_VERSION: &str = "2.0";

// A maximum-size fs/walk response has at most 50,000 entries and needs roughly
// 150,000 JSON values. Keep ample headroom for legitimate protocol messages
// while preventing compact arrays from expanding into millions of heap values.
const MAX_JSONRPC_VALUE_NODES: usize = 256 * 1024;

#[derive(Debug, Clone, PartialEq, PartialOrd, Ord, Deserialize, Serialize, Hash, Eq)]
#[serde(untagged)]
pub enum RequestId {
    String(String),
    Integer(i64),
}

impl fmt::Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::String(value) => f.write_str(value),
            Self::Integer(value) => write!(f, "{value}"),
        }
    }
}

pub type Result = serde_json::Value;

/// Any valid exec-server JSON-RPC object that can be decoded from or encoded onto the wire.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum JSONRPCMessage {
    Request(JSONRPCRequest),
    Notification(JSONRPCNotification),
    Response(JSONRPCResponse),
    Error(JSONRPCError),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum JSONRPCMessageRepr {
    Request(JSONRPCRequest),
    Notification(JSONRPCNotification),
    Response(JSONRPCResponse),
    Error(JSONRPCError),
}

impl From<JSONRPCMessageRepr> for JSONRPCMessage {
    fn from(value: JSONRPCMessageRepr) -> Self {
        match value {
            JSONRPCMessageRepr::Request(request) => Self::Request(request),
            JSONRPCMessageRepr::Notification(notification) => Self::Notification(notification),
            JSONRPCMessageRepr::Response(response) => Self::Response(response),
            JSONRPCMessageRepr::Error(error) => Self::Error(error),
        }
    }
}

impl<'de> Deserialize<'de> for JSONRPCMessage {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut remaining = MAX_JSONRPC_VALUE_NODES;
        let value = BoundedValueSeed {
            remaining: &mut remaining,
        }
        .deserialize(deserializer)?;
        JSONRPCMessageRepr::deserialize(value)
            .map(Self::from)
            .map_err(de::Error::custom)
    }
}

struct BoundedValueSeed<'a> {
    remaining: &'a mut usize,
}

impl<'de> DeserializeSeed<'de> for BoundedValueSeed<'_> {
    type Value = Value;

    fn deserialize<D>(self, deserializer: D) -> std::result::Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        let Some(remaining) = self.remaining.checked_sub(1) else {
            return Err(de::Error::custom(format!(
                "JSON-RPC message exceeds the limit of {MAX_JSONRPC_VALUE_NODES} JSON values"
            )));
        };
        *self.remaining = remaining;
        deserializer.deserialize_any(BoundedValueVisitor {
            remaining: self.remaining,
        })
    }
}

struct BoundedValueVisitor<'a> {
    remaining: &'a mut usize,
}

impl<'de> Visitor<'de> for BoundedValueVisitor<'_> {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a JSON value within the exec-server complexity limit")
    }

    fn visit_bool<E>(self, value: bool) -> std::result::Result<Self::Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> std::result::Result<Self::Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_u64<E>(self, value: u64) -> std::result::Result<Self::Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_f64<E>(self, value: f64) -> std::result::Result<Self::Value, E> {
        Ok(Number::from_f64(value).map_or(Value::Null, Value::Number))
    }

    fn visit_str<E>(self, value: &str) -> std::result::Result<Self::Value, E> {
        Ok(Value::String(value.to_string()))
    }

    fn visit_string<E>(self, value: String) -> std::result::Result<Self::Value, E> {
        Ok(Value::String(value))
    }

    fn visit_none<E>(self) -> std::result::Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_some<D>(self, deserializer: D) -> std::result::Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        BoundedValueSeed {
            remaining: self.remaining,
        }
        .deserialize(deserializer)
    }

    fn visit_unit<E>(self) -> std::result::Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_seq<A>(self, mut sequence: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element_seed(BoundedValueSeed {
            remaining: &mut *self.remaining,
        })? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }

    fn visit_map<A>(self, mut object: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = Map::new();
        while let Some(key) = object.next_key::<String>()? {
            let value = object.next_value_seed(BoundedValueSeed {
                remaining: &mut *self.remaining,
            })?;
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}

/// A request that expects a response.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct JSONRPCRequest {
    pub id: RequestId,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace: Option<W3cTraceContext>,
}

/// A notification that does not expect a response.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct JSONRPCNotification {
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// A successful response to a request.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct JSONRPCResponse {
    pub id: RequestId,
    pub result: Result,
}

/// A response indicating that a request failed.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct JSONRPCError {
    pub error: JSONRPCErrorError,
    pub id: RequestId,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct JSONRPCErrorError {
    pub code: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    pub message: String,
}

#[cfg(test)]
#[path = "rpc_tests.rs"]
mod tests;
