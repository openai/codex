//! Types used when representing Model Context Protocol (MCP) values inside the
//! Codex protocol.
//!
//! We intentionally keep these types TS/JSON-schema friendly (via `ts-rs` and
//! `schemars`) so they can be embedded in Codex's own protocol structures.
use std::collections::HashMap;

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use ts_rs::TS;

/// ID of a request, which can be either a string or an integer.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(untagged)]
pub enum RequestId {
    String(String),
    #[ts(type = "number")]
    Integer(i64),
}

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RequestId::String(s) => f.write_str(s),
            RequestId::Integer(i) => i.fmt(f),
        }
    }
}

/// Definition for a tool the client can call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub description: Option<String>,
    pub input_schema: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub output_schema: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub annotations: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub icons: Option<Vec<serde_json::Value>>,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub meta: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct AdvertisedMcpToolCatalog {
    pub has_servers: bool,
    pub tools: HashMap<String, AdvertisedMcpToolInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct AdvertisedMcpToolInfo {
    /// Raw MCP server name used for routing the tool call.
    pub server_name: String,
    /// Model-visible tool name used in Responses API tool declarations.
    pub callable_name: String,
    /// Model-visible namespace used for deferred tool loading.
    pub callable_namespace: String,
    /// Instructions from the MCP server initialize result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub server_instructions: Option<String>,
    /// Serialized raw MCP tool definition used to rebuild model-visible schemas on resume.
    pub tool: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub connector_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub connector_name: Option<String>,
    #[serde(default)]
    pub plugin_display_names: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub connector_description: Option<String>,
}

/// A known resource that the server is capable of reading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct Resource {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub annotations: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub mime_type: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[ts(type = "number")]
    pub size: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub title: Option<String>,
    pub uri: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub icons: Option<Vec<serde_json::Value>>,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub meta: Option<serde_json::Value>,
}

/// Contents returned when reading a resource from an MCP server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(untagged)]
pub enum ResourceContent {
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    Text {
        /// The URI of this resource.
        uri: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        mime_type: Option<String>,
        text: String,
        #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        meta: Option<serde_json::Value>,
    },
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    Blob {
        /// The URI of this resource.
        uri: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        mime_type: Option<String>,
        blob: String,
        #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        meta: Option<serde_json::Value>,
    },
}

/// A template description for resources available on the server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ResourceTemplate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub annotations: Option<serde_json::Value>,
    pub uri_template: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub mime_type: Option<String>,
}

/// The server's response to a tool call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct CallToolResult {
    pub content: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub structured_content: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub is_error: Option<bool>,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub meta: Option<serde_json::Value>,
}

// === Adapter helpers ===
//
// These types and conversions intentionally live in `codex-protocol` so other crates can convert
// “wire-shaped” MCP JSON (typically coming from rmcp model structs serialized with serde) into our
// TS/JsonSchema-friendly protocol types without depending on `mcp-types`.

fn deserialize_lossy_opt_i64<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match Option::<serde_json::Number>::deserialize(deserializer)? {
        Some(number) => {
            if let Some(v) = number.as_i64() {
                Ok(Some(v))
            } else if let Some(v) = number.as_u64() {
                Ok(i64::try_from(v).ok())
            } else {
                Ok(None)
            }
        }
        None => Ok(None),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolSerde {
    name: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default, rename = "inputSchema", alias = "input_schema")]
    input_schema: serde_json::Value,
    #[serde(default, rename = "outputSchema", alias = "output_schema")]
    output_schema: Option<serde_json::Value>,
    #[serde(default)]
    annotations: Option<serde_json::Value>,
    #[serde(default)]
    icons: Option<Vec<serde_json::Value>>,
    #[serde(rename = "_meta", default)]
    meta: Option<serde_json::Value>,
}

impl From<ToolSerde> for Tool {
    fn from(value: ToolSerde) -> Self {
        let ToolSerde {
            name,
            title,
            description,
            input_schema,
            output_schema,
            annotations,
            icons,
            meta,
        } = value;
        Self {
            name,
            title,
            description,
            input_schema,
            output_schema,
            annotations,
            icons,
            meta,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResourceSerde {
    #[serde(default)]
    annotations: Option<serde_json::Value>,
    #[serde(default)]
    description: Option<String>,
    #[serde(rename = "mimeType", alias = "mime_type", default)]
    mime_type: Option<String>,
    name: String,
    #[serde(default, deserialize_with = "deserialize_lossy_opt_i64")]
    size: Option<i64>,
    #[serde(default)]
    title: Option<String>,
    uri: String,
    #[serde(default)]
    icons: Option<Vec<serde_json::Value>>,
    #[serde(rename = "_meta", default)]
    meta: Option<serde_json::Value>,
}

impl From<ResourceSerde> for Resource {
    fn from(value: ResourceSerde) -> Self {
        let ResourceSerde {
            annotations,
            description,
            mime_type,
            name,
            size,
            title,
            uri,
            icons,
            meta,
        } = value;
        Self {
            annotations,
            description,
            mime_type,
            name,
            size,
            title,
            uri,
            icons,
            meta,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResourceTemplateSerde {
    #[serde(default)]
    annotations: Option<serde_json::Value>,
    #[serde(rename = "uriTemplate", alias = "uri_template")]
    uri_template: String,
    name: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(rename = "mimeType", alias = "mime_type", default)]
    mime_type: Option<String>,
}

impl From<ResourceTemplateSerde> for ResourceTemplate {
    fn from(value: ResourceTemplateSerde) -> Self {
        let ResourceTemplateSerde {
            annotations,
            uri_template,
            name,
            title,
            description,
            mime_type,
        } = value;
        Self {
            annotations,
            uri_template,
            name,
            title,
            description,
            mime_type,
        }
    }
}

impl Tool {
    pub fn from_mcp_value(value: serde_json::Value) -> Result<Self, serde_json::Error> {
        Ok(serde_json::from_value::<ToolSerde>(value)?.into())
    }
}

impl Resource {
    pub fn from_mcp_value(value: serde_json::Value) -> Result<Self, serde_json::Error> {
        Ok(serde_json::from_value::<ResourceSerde>(value)?.into())
    }
}

impl ResourceTemplate {
    pub fn from_mcp_value(value: serde_json::Value) -> Result<Self, serde_json::Error> {
        Ok(serde_json::from_value::<ResourceTemplateSerde>(value)?.into())
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn resource_size_deserializes_without_narrowing() {
        let resource = serde_json::json!({
            "name": "big",
            "uri": "file:///tmp/big",
            "size": 5_000_000_000u64,
        });

        let parsed = Resource::from_mcp_value(resource).expect("should deserialize");
        assert_eq!(parsed.size, Some(5_000_000_000));

        let resource = serde_json::json!({
            "name": "negative",
            "uri": "file:///tmp/negative",
            "size": -1,
        });

        let parsed = Resource::from_mcp_value(resource).expect("should deserialize");
        assert_eq!(parsed.size, Some(-1));

        let resource = serde_json::json!({
            "name": "too_big_for_i64",
            "uri": "file:///tmp/too_big_for_i64",
            "size": 18446744073709551615u64,
        });

        let parsed = Resource::from_mcp_value(resource).expect("should deserialize");
        assert_eq!(parsed.size, None);
    }
}
