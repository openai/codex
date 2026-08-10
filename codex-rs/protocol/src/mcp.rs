//! Types used when representing Model Context Protocol (MCP) values inside the
//! Codex protocol.
//!
//! We intentionally keep these types TS/JSON-schema friendly (via `ts-rs` and
//! `schemars`) so they can be embedded in Codex's own protocol structures.
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use ts_rs::TS;

/// Extension ID for OpenAI form elicitation.
pub const OPENAI_FORM_EXTENSION_ID: &str = "openai/form";
/// Extension ID for standard MCP form elicitations that require user-entered input.
pub const OPENAI_STANDARD_FORM_INPUT_EXTENSION_ID: &str = "openai/standard-form-input";
/// Extension ID for MCP App UI rendering.
pub const MCP_APP_UI_EXTENSION_ID: &str = "io.modelcontextprotocol/ui";

/// Client extensions that must not be advertised to MCP servers.
const MCP_CLIENT_ONLY_EXTENSION_IDS: [&str; 1] = [OPENAI_STANDARD_FORM_INPUT_EXTENSION_ID];

/// MCP extensions supplied by the client that created a Codex session.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ClientMcpExtensions {
    extensions: HashMap<String, serde_json::Value>,
}

impl ClientMcpExtensions {
    /// Creates a session extension set from trusted client declarations.
    pub fn new(extensions: impl IntoIterator<Item = (String, serde_json::Value)>) -> Self {
        Self {
            extensions: extensions.into_iter().collect(),
        }
    }

    /// Returns whether the client declared the given extension.
    pub fn contains(&self, extension_id: &str) -> bool {
        self.extensions.contains_key(extension_id)
    }

    /// Returns the client's settings for the given extension.
    pub fn get(&self, extension_id: &str) -> Option<&serde_json::Value> {
        self.extensions.get(extension_id)
    }

    /// Returns only client extensions that should be advertised to MCP servers.
    pub fn for_mcp_servers(&self) -> Self {
        Self::new(
            self.extensions
                .iter()
                .filter(|(id, _)| !MCP_CLIENT_ONLY_EXTENSION_IDS.contains(&id.as_str()))
                .map(|(id, settings)| (id.clone(), settings.clone())),
        )
    }

    /// Iterates over the extensions and their settings.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &serde_json::Value)> {
        self.extensions
            .iter()
            .map(|(id, settings)| (id.as_str(), settings))
    }
}

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

/// Presentation metadata advertised by an initialized MCP server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct McpServerInfo {
    pub name: String,
    pub title: Option<String>,
    pub version: String,
    pub description: Option<String>,
    pub icons: Option<Vec<serde_json::Value>>,
    pub website_url: Option<String>,
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
    fn client_mcp_extensions_for_mcp_servers_excludes_client_only_extensions() {
        let openai_form_settings = serde_json::json!({ "version": 1 });
        let app_ui_settings = serde_json::json!({ "mimeTypes": ["text/html"] });
        let future_server_extension_settings = serde_json::json!({ "version": 2 });
        let extensions = ClientMcpExtensions::new([
            (
                OPENAI_FORM_EXTENSION_ID.to_string(),
                openai_form_settings.clone(),
            ),
            (MCP_APP_UI_EXTENSION_ID.to_string(), app_ui_settings.clone()),
            (
                OPENAI_STANDARD_FORM_INPUT_EXTENSION_ID.to_string(),
                serde_json::json!({}),
            ),
            (
                "example/future-server-extension".to_string(),
                future_server_extension_settings.clone(),
            ),
        ]);

        assert_eq!(
            extensions.for_mcp_servers(),
            ClientMcpExtensions::new([
                (OPENAI_FORM_EXTENSION_ID.to_string(), openai_form_settings),
                (MCP_APP_UI_EXTENSION_ID.to_string(), app_ui_settings),
                (
                    "example/future-server-extension".to_string(),
                    future_server_extension_settings,
                ),
            ])
        );
    }

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
