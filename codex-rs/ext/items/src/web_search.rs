use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use ts_rs::TS;

// Standalone web-search item owned by the web extension. This is also the
// field-level representation exposed by app-server; core and rollout
// persistence only carry it inside an ExtensionItem envelope.
#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct WebSearchItem {
    pub id: String,
    pub query: String,
    pub action: Option<WebSearchAction>,
    /// Structured search results returned out-of-band by standalone web search.
    ///
    /// These stay as opaque JSON at the extension/app-server boundary so new
    /// result fields and result types can pass through without a Codex release.
    #[serde(default)]
    pub results: Option<Vec<JsonValue>>,
}

// App-server-facing description of the action performed by standalone web search.
#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type", rename_all = "camelCase")]
// Keep app-server's existing v2 TS path. The root WebSearchAction name is
// already used by the snake_case Responses API action type.
#[ts(export_to = "v2/")]
pub enum WebSearchAction {
    Search {
        query: Option<String>,
        queries: Option<Vec<String>>,
    },
    OpenPage {
        url: Option<String>,
    },
    FindInPage {
        url: Option<String>,
        pattern: Option<String>,
    },
    #[serde(other)]
    Other,
}
